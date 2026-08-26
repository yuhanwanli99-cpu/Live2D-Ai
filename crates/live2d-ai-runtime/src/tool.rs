//! `live2d_perform_action` 的 **wire 类型**：只定义协议 JSON 形状。
//!
//! 职责边界（RFC D9）：
//! - 本模块**只**固定请求 `tools` 数组条目与参数解析类型；模型输出哪个动作、
//!   能力 gate（[`ModelCapabilities`]）与优先级仲裁（user > tool > rule）
//!   全部由 `live2d-ai-core` 处理，本 crate 不做任何策略判断；
//! - 动作名与 RFC D9 / core `ActionId::name()` 完全一致：
//!   `nod` / `shake_no` / `tilt` / `look_around` / `listen` / `surprise`
//!   （`release` 是控制命令，不是动作，不进 tool schema）。

use serde::{Deserialize, Serialize};

/// 固定的唯一工具名。
pub const PERFORM_ACTION_TOOL_NAME: &str = "live2d_perform_action";

/// 六个可播放动作的协议枚举（serde 协议名为 snake_case，与 core 一致）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PerformAction {
    /// 点头（同意/应和）。
    Nod,
    /// 摇头（否定）。
    ShakeNo,
    /// 歪头（疑惑/好奇）。
    Tilt,
    /// 环视 / 视线游移。
    LookAround,
    /// 倾听姿态。
    Listen,
    /// 惊讶。
    Surprise,
}

impl PerformAction {
    /// 全部动作（顺序与 RFC D9 表一致）。
    pub const ALL: [Self; 6] = [
        Self::Nod,
        Self::ShakeNo,
        Self::Tilt,
        Self::LookAround,
        Self::Listen,
        Self::Surprise,
    ];

    /// 稳定协议字符串（与 serde 序列化结果一致，由测试锁定）。
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Nod => "nod",
            Self::ShakeNo => "shake_no",
            Self::Tilt => "tilt",
            Self::LookAround => "look_around",
            Self::Listen => "listen",
            Self::Surprise => "surprise",
        }
    }
}

/// `live2d_perform_action` 参数 wire 类型：
/// `action` 六动作之一、`strength` 协议值 1..=3、`reason` 可选。
///
/// 仅定义形状；`strength` 越界或模型不支持的动作由 core 的 capability gate 拒绝，
/// 本 crate 不预校验（fail-closed 判定权在 core）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Live2dPerformActionArgs {
    /// 要表演的动作。
    pub action: PerformAction,
    /// 强度协议值：1 轻 / 2 中 / 3 强。
    pub strength: u8,
    /// 模型给出的触发理由（可选，仅用于观测/日志）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// 生成请求体 `tools` 数组条目（JSON Schema 固定，本 crate 唯一的工具）。
pub fn perform_action_tool() -> serde_json::Value {
    serde_json::json!({
        "type": "function",
        "function": {
            "name": PERFORM_ACTION_TOOL_NAME,
            "description": "驱动 Live2D 角色表演一个动作。仅在语义确实需要时调用；strength 表示幅度大小。",
            "parameters": {
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["nod", "shake_no", "tilt", "look_around", "listen", "surprise"],
                        "description": "动作名：点头/摇头/歪头/环视/倾听/惊讶"
                    },
                    "strength": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 3,
                        "description": "强度：1 轻、2 中、3 强"
                    },
                    "reason": {
                        "type": "string",
                        "description": "触发该动作的简短理由（可选）"
                    }
                },
                "required": ["action", "strength"],
                "additionalProperties": false
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_names_match_rfc_d9() {
        let names: Vec<&str> = PerformAction::ALL.iter().map(|a| a.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "nod",
                "shake_no",
                "tilt",
                "look_around",
                "listen",
                "surprise"
            ]
        );
    }

    #[test]
    fn serde_names_match_as_str() {
        for action in PerformAction::ALL {
            let json = serde_json::to_value(action).unwrap();
            assert_eq!(json, serde_json::json!(action.as_str()));
            let back: PerformAction = serde_json::from_value(json).unwrap();
            assert_eq!(back, action);
        }
    }

    #[test]
    fn args_roundtrip_with_and_without_reason() {
        let full = Live2dPerformActionArgs {
            action: PerformAction::ShakeNo,
            strength: 3,
            reason: Some("用户否定了提议".to_string()),
        };
        let v = serde_json::to_value(&full).unwrap();
        assert_eq!(
            v,
            serde_json::json!({
                "action": "shake_no",
                "strength": 3,
                "reason": "用户否定了提议"
            })
        );
        assert_eq!(
            serde_json::from_value::<Live2dPerformActionArgs>(v).unwrap(),
            full
        );

        // 无 reason 时字段整体省略（wire 上不出现 null）。
        let minimal = Live2dPerformActionArgs {
            action: PerformAction::Tilt,
            strength: 1,
            reason: None,
        };
        let v = serde_json::to_value(&minimal).unwrap();
        assert_eq!(v, serde_json::json!({ "action": "tilt", "strength": 1 }));
    }

    #[test]
    fn tool_schema_shape_is_fixed() {
        let tool = perform_action_tool();
        assert_eq!(tool["type"], "function");
        assert_eq!(tool["function"]["name"], PERFORM_ACTION_TOOL_NAME);
        let params = &tool["function"]["parameters"];
        assert_eq!(params["type"], "object");
        assert_eq!(params["properties"]["action"]["enum"][0], "nod");
        assert_eq!(
            params["properties"]["action"]["enum"]
                .as_array()
                .unwrap()
                .len(),
            6
        );
        assert_eq!(params["properties"]["strength"]["minimum"], 1);
        assert_eq!(params["properties"]["strength"]["maximum"], 3);
        assert_eq!(
            params["required"],
            serde_json::json!(["action", "strength"])
        );
        // reason 不在 required 里（可选）。
        assert!(
            !params["required"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v == "reason")
        );
    }

    #[test]
    fn model_tool_call_args_parse_into_wire_type() {
        let raw = r#"{"action":"surprise","strength":2,"reason":"saw a bug"}"#;
        let args: Live2dPerformActionArgs = serde_json::from_str(raw).unwrap();
        assert_eq!(args.action, PerformAction::Surprise);
        assert_eq!(args.strength, 2);
        assert_eq!(args.reason.as_deref(), Some("saw a bug"));
    }
}
