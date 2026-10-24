//! 打包临时树的宿主接线（**仅** `feature = "packed-mods"` 编译）。
//!
//! 这棵模块树只在 `scripts/pack_windows_glue.py` 生成的临时树里参与编译：
//! 那里七个 Mod crate 被静态链进 `live2d-ai-desktop`（工厂数 7、optional 依赖 +
//! `packed-mods` 特性）。主仓缺省提交**不编译**这里任何一行，三条前置路由仍是
//! `mod_front_routes` 里的未挂载存根（404 `mod_disabled`），外部注入泵不起。
//!
//! 边界：接线代码只做「宿主门 + 队列 + 调用钉死提交里已有的 Mod 纯函数」，
//! 不重写 Mod 协议、不改 `[llm]` / `[tts]`、不碰 `action_tx`。

pub mod a2a;
pub mod external;
pub mod pump;
pub mod voice;
