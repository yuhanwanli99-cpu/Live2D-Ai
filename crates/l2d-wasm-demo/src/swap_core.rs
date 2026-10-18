//! 平台无关的热换模型状态机（0.3.6 T1）。
//!
//! 为什么抽出来：0.3.5 的换模逻辑整份住在 wasm-only 的 `web/swap.rs` 里，原生
//! `cargo test` 编译不到——这个仓库在 `mouth.rs` / `stage_bg.rs` 上踩过同一个坑
//! （纯逻辑藏在 wasm 门控后面 = 没有回归）。本模块用 trait 把「取模型 / 装载核心 /
//! 重载字段表 / 报状态」与 wasm 解耦，wasm 与 native fake loader 各实现一份。
//!
//! # 不变量（0.3.5 起，修 0.3.6 T1 后逐条保持）
//!
//! - **成功才生效**：只有 `install` 成功后才 revoke 旧字段贡献 / pending、才更新
//!   `current_url`、才按新 model id 重载 `field_map`、才发 `loaded`；
//! - **失败保留旧模型**：`SwapHost::install` 的契约是**原子**的——`Err` 时旧核心
//!   必须一个字段都没被碰过。因此这里**不再**「重载旧 URL 回滚」（0.3.6 T1 修法 B
//!   把回滚路径整体消掉：它正是「对同一个 renderer 第二次 load_model」的唯一来源）；
//! - **generation 守卫**：请求发起时自增；每个 await 之后重查，被更新的请求取代的
//!   结果一律丢弃（连点只装最后一次）；
//! - **坏 URL 只回 error 帧**：不碰渲染核心、不动当前模型。

use std::future::Future;

use crate::preset::model_id_from_url;

/// 热换过程中宿主（wasm 渲染面 / native 测试）必须提供的副作用面。
pub(crate) trait SwapHost {
    /// 已解析、待装进渲染核心的模型类型（wasm = `LoadedModel`）。
    type Model;

    /// 新换模请求代际：自增并返回。每个请求（含随后被丢弃的）都必须走这里。
    fn begin_generation(&mut self) -> u64;

    /// 当前代际（用于 await 之后重查）。
    fn generation(&self) -> u64;

    /// 把模型装进渲染核心。**契约（原子）**：`Ok` 才表示新模型已生效；
    /// `Err` 时旧模型必须仍然完整可用（调用方不会再做任何回滚装载）。
    fn install(&mut self, model: &Self::Model) -> Result<(), String>;

    /// 成功后清旧字段贡献 / pending（idle 在 core 姿态栈里，不归这里）。
    fn revoke_fields(&mut self);

    /// 成功后记录新的 `current_url`。
    fn set_current_url(&mut self, url: String);

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

/// 换模成功后的字段映射重载（wasm = fetch `/actions/field_map.json`）。
pub(crate) trait FieldMapReloader {
    type Fut: Future<Output = String>;

    fn reload(&self, model_id: &str) -> Self::Fut;
}

/// 执行一次热换。调用方负责把返回值（无）之后的状态同步给上层。
///
/// 时序与 0.3.5 的 `web/swap.rs` 一致（只去掉失败后的回滚装载）：
/// 自增代际 → 拉取新模型 → generation 重查 → 原子装载 → 成功收尾 / 失败只回 error。
pub(crate) async fn run_swap<H, L, F>(host: &mut H, url: String, loader: &L, field_map: &F)
where
    H: SwapHost,
    L: ModelLoader<Model = H::Model>,
    F: FieldMapReloader,
{
    host.status("切换模型中…");
    let generation = host.begin_generation();
    host.phase(&format!("请求开始 url={url} generation={generation}"));

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
                host.emit("error", serde_json::json!({ "message": msg, "url": url }));
            }
            return;
        }
    };

    // 加载期间又来了新请求：旧结果直接丢弃（新请求会自己装核心）。
    if host.generation() != generation {
        host.phase(&format!("丢弃结果：generation 已变（新请求取代）url={url}"));
        return;
    }

    host.phase(&format!("开始原子装载核心 url={url}"));
    if let Err(e) = host.install(&loaded) {
        host.phase(&format!("装载失败：旧核心未受影响 url={url} err={e}"));
        if host.generation() == generation {
            // 修法 B：install 原子 → 旧模型还在，**不做**回滚装载，只回 error。
            let msg = format!("渲染核心换模型失败：{e}");
            host.status(&format!("{msg}（旧模型未受影响）"));
            host.emit("error", serde_json::json!({ "message": msg, "url": url }));
        }
        return;
    }
    host.phase(&format!("装载成功 url={url}"));

    // 成功：清旧贡献 / pending（idle 不受影响），记住新 URL。
    host.revoke_fields();
    host.set_current_url(url.clone());

    // 按新 id 重载映射（取不到时 wasm 侧会落回新 id 的内建表）。
    let model_id = model_id_from_url(&url);
    host.phase(&format!(
        "成功后：revoke 旧贡献 / pending；重载 field_map model_id={model_id}"
    ));
    let note = field_map.reload(&model_id).await;
    host.status(&format!("模型已切换（{note}）"));

    if host.generation() == generation {
        host.phase(&format!("完成：emit loaded url={url}"));
        host.emit("loaded", serde_json::json!({ "url": url }));
    } else {
        host.phase("完成但 generation 已变：不发 loaded");
    }
}

/// 原生 fake loader 单测：成功 / 失败（旧核心不变）/ 连点。
///
/// 单独成文件是为了守住「源码 ≤500 行」预算（逻辑本身在父模块）。
#[cfg(test)]
#[path = "swap_core_tests.rs"]
mod tests;
