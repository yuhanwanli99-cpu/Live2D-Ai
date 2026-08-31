//! D4 命令注册表（节点 D §7 落地，2026-08-29）。
//!
//! # 责任
//!
//! - 静态命令清单（**编译期**定，禁止动态注册）：6 个基础动作（对齐
//!   `live2d_ai_core::ActionId`）。
//! - 每个命令携带 `id` / `name` / `description` / `params_schema`（JSON
//!   Schema 2020-12 子集）/ `allowed_sources` / `kind`（恒为 `single`）。
//! - 提供 [`validate_params`] 与 [`lookup`] 两个**纯函数**——
//!   供 `commands_routes` 直接调用，单测可独立覆盖。
//!
//! # 设计抉择（v1 简洁优先）
//!
//! - **不含 `repeat` 参数**：`for _ in 0..repeat` 的伪实现连发同动作，
//!   core 仲裁第二条起返回 `AlreadyActive`，动作只播一次却返回
//!   `steps=N`，与"实际播放次数"不一致。v1 直接砍掉，前端按需连发即可。
//! - **不含 composed 命令**：v1 无真正顺序执行基础设施（`gap_ms`
//!   被 `let _ = gap_ms` 丢弃；同优先级"后来者胜"导致第一步被第二步
//!   抢占）。v1 直接砍掉，未来需要编排时再实现 `ActionSequence`。
//!
//! # 严格仲裁（§7.3）
//!
//! 本文件**不**直接构造 `SemanticAction` 也不调 `core::apply`；基础
//! 命令的语义动作由 `commands_routes` 内的
//! `SupervisorHandle::make_user_action(...)` 构造（user_command 来源，
//! 优先级 100），**都**走 supervisor `trigger_action` 通道 → core
//! `RootEvent::Action` 闸门（capability gate / 优先级 / 幂等）→
//! `forward_effects`。
//!
//! # 文件大小约束
//!
//! 模块 ≤ 500 行（与同级 `chat_routes.rs` / `dispatch.rs` 同模式）。
//!
//! # 与 plan 字段对齐
//!
//! | 字段 | Rust 类型 | 序列化 |
//! |---|---|---|
//! | `id` | `&'static str` | snake_case 稳定 |
//! | `name` | `&'static str` | i18n key（v1 留字符串） |
//! | `description` | `&'static str` | i18n key |
//! | `params_schema` | `serde_json::Value` | JSON Schema 子集 |
//! | `allowed_sources` | `&'static [&'static str]` | snake_case 与 `ActionSource` 同构 |
//! | `kind` | `CommandKind` | 恒为 `single` |
//!
//! 与 `docs/plans/node-d-api-contract-2026-08-28.md` §1.4 / §6.4 / §7.1
//! 字段对齐。

use serde_json::Value;
use serde_json::json;

/// 命令的触发形态（v1：恒为 `Single`，仅保留 future 扩展点）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
    /// 单一基础动作（映射到 `live2d_ai_core::ActionId`）；`invoke`
    /// 时把请求 params 翻译为单个 `SemanticAction`。
    Single {
        /// core [`ActionId`]（6 选 1）；name 与 `ActionId::name()` 一致。
        action: &'static str,
    },
}

/// 命令描述符（与 plan §7.1 `CommandDescriptor` 字段一一对应）。
#[derive(Debug, Clone)]
pub struct CommandDescriptor {
    /// 稳定协议 ID（snake_case；不可改名）。
    pub id: &'static str,
    /// i18n key / 显示名（v1 直接当显示文本）。
    pub name: &'static str,
    /// i18n key / 描述。
    pub description: &'static str,
    /// 参数 schema（JSON Schema 2020-12 子集：type / properties / required /
    /// enum / minimum / maximum / additionalProperties）。
    pub params_schema: Value,
    /// 允许的来源（v1 = `["user_command"]`；与 `live2d_ai_core::ActionSource`
    /// snake_case 同构）。
    pub allowed_sources: &'static [&'static str],
    /// 触发形态（v1 恒为 `Single`）。
    pub kind: CommandKind,
}

// ---------------------------------------------------------------- 单步 schema
//
// 6 个基础动作的 params_schema 共享同一形状（strength 1..=3 可选默认 2）。
// v1 不暴露 `action` 参数——它由命令 id 本身锁定（command == action）——
// 避免前端可改 action 字段绕过 schema。
//
// **为何不在 schema 暴露 `action` 字段**：plan §7.1 列举的命令清单
// 形式上把 action 列在 schema，但 D4 任务把"id == action"作为硬约束：
// 基础命令的 id 与 ActionId 一一对应，参数 schema 只能调强度，不可改
// 动作种类（防止前端构造 `{"action":"surprise"}` 配合 `nod` 命令 id
// 绕过仲裁）。
//
// **const 限制**：`json!` 宏不能直接放 const（内部调非 const 函数），
// 所以用 `LazyLock` 包装成运行时单例——编译期值已确定，无运行时成本。

use std::sync::LazyLock;

static SINGLE_BASE_SCHEMA: LazyLock<Value> = LazyLock::new(|| {
    json!({
        "type": "object",
        "properties": {
            "strength": {
                "type": "integer",
                "minimum": 1,
                "maximum": 3,
                "default": 2,
                "description": "动作强度 1..=3（默认 2）"
            }
        },
        "additionalProperties": false
    })
});

/// 返回单命令 schema 的引用（不复制；调用方应只在序列化时用一次）。
fn single_schema() -> &'static Value {
    &SINGLE_BASE_SCHEMA
}

/// **D4 静态命令清单**（v1：6 个基础动作）。
///
/// 与 `live2d_ai_core::ActionId::ALL` 同构（nod / shake_no / tilt /
/// look_around / listen / surprise）。新增命令 = 修改本数组 +
/// 在 `commands_routes` 单独处理 invoke（如果形态非基础）——
///
/// 禁止动态注册（§7.3 drift 防御）。
pub static COMMANDS: LazyLock<Vec<CommandDescriptor>> = LazyLock::new(|| {
    vec![
        // -------- 6 基础动作 --------
        CommandDescriptor {
            id: "live2d_perform_action_nod",
            name: "点头",
            description: "表演点头动作（同意 / 应和）",
            params_schema: single_schema().clone(),
            allowed_sources: &["user_command"],
            kind: CommandKind::Single { action: "nod" },
        },
        CommandDescriptor {
            id: "live2d_perform_action_shake_no",
            name: "摇头",
            description: "表演摇头动作（否定）",
            params_schema: single_schema().clone(),
            allowed_sources: &["user_command"],
            kind: CommandKind::Single { action: "shake_no" },
        },
        CommandDescriptor {
            id: "live2d_perform_action_tilt",
            name: "歪头",
            description: "表演歪头动作（疑惑 / 好奇）",
            params_schema: single_schema().clone(),
            allowed_sources: &["user_command"],
            kind: CommandKind::Single { action: "tilt" },
        },
        CommandDescriptor {
            id: "live2d_perform_action_look_around",
            name: "环视",
            description: "表演环视 / 视线游移动作",
            params_schema: single_schema().clone(),
            allowed_sources: &["user_command"],
            kind: CommandKind::Single {
                action: "look_around",
            },
        },
        CommandDescriptor {
            id: "live2d_perform_action_listen",
            name: "倾听",
            description: "表演倾听姿态",
            params_schema: single_schema().clone(),
            allowed_sources: &["user_command"],
            kind: CommandKind::Single { action: "listen" },
        },
        CommandDescriptor {
            id: "live2d_perform_action_surprise",
            name: "惊讶",
            description: "表演惊讶动作",
            params_schema: single_schema().clone(),
            allowed_sources: &["user_command"],
            kind: CommandKind::Single { action: "surprise" },
        },
    ]
});

/// 查找命令（id 不存在返回 `None`）。
pub fn lookup(id: &str) -> Option<&'static CommandDescriptor> {
    COMMANDS.iter().find(|c| c.id == id)
}

/// 命令总数（懒求值 + 编译期数量 = 6 的快速断言；运行时调用）。
#[allow(dead_code)] // 当前仅测试内使用；保留 API 供未来调度统计 / 校验使用。
pub fn count() -> usize {
    COMMANDS.len()
}

// ---------------------------------------------------------------- 参数校验

/// 校验结果（成功 → `Ok(parsed)`，失败 → `Err(msg)`）。
///
/// v1 仅 `Single{strength}`：未知字段一律拒绝（`additionalProperties: false`）。
pub fn validate_params(_cmd: &CommandDescriptor, params: &Value) -> Result<ParsedParams, String> {
    // 顶层必须为 object 或缺失（缺失 = 走默认）。
    let obj = match params {
        Value::Null => &Value::Object(Default::default()),
        Value::Object(_) => params,
        _ => return Err("params 必须是 JSON object".into()),
    };
    validate_single(obj)
}

/// 校验后的参数（v1：仅 `Single{strength}`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParsedParams {
    /// 基础命令。
    Single {
        /// 强度（1..=3）。
        strength: u8,
    },
}

fn validate_single(obj: &Value) -> Result<ParsedParams, String> {
    let o = obj.as_object().expect("caller ensured Object");
    // additionalProperties: false 强制（schema 一致性）。
    for k in o.keys() {
        if k != "strength" {
            return Err(format!("未知字段 {k:?}（仅允许 strength）"));
        }
    }
    let strength = match o.get("strength") {
        None => 2u8, // 默认 2 = Medium
        Some(Value::Number(n)) => {
            let n = n
                .as_u64()
                .ok_or_else(|| "strength 必须是正整数（u64 范围）".to_string())?;
            if !(1..=3).contains(&n) {
                return Err(format!("strength 必须在 1..=3 范围（收到 {n}）"));
            }
            n as u8
        }
        Some(other) => {
            return Err(format!("strength 必须是 integer（收到 {other:?}）"));
        }
    };
    Ok(ParsedParams::Single { strength })
}

// ---------------------------------------------------------------- 单元测试

// ---------------------------------------------------------------- 单元测试
//
// D4 注册表测试独立到 （保持主体 ≤500）。
