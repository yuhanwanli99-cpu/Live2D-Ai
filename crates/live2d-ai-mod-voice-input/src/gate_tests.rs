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
        evaluate(&serde_json::json!({ "wake_phrase": "" }), "随便说"),
        GateOutcome::GateClosed,
        "显式空 wake_phrase = 总闸关（优先级高于唤醒词检查）"
    );
    assert_eq!(
        evaluate(&serde_json::json!({}), "随便说"),
        GateOutcome::WakeRequired,
        "缺该键 = 走产品缺省唤醒词（小可爱），总闸默认开"
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
        GateOutcome::WakeRequired,
        "短语在中间不命中（P0-4 句首锚定）"
    );
    assert_eq!(
        evaluate(&zh, "我昨天说 小爱 好看"),
        GateOutcome::WakeRequired,
        "中间出现唤醒词不得误触发（旧行为会把后半句发出去）"
    );
    assert_eq!(
        evaluate(&zh, "小 爱 关灯"),
        allow("关灯"),
        "忽略空白差异：小 爱 在句首命中 小爱"
    );
    assert_eq!(
        evaluate(&zh, "   小爱 关灯"),
        allow("关灯"),
        "前导空白不算「不在句首」"
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

// ---------------------------------------------------- PTT（按住说话，P0-4）

/// PTT 跳过唤醒匹配；手动闸 / 总闸不变；按住时仍说唤醒词则顺手剥掉。
#[test]
fn ptt_skips_wake_match_but_manual_and_total_gate_still_apply() {
    let zh = serde_json::json!({ "wake_phrase": "小爱" });
    // 裸正文（没有唤醒词）在 PTT 下放行。
    assert_eq!(
        evaluate_ptt(&zh, "今天天气怎么样"),
        GateOutcome::Allow {
            text: "今天天气怎么样".to_string()
        }
    );
    // 说了唤醒词 → 顺手剥掉，正文里不留。
    assert_eq!(
        evaluate_ptt(&zh, "小爱 今天天气怎么样"),
        GateOutcome::Allow {
            text: "今天天气怎么样".to_string()
        }
    );
    // 手动闸仍然优先。
    assert_eq!(
        evaluate_ptt(
            &serde_json::json!({"wake_phrase": "小爱", "manual_enabled": false}),
            "今天天气"
        ),
        GateOutcome::ManualOff
    );
    // 总闸（wake_phrase 显式空 = 语音输入整体关闭）仍然拦住 PTT。
    assert_eq!(
        evaluate_ptt(&serde_json::json!({ "wake_phrase": "" }), "今天天气"),
        GateOutcome::GateClosed
    );
    // evaluate 与 evaluate_mode(_, _, false) 完全等价（旧路径不变）。
    assert_eq!(evaluate(&zh, "你好"), evaluate_mode(&zh, "你好", false));
    // 空输入在 PTT 下是「放行但正文为空」，由 handler 按 empty_transcript 拒绝。
    assert_eq!(
        evaluate_ptt(&zh, ""),
        GateOutcome::Allow {
            text: String::new()
        }
    );
}

#[test]
fn gate_config_readers_are_lenient_and_trimmed() {
    // 键缺失 / 类型写错 → 产品缺省「小可爱」（新装默认开着唤醒词）。
    assert_eq!(
        wake_phrase_from_config(&serde_json::json!({})),
        DEFAULT_WAKE_PHRASE
    );
    assert_eq!(
        wake_phrase_from_config(&serde_json::json!({"wake_phrase": 7})),
        DEFAULT_WAKE_PHRASE
    );
    assert_eq!(
        wake_phrase_from_config(&serde_json::json!({"wake_phrase": "  小爱  "})),
        "小爱"
    );
    // 显式空 = 用户主动关总闸（这条语义不变）。
    assert_eq!(
        wake_phrase_from_config(&serde_json::json!({"wake_phrase": "  "})),
        ""
    );
    assert!(!wake_gate_open(&serde_json::json!({"wake_phrase": "  "})));
    assert!(wake_gate_open(&serde_json::json!({})));
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
