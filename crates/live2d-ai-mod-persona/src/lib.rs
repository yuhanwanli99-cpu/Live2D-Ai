//! live2d-ai-mod-persona（rc.4 M5 立，persona-polish 打磨）——**酒馆角色卡标准 Mod**。
//!
//! # 它解决什么
//!
//! rc.4 起主链的 `[persona]` **只剩** `system_prompt` + `max_history_pairs`：
//! 角色扮演的「人设」不再内嵌进核心配置，而是这条 Mod 的职责。它把一张
//! SillyTavern 卡（V1 扁平 JSON / V2 嵌套 JSON / **PNG 内嵌 `chara` 负载**）
//! 合成一段 `system_prompt`，经**一等** `ModServices.apply_settings` 写回
//! 主链（写盘 + `supervisor.reload()`）。
//!
//! # 为什么写回主链而不是自己造一条对话通道
//!
//! 主链的 system 只有一处入口（`live2d-ai-runtime` 的 `ConversationConfig`）。
//! 让 Mod 在运行时另开一条人设通道，就等于在核心里偷偷埋第二个 system 来源——
//! 那是「第二个产品」的雏形。rc.4 的口径是：**Mod 可以加，主链只留一个入口**。
//!
//! # 启停语义（唯一真源 = Mod manifest 的 `enabled`）
//!
//! - **启用**：`mods.json` 里 `mods.persona.enabled = true`（前端「Mod 管理」
//!   或 `POST /api/v1/mods/persona/enable`）。host 才 `create` 本 runtime 并调
//!   [`ModRuntime::start`]：载入角色卡 → 合成 → 写回主链 `system_prompt`。
//! - **停用**：`disable`（或改 `mods.json` 后重启）。host 调 `shutdown`，
//!   主链 `system_prompt` **还原成启用前的基线**。
//! - 本 Mod 的 config / settings schema 里**没有**第二个 `enabled` 字段：
//!   「Mod 开关」只能有一处真相（与 `external-input` 同一条纪律，见
//!   `docs/architecture/mod-product-chain.md` §2）。config 里若混进 `enabled`
//!   键，它**不参与任何判断**（`PersonaConfig` 不读它）。
//!
//! # 坏输入 = 显式失败（不 panic、不静默、不动主链）
//!
//! 一条纪律：**没真的接管主链，就不许留下「接管了」的痕迹**。
//!
//! | 输入 | 结局 |
//! |---|---|
//! | `card_json` 不是 JSON / 不是角色卡 | `start` 返回 `Err` → `ModStatus::Failed`，主链提示词一字不动 |
//! | `card_path` 不存在 / 不是普通文件 / 读失败 | 同上，错误里带**路径**与**系统错误** |
//! | 卡文件 > [`MAX_CARD_FILE_BYTES`] | 同上；**读盘之前**就拒（`metadata` 先量），不做「读到 OOM 再报错」 |
//! | `card_json` > [`MAX_CARD_JSON_BYTES`] | 同上 |
//! | PNG 截断 / chunk 长度越界 / 无 `chara` / 压缩 iTXt | 同上（`tEXt`/未压缩 `iTXt` 之外一律明确报错） |
//! | 导入卡损坏 / 超大（`persona-mod-card.json`） | 同上；错误里带**路径**与「清除导入卡」的指引（面板上那个按钮） |
//! | `apply_settings` 写盘被拒 | 同上；基线快照**不落盘**（没接管就不记基线） |
//! | 卡没配（`card_path` / `card_json` 全空、也无覆盖） | **合法 no-op**：`Running`，主链提示词保持不变 |
//!
//! 「失败」用 host 的失败隔离（`mod-product-chain.md` §7）：本 Mod `Failed`，
//! 主链继续跑。**不要**改成「只写一行 warn 然后报 Running」——那会让界面显示
//! 「运行中」而实际什么都没发生（自检说谎）。
//!
//! # 禁用时怎么「回到仅主链 system_prompt」
//!
//! `apply_settings` 会真的改写 `live2d-ai.toml`，所以 Mod 必须能还回去。
//! 启用时它把**当前** `persona.system_prompt` 快照到 `persona-mod-base.txt`
//! （与 `live2d-ai.toml` 同目录，Mod 自己的状态文件），停用时写回该快照。
//! 快照只在**首次成功接管**后落盘，之后的启停都以它为基线，不会被合成产物污染。
//!
//! # 配置（住 `mods.json`，不回流主链）
//!
//! - `card_path`：角色卡文件路径（`.json` 或内嵌 `chara` 的 `.png`）；
//! - `card_json`：角色卡 JSON 文本（与路径二选一，**优先**）；
//! - `include_discipline`：是否附加对话纪律模板（缺省 true）；
//! - `say_first_mes`：启用时是否朗读卡里的开场白（缺省 false）；
//! - `name` / `description` / `personality` / `scenario`：手工覆盖（非空优先于卡）。
//!
//! # 与 memory 共存（last-writer-wins，无仲裁）
//!
//! `persona` 与 `memory`（`live2d-ai-mod-memory`）都可能写
//! `persona.system_prompt`。两侧共用同一段契约文字，与
//! `docs/architecture/memory-mod-v0.md` §5.1 **逐字一致**：
//!
//! > 两者都可能写 `persona.system_prompt`，规则是 **last-writer-wins，没有仲裁**：
//! >
//! > | 事件顺序 | 结果 |
//! > | --- | --- |
//! > | `persona` 后写 | 它的合成结果覆盖整个 `system_prompt`，**记忆块被冲掉** |
//! > | memory 后写（下一轮） | 它在 persona 的合成结果之上重新拼上记忆块 |
//! > | memory 停用 | `shutdown` 检查 marker，有就剥掉写回——**不留残留** |
//! >
//! > 这是**已知取舍**，不是 bug：主链只有一个 system 入口，加一套优先级表就是
//! > 在核心里埋第二个产品。要「两个都生效」必须先论证仲裁规则（见 §8 非目标）。
//!
//! 本 Mod 侧的直接结论：`start` 是**整段替换**（不做拼接、不解析 memory 的
//! marker），所以 persona 后写必然冲掉记忆块；`shutdown` 写回的是启用那一刻的
//! **整段**快照，若当时含记忆块就原样还回（memory 下一轮会幂等重拼或按需剥离）。
//! 回归在 `src/tests_e2e.rs`，用 memory crate 的**真实**
//! `strategy::compose_injection` / `strip_memory_block`（不是本地复刻）。
//!
//! # 产品级命令通道与运行态（persona-polish）
//!
//! 除了「改配置 → 重启」，本 Mod 还接了一条**一次性命令**通道
//!（`import_card` / `clear_import`，由 Mod 管理面板的「导入角色卡」驱动），
//! 并实现了 `ModRuntime::state_json`——`GET /api/v1/mods/persona/state`
//! 因此从**恒 503** 变成 200。两者的完整契约（为什么导入卡落成 Mod 自己的
//! 文件、为什么它优先于 config、失败怎么回滚）见 `src/command.rs` 头注。
//!
//! 一句话版本：**导入卡住 `persona-mod-card.json`，优先级高于 config 的
//! `card_json` / `card_path`；导入即写回主链；命令通道要 Mod 正在运行**
//!（停用时 host 回 503 `command_unavailable`），所以面板上的顺序是
//! 「先打开开关、再导入」。
//!
//! # 会话绑定（L1，2026-09-15）
//!
//! 带 `session_id` 的导入把卡写进宿主注入的**会话覆盖表**
//!（`ModServices.session_prompts`），**不碰**全局 `persona.system_prompt`：
//! 换会话不串卡，回原会话仍在（档案 `persona-mod-cards.json` 落盘，`start`
//! 时灌回）；`shutdown` 清空全部会话绑定 + 还原全局基线；不带 `session_id`
//!（或空串）仍是老全局语义；非空非法的 id → 可读错误，不写任何地方。
//! 完整契约见 `src/sessions.rs` 头注与 `docs/architecture/persona-mod-v0.md` §8。
//!
//! # 文件大小
//!
//! 源码 > 500 行（< 1000）：卡解析与主链写回共用同一份不变量（V2 判定 +
//! `system_prompt` 合成口径），拆成两文件只会让读者来回跳；测试已拆到
//! `src/tests.rs`，源码保持**一个文件顺序读完**。
//!
//! `src/tests.rs` 略超「测试文件 ≤ 800 行」：超出的部分是启停循环三条入口
//! （`card_json` / `card_path` JSON / `card_path` PNG）各自的端到端回归——
//! 合并成参数化用例会把「哪条入口坏了」这个信息藏起来。
//!
//! Wave 3 轨 E 新增的坏卡 E2E 与共存契约**另起** `src/tests_e2e.rs`（318 行），
//! 不再往 `tests.rs` 里加——两个文件各自守一组契约，也守住单文件行数纪律。
//!
//! persona-polish 波次同理，拆了两个文件：`src/command.rs`（命令通道 + 运行态快照）
//! 与 `src/tests_command.rs`（那两条面的回归）。**不拆的是主链接管语义**：卡解析、
//! 基线快照、`apply_card` 全留在本文件——它们共用同一条不变量
//!（「没真的接管，就不许留下痕迹」），拆开只会让读者来回跳。
//!
//! L1 会话绑定再拆一个 `src/sessions.rs`（档案读写 + 上限 + 灌回；本文件只留
//! 「什么时候载入 / 什么时候清」），回归进 `src/tests_e2e.rs`（那条文件守的正是
//! 「跨重启 / 跨组件」的端到端语义，且 `tests_command.rs` 已接近 800 行）。

mod card;
mod command;
mod factory;
mod runtime;
mod sessions;

use live2d_ai_mod_system::*;

// 拆分后对外 API 路径不变：`live2d_ai_mod_persona::PersonaCard` / `PersonaRuntime` /
// `PersonaFactory` / `FACTORY` / `compose_system_prompt` 仍从 crate 根可见。
pub use self::card::{PersonaCard, compose_system_prompt};
pub use self::factory::{FACTORY, PersonaFactory};
pub use self::runtime::PersonaRuntime;
/// Mod 描述符（静态身份）。
///
/// `version` 随行为变化走：persona-polish 起坏配置不再「假装 Running」，
/// 而是显式 `Failed`（见模块头注「坏输入 = 显式失败」）。
pub const DESCRIPTOR: ModDescriptor = ModDescriptor {
    id: "persona",
    name: "角色卡",
    version: "0.2.0",
    api_version: MOD_API_VERSION,
};

/// 基线快照文件名（与 `live2d-ai.toml` 同目录）。
const BASE_STATE_FILE: &str = "persona-mod-base.txt";

/// 界面导入的角色卡（与 `live2d-ai.toml` 同目录；见 `src/command.rs` 头注）。
///
/// **优先级高于** config 的 `card_json` / `card_path`：界面上的「导入」是一次
/// 显式动作，它必须能压过配置文件里写的卡；要回到配置里的卡就 `clear_import`。
pub const IMPORTED_CARD_FILE: &str = "persona-mod-card.json";

/// 角色卡**文件**大小上限（字节）。
///
/// 必须**在读盘之前**判断：`fs::read` 会先吃掉整份文件，一个误指向的 GB 级
/// 文件（或管道/设备文件）足以把常驻内存打穿——「坏输入」的正确结局是一条
/// 可读的错误，不是 OOM。
///
/// 为什么是 16 MiB：PNG 卡把整张立绘和 base64 的 JSON 塞进同一个文件，
/// 实测正常卡 < 2 MiB；16 MiB 余量给足，同时仍是一条能对用户解释清楚的界。
pub const MAX_CARD_FILE_BYTES: u64 = 16 * 1024 * 1024;

/// `card_json` **文本**大小上限（字节）。
///
/// `card_json` 住在 `mods.json` 里；典型 V2 卡 2–8 KiB，1 MiB 已远超任何
/// 真实角色卡，超过它基本等于「把文件内容贴错了地方」。
pub const MAX_CARD_JSON_BYTES: usize = 1024 * 1024;

/// 对话纪律模板（可关；配合「一句一单元」的语音契约：句数少 ⇒ TTS 请求少）。
const DISCIPLINE_TEMPLATE: &str = "【对话纪律】\n\
1. 每次回复只写 1-5 句，句子要短，像即时通讯。\n\
2. 每句话以真实句读结尾（。！？），不要用逗号把多层意思串成长句。\n\
3. 不要写 Markdown 标题/列表/加粗，不要替用户说话。";

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_command;
#[cfg(test)]
mod tests_e2e;

// 拆分后测试模块（`tests` / `tests_command` / `tests_e2e`）仍从 crate 根取这三项；
// 只在 test 构建下重导出，生产路径一律走各自模块内的直接引用。
#[cfg(test)]
pub(crate) use self::card::PNG_SIGNATURE;
#[cfg(test)]
pub(crate) use self::factory::persona_settings_spec;
#[cfg(test)]
pub(crate) use self::runtime::PersonaConfig;
#[cfg(test)]
pub(crate) use base64::Engine as _;
