//! **唯一模型根**（2026-09-12，rc.2 裁决）。
//!
//! 前端舞台（`/render` iframe）按 `/models/<id>/...` 取模型，服务端把它解析成
//! `<cwd>/assets/models/<id>/...`；模型库的 import / registry **必须**写同一个根，
//! 否则会出现「导入成功、界面也显示已激活，但皮套不换」——iframe 永远拉 cwd，
//! 被导入的模型却在别的地方。
//!
//! # 这里以前有两个根
//!
//! | 用途 | rc.2 之前 | 现在 |
//! |---|---|---|
//! | 静态 `/models/*`（`wasm_assets.rs`） | `<cwd>/assets/models/` | **同一个**（本模块） |
//! | registry / import（`models_routes`） | XDG `~/.local/share/live2d-ai/models/` | **同一个**（本模块） |
//!
//! XDG 那条路径已删除（该目录在本机从未存在，无需迁移）。**不得再引入第二个根**：
//! 两个根的表现是「激活了但没换皮」，而这不是报错，是最难查的一类缺陷。
//!
//! # 为什么是 cwd 相对
//!
//! 与静态服务用的是**同一个解析口径**，因此「进程 cwd 变了」会让两边一起失效，
//! 而不是让两边不一致（不一致才是真正难查的）。点火脚本 `scripts/ignite.sh`
//! 已把 cwd 固定在仓库根，`LIVE2D_AI_FLUTTER_WEB_DIR` 也由它锚定成绝对路径。
//!
//! 该目录被 `.gitignore` 排除（`assets/models/*`，仅 `README.md` / `.gitkeep`
//! 例外）：**仓库不分发模型二进制**，registry 也是可重建的运行时数据。

use std::path::PathBuf;

/// 渲染面自带默认模型的 URL（`/models/` 前缀，可直接 `GET`）。
///
/// **必须与 `l2d-wasm-demo` 的 `DEFAULT_MODEL_URL` 一致**——stage 首帧加载的就是
/// 这个地址，所以「没激活过任何模型」时状态栏报它是**如实**而非猜测。
/// 该常量在 `crates/l2d-wasm-demo/src/main.rs`（wasm bin，桌面侧不能依赖它，
/// 故此处重述；改一处必须改两处）。
pub(crate) const BUILTIN_MODEL_URL: &str = "/models/bai/runtime/bai.model3.json";

/// 仓库内模型根：`<cwd>/assets/models`；cwd 不在仓库根时**向上找**仓库根。
///
/// 静态服务与 registry / import 都只能用它，见模块头注。
///
/// 两级解析的理由与 `flutter_app` 的构建目录同一条：`/app/` 曾经因为「cwd 不对
/// → 503」而整片打不开，模型根同样会因 cwd 不对而「模型库是空的 / model_url 404」。
/// 常态是第一级命中（`scripts/ignite.sh` 把 cwd 固定在仓库根）；第二级是为
/// 「从 `crates/` 下启动」准备的，判定条件与 `flutter_app::repo_root` 同一口径
/// （同时含 `Cargo.toml` 与 `assets/`，避免在 `crates/*/Cargo.toml` 就停下）。
pub(crate) fn model_root() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let direct = cwd.join("assets").join("models");
    if direct.is_dir() {
        return direct;
    }
    for d in cwd.ancestors() {
        if d.join("Cargo.toml").is_file() && d.join("assets/models").is_dir() {
            return d.join("assets/models");
        }
    }
    direct
}

/// registry 文件路径：模型根**内部**的 `model_registry.json`。
///
/// 放在模型根内部而不是另找一个数据目录：两者是同一件事（「这个根里有哪些模型」），
/// 分开只会重新制造出两个根。该文件被 `.gitignore` 覆盖，属运行时数据。
pub(crate) fn registry_path() -> PathBuf {
    model_root().join("model_registry.json")
}

/// 把 `BUILTIN_MODEL_URL` 的 id 段取出来（`/models/bai/runtime/...` → `bai`）。
///
/// 失败（常量被写成别的形状）= 空串：宁可状态栏空着，也不要报一个猜出来的 id。
pub(crate) fn builtin_model_id() -> &'static str {
    BUILTIN_MODEL_URL
        .strip_prefix("/models/")
        .and_then(|rest| rest.split('/').next())
        .unwrap_or("")
}

/// 默认模型在磁盘上的 `*.model3.json` 绝对路径。
pub(crate) fn builtin_model3_path() -> PathBuf {
    let rel = BUILTIN_MODEL_URL
        .strip_prefix("/models/")
        .unwrap_or(BUILTIN_MODEL_URL);
    model_root().join(rel)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_root_is_the_single_root() {
        let root = model_root();
        assert!(
            root.ends_with("assets/models"),
            "模型根应以 assets/models 结尾：{}",
            root.display()
        );
        // registry 必须在模型根内部——两个根正是这次裁决要消灭的东西。
        assert!(
            registry_path().starts_with(&root),
            "registry 必须住在模型根内部：{}",
            registry_path().display()
        );
    }

    /// 从 crate 目录启动时也要落到仓库根（`cargo test` 的 cwd 就是 crate 目录）。
    #[test]
    fn model_root_walks_up_to_the_repo_root() {
        let root = model_root();
        assert!(
            root.join("bai/runtime/bai.model3.json").is_file() || !root.join("bai").is_dir(),
            "向上找到的根应含内置模型的 model3.json：{}",
            root.display()
        );
        assert!(
            root.parent().is_some_and(|p| p.ends_with("assets")),
            "模型根的父目录应是 assets：{}",
            root.display()
        );
    }

    /// 状态栏在两处必须报同一个 id：URL 常量与从 URL 推出的 id。
    #[test]
    fn builtin_id_matches_the_url() {
        assert_eq!(builtin_model_id(), "bai");
        assert!(BUILTIN_MODEL_URL.starts_with("/models/"));
        assert!(BUILTIN_MODEL_URL.ends_with(".model3.json"));
        assert!(builtin_model3_path().ends_with("bai/runtime/bai.model3.json"));
    }
}
