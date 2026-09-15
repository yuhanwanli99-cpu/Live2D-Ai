//! `gate` 的纯函数回归（L1 产品级，2026-09-15）。
//!
//! 从 `tests.rs` 拆出：crate 测试文件硬上限 800 行，而 L1 新增的
//! 四态 / 剥离 / 配置读取用例在这里成组。挂载见 `gate.rs` 末尾的 `#[path]`。

use super::*;

// ---------------------------------------------------- 唤醒闸 / 手动闸（L1，纯函数）

/// 四态 + 优先级：manual 压过总闸，总闸压过唤醒词，命中 → Allow 且已剥离。
#[test]
fn gate_four_states_and_priority() {
    assert_eq!(
        evaluate(&serde_json::json!({"manual_enabled": false}), "小爱你好"),
        GateOutcome::ManualOff,
        "手闸关优先于其它一切"
    );
    assert_eq!(
        evaluate(&serde_json::json!({}), "随便说"),
        GateOutcome::GateClosed,
        "没配 wake_phrase = 总闸关（优先级高于唤醒词检查）"
    );
    assert_eq!(
        evaluate(&serde_json::json!({"wake_phrase": "小爱"}), "你好"),
        GateOutcome::WakeRequired
    );
    assert_eq!(
        evaluate(&serde_json::json!({"wake_phrase": "小爱"}), "小爱关灯"),
        GateOutcome::Allow {
            text: "关灯".to_string()
        }
    );
}

/// 剥离细节：开头标点 / 空白、短语在中间、空白差异、大小写、整句只有唤醒词。
#[test]
fn gate_strips_phrase_ignoring_whitespace_and_case() {
    let zh = serde_json::json!({"wake_phrase": "小爱"});
    let allow = |text: &str| GateOutcome::Allow {
        text: text.to_string(),
    };
    assert_eq!(
        evaluate(&zh, "小爱，把窗户关小一点"),
        allow("把窗户关小一点"),
        "短语在开头 + 紧跟标点：标点一起去掉"
    );
    assert_eq!(
        evaluate(&zh, "小爱 把窗户关小一点"),
        allow("把窗户关小一点")
    );
    assert_eq!(
        evaluate(&zh, "请 小爱 关灯"),
        allow("请 关灯"),
        "短语在中间：删短语 + 折叠多余空白"
    );
    assert_eq!(
        evaluate(&zh, "小 爱 关灯"),
        allow("关灯"),
        "忽略空白差异：小 爱 命中 小爱"
    );
    assert_eq!(
        evaluate(&zh, "小爱！"),
        allow(""),
        "整句只有唤醒词 → 空正文（handler 按 empty_transcript 拒绝）"
    );
    assert_eq!(
        evaluate(&serde_json::json!({"wake_phrase": "Hey"}), "hey 你好"),
        allow("你好"),
        "拉丁唤醒词大小写不敏感"
    );
    assert_eq!(
        evaluate(&serde_json::json!({"wake_phrase": "小爱"}), ""),
        GateOutcome::WakeRequired,
        "空输入 = 没听见唤醒词"
    );
}

#[test]
fn gate_config_readers_are_lenient_and_trimmed() {
    assert_eq!(wake_phrase_from_config(&serde_json::json!({})), "");
    assert_eq!(
        wake_phrase_from_config(&serde_json::json!({"wake_phrase": "  小爱  "})),
        "小爱"
    );
    assert_eq!(
        wake_phrase_from_config(&serde_json::json!({"wake_phrase": 7})),
        ""
    );
    assert!(!wake_gate_open(&serde_json::json!({"wake_phrase": "  "})));
    assert!(wake_gate_open(&serde_json::json!({"wake_phrase": "小爱"})));
    assert!(manual_enabled_from_config(&serde_json::json!({})));
    assert!(!manual_enabled_from_config(
        &serde_json::json!({"manual_enabled": false})
    ));
    assert!(
        manual_enabled_from_config(&serde_json::json!({"manual_enabled": "no"})),
        "非布尔按缺省（宽容）"
    );
}
