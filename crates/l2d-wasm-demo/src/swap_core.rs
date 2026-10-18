//! 平台无关的热换模型状态机（0.3.6 T1；0.3.6 T2 加模型身份）。
//!
//! 为什么抽出来：0.3.5 的换模逻辑整份住在 wasm-only 的 `web/swap.rs` 里，原生
//! `cargo test` 编译不到——这个仓库在 `mouth.rs` / `stage_bg.rs` 上踩过同一个坑
//! （纯逻辑藏在 wasm 门控后面 = 没有回归）。本模块用 trait 把「取模型 / 装载核心 /
//! 重载字段表 / 报状态」与 wasm 解耦，wasm 与 native fake loader 各实现一份。
//!
//! # 不变量（0.3.5 起，修 0.3.6 T1 后逐条保持）
//!
//! - **成功才生效**：只有 `install` 成功后才 revoke 旧字段贡献 / pending、才更新
//!   `current_url`、才**推进 `model_epoch`**、才按新 model id 装 `field_map`、才发 `loaded`；
//! - **失败保留旧模型**：`SwapHost::install` 的契约是**原子**的——`Err` 时旧核心
//!   必须一个字段都没被碰过。因此这里**不再**「重载旧 URL 回滚」（0.3.6 T1 修法 B
//!   把回滚路径整体消掉：它正是「对同一个 renderer 第二次 load_model」的唯一来源）；
//! - **失败不换 model_epoch**：`error` 帧带**当前（旧）模型**的 `model_id` /
//!   `model_epoch`，上层据此知道「旧模型仍在」；
//! - **generation 守卫**：请求发起时自增；每个 await 之后重查，被更新的请求取代的
//!   结果一律丢弃（连点只装最后一次）；字段表与 `loaded` 也在这个守卫后面——
//!   旧请求不得把旧皮套的映射表装到新皮套上；
//! - **坏 URL 只回 error 帧**：不碰渲染核心、不动当前模型。
//!
//! # 模型身份（0.3.6 T2 契约）
//!
//! 首次装载 `model_epoch = 1`；只有**成功**换模才 +1。`loaded` 与换模失败 `error`
//! 都带 `model_id` + `model_epoch`，`loaded` 另带字段表来源
//! `field_map: {source, degraded, reason}`（见 [`crate::preset::FieldMapReport`]）。

use std::future::Future;

use crate::preset::{FieldMap, FieldMapLoad, model_id_from_url};

/// 热换过程中宿主（wasm 渲染面 / native 测试）必须提供的副作用面。
pub(crate) trait SwapHost {
    /// 已解析、待装进渲染核心的模型类型（wasm = `LoadedModel`）。
    type Model;

    /// 新换模请求代际：自增并返回。每个请求（含随后被丢弃的）都必须走这里。
    fn begin_generation(&mut self) -> u64;

    /// 当前代际（用于 await 之后重查）。
    fn generation(&self) -> u64;

    /// 当前模型的渲染面代号（0.3.6 T2）：首次装载 = 1，成功换模才 +1。
    fn model_epoch(&self) -> u64;

    /// **成功换模后**推进模型代号并返回新值（失败路径绝不调用）。
    fn advance_model_epoch(&mut self) -> u64;

    /// 当前仍在用的模型 id（换模失败时 `error` 帧用它标明「旧模型仍在」）。
    fn current_model_id(&self) -> String;

    /// 把模型装进渲染核心。**契约（原子）**：`Ok` 才表示新模型已生效；
    /// `Err` 时旧模型必须仍然完整可用（调用方不会再做任何回滚装载）。
    fn install(&mut self, model: &Self::Model) -> Result<(), String>;

    /// 成功后清旧字段贡献 / pending（idle 在 core 姿态栈里，不归这里）。
    fn revoke_fields(&mut self);

    /// 成功后记录新的 `current_url`。
    fn set_current_url(&mut self, url: String);

    /// 成功后把**新模型**的字段表装进运行时（generation 已确认未变）。
    fn apply_field_map(&mut self, map: FieldMap);

    /// 页面状态栏（用户可见）。失败与成功都会写。
    fn status(&mut self, msg: &str);

    /// 分阶段诊断日志（wasm = console.log `[swap ...ms]`；native 测试 = 记录）。
    fn phase(&mut self, msg: &str);

    /// 向上层（Flutter / 宿主）发 v1 帧。
    fn emit(&mut self, ty: &str, payload: serde_json::Value);
}

/// 模型拉取（wasm = fetch model3 + 引用资源 + 内存组包；native 测试 = 脚本化结果）。
pub(crate) trait ModelLoader {
    type Model;
    type Fut: Future<Output = Result<Self::Model, String>>;

    fn load(&self, url: &str) -> Self::Fut;
}

/// 换模成功后的字段映射重建（wasm = fetch `/actions/field_map.json` + 纯逻辑重建）。
pub(crate) trait FieldMapReloader {
    type Fut: Future<Output = FieldMapLoad>;

    fn reload(&self, model_id: &str) -> Self::Fut;
}

/// 换模失败帧（0.3.6 T2）：`{message,url,model_id,model_epoch}`。
///
/// `model_id` / `model_epoch` 是**当前仍在用的旧模型**（失败不换 epoch），
/// `url` 是请求失败的新地址。
fn emit_error<H: SwapHost>(host: &mut H, message: String, url: &str) {
    host.emit(
        "error",
        serde_json::json!({
            "message": message,
            "url": url,
            "model_id": host.current_model_id(),
            "model_epoch": host.model_epoch(),
        }),
    );
}

/// 执行一次热换。
///
/// # 时序（0.3.6 T2 修订：**所有 await 都在任何状态变更之前**）
///
/// 自增代际 → 拉取新模型（await）→ generation 重查 → 重建字段表（await）→
/// generation 再查 → **一个无 await 的同步提交块**：原子装载 → `model_epoch` +1 →
/// revoke 旧贡献 / pending → 记新 URL → 装字段表 → 发 `loaded`。
///
/// 为什么把字段表提到装载之前：换模的成功回执（`loaded`）一旦发出，Flutter 就会
/// 记下新 `model_epoch`；若在「已装核心、已 +1」之后还有 await，其间到来的新请求
/// 会让这次换模的 `loaded` 被代际守卫丢弃——渲染面已经换了模型与代号，上层却永远
/// 不知道，后续 preset 帧带旧代号会被 Ignore，动作静默失效。现在只有两处 await，
/// 且都在提交之前：被取代的请求**一个状态都没改**（不装核心、不 +1、不换字段表、
/// 不发 loaded）。
///
/// 字段表取不到时 rebuild 仍返回**新 model id 的内建表** + 来源报告，因此不需要
/// 失败回滚；装载失败只回 `error`（旧模型 / 旧代号原样）。
pub(crate) async fn run_swap<H, L, F>(host: &mut H, url: String, loader: &L, field_map: &F)
where
    H: SwapHost,
    L: ModelLoader<Model = H::Model>,
    F: FieldMapReloader,
{
    host.status("切换模型中…");
    let generation = host.begin_generation();
    host.phase(&format!("请求开始 url={url} generation={generation}"));

    // ── await 1/2：拉取并解码新模型（不改任何状态）。 ──
    let loaded = match loader.load(&url).await {
        Ok(model) => {
            host.phase(&format!("资源就绪（清单 / 纹理 / moc3 全部解码）url={url}"));
            model
        }
        Err(e) => {
            host.phase(&format!("资源加载失败 url={url} err={e}"));
            // 被更新请求取代的失败不上报（否则连点会闪旧请求的错误）。
            if host.generation() == generation {
                let msg = format!("切换失败: {e}");
                host.status(&msg);
                emit_error(host, msg, &url);
            }
            return;
        }
    };

    // 加载期间又来了新请求：旧结果直接丢弃（新请求会自己装核心）。
    if host.generation() != generation {
        host.phase(&format!("丢弃结果：generation 已变（新请求取代）url={url}"));
        return;
    }

    // ── await 2/2：重建字段表（同样不改状态；取不到就回新 id 的内建表）。 ──
    let model_id = model_id_from_url(&url);
    host.phase(&format!(
        "装载前重建 field_map model_id={model_id}（generation={generation}）"
    ));
    let FieldMapLoad { map, report } = field_map.reload(&model_id).await;
    if host.generation() != generation {
        host.phase(&format!(
            "丢弃 field_map / loaded：generation 已变（未装核心、epoch 不变）url={url}"
        ));
        return;
    }

    // ── 从这里到 loaded 是**同步提交**：中途没有 await，不会有新请求插进来。 ──
    host.phase(&format!("开始原子装载核心 url={url}"));
    if let Err(e) = host.install(&loaded) {
        host.phase(&format!("装载失败：旧核心未受影响 url={url} err={e}"));
        // 修法 B：install 原子 → 旧模型还在，**不做**回滚装载，只回 error。
        let msg = format!("渲染核心换模型失败：{e}");
        host.status(&format!("{msg}（旧模型未受影响）"));
        emit_error(host, msg, &url);
        return;
    }
    host.phase(&format!("装载成功 url={url}"));

    // 成功提交：推进代号 → 清旧贡献 / pending（idle 不受影响）→ 记新 URL →
    // 装**新模型**的字段表 → 唯一一条 loaded。
    let model_epoch = host.advance_model_epoch();
    host.revoke_fields();
    host.set_current_url(url.clone());
    host.apply_field_map(map);
    host.status(&format!("模型已切换（{}）", report.reason));
    host.phase(&format!(
        "完成：emit loaded url={url} model_id={model_id} model_epoch={model_epoch}"
    ));
    host.emit(
        "loaded",
        serde_json::json!({
            "url": url,
            "model_id": model_id,
            "model_epoch": model_epoch,
            "field_map": report.to_json(),
        }),
    );
}

/// 原生 fake loader 单测：成功 / 失败（旧核心不变）/ 连点。
///
/// 单独成文件是为了守住「源码 ≤500 行」预算（逻辑本身在父模块）。
#[cfg(test)]
#[path = "swap_core_tests.rs"]
mod tests;
