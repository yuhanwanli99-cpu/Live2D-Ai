//! \`test_inject\` 命令（L1 产品级波次：external-input 的壳内测试注入）。
//!
//! # 为什么单独一个文件
//!
//! \`lib.rs\` 已承载描述符 / settings_spec / 纯渲染函数 / runtime 与既有回归；
//! 本命令的「取参与校验」是**可独立单测的纯函数**，放这里既守住 \`lib.rs\` 的行数，
//! 也让「与 HTTP 端点同一条渲染口径」这件事有一处可以逐字对照的实现。
//!
//! # 与 \`POST /api/v1/external/chat\` 的关系
//!
//! **同一条渲染函数**（[\`render_from_config\`]，内部走 [\`render_injected_text\`]）、
//! **同一个** \`say_tx\` 主链。区别只有命令通道固有的三点：
//!
//! 1. **不需要 token**——命令通道只在本机同源 UI 内可达（见 \`mods_routes\`），
//!    没有独立的鉴权位；HTTP 端点则可能要求 env/config 里的 token；
//! 2. 不经过 HTTP 的 Origin / Content-Type 校验，也不走 \`403 mod_disabled\` 门禁：
//!    未启用时由 host 直接回 \`503 command_unavailable\`（可重试）；
//! 3. 长度上限是同口径**复刻**的常量 [\`MAX_INJECT_TEXT_LEN\`]——HTTP handler 的
//!    \`MAX_TEXT_LEN\` 是私有的、跨 crate 拿不到；两处都对**渲染后**文本判定，
//!    改动任何一处都要同步另一处（各自都有回归钉住 2000 这个数）。
//!
//! \`prefix\` 参数（可选 bool，默认 \`true\`）：\`true\` = 按 Mod 配置的
//! \`prefix\` + \`text_template\` 渲染（与 HTTP 端点逐字一致）；\`false\` = 文本
//! 已是最终形态，不再套用配置前缀/模板。面板把**本地渲染好的示例**填进输入框，
//! 发送时走 \`false\`，从而「看到什么就注入什么」，不会出现双重前缀。

use live2d_ai_mod_system::ModError;

use crate::render_from_config;

/// 命令名（host 的 \`POST /api/v1/mods/external-input/command\` 认这个名字）。
pub const COMMAND_TEST_INJECT: &str = "test_inject";

/// 渲染后文本的长度上限（字符）。
///
/// **必须**与 \`crates/live2d-ai-desktop/src/web_api/external_routes.rs\` 的
/// \`MAX_TEXT_LEN\` 保持一致：命令通道与 HTTP 端点送进的是同一支 \`say_tx\`，
/// 两条入口对同一条文本不该有不同判据。
pub const MAX_INJECT_TEXT_LEN: usize = 2000;

/// 命令返回体里的 \`note\`：一句话说清「与 HTTP 端点同源、但不需要 token」。
pub const INJECT_NOTE: &str = "与 POST /api/v1/external/chat 共用同一渲染口径与同一 say_tx；\
区别是本命令走 Mod 命令通道、不需要 token";

/// 从命令 args 取出并校验一条待注入文本。
///
/// - \`text\` 缺失 / 非字符串 / trim 后为空 → [\`ModError::Other\`]「text 必填且非空」；
/// - \`prefix\` 缺省 \`true\`：按 config 渲染（与端点同口径）；
///   显式 \`false\`：原样返回（调用方已自带最终文本）；
/// - 渲染后超过 [\`MAX_INJECT_TEXT_LEN\`] → [\`ModError::Other\`]，
///   文案含**实际长度**与**上限**（面板直接把这句话显示给用户）。
pub fn prepare_injection(
    config: &serde_json::Value,
    args: &serde_json::Value,
) -> Result<String, ModError> {
    let raw = args
        .get("text")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ModError::Other("text 必填且非空".to_string()))?;
    let apply_config = args.get("prefix").and_then(|v| v.as_bool()).unwrap_or(true);
    let rendered = if apply_config {
        render_from_config(config, raw)
    } else {
        raw.to_string()
    };
    let len = rendered.chars().count();
    if len > MAX_INJECT_TEXT_LEN {
        return Err(ModError::Other(format!(
            "注入文本过长：渲染后 {len} 字符，上限 {MAX_INJECT_TEXT_LEN} 字符\
（与 POST /api/v1/external/chat 同一口径）"
        )));
    }
    Ok(rendered)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> serde_json::Value {
        serde_json::json!({"prefix": "[弹幕] ", "text_template": "{text}"})
    }

    /// 缺省 \`prefix\` = \`true\`：渲染结果与端点用的 \`render_from_config\` **逐字一致**
    /// （这是「同一条渲染口径」的直接证据；不是另写一份逻辑）。
    #[test]
    fn default_render_matches_endpoint_render_from_config() {
        let config = cfg();
        let got = prepare_injection(&config, &serde_json::json!({"text": "  主播好  "})).unwrap();
        assert_eq!(got, render_from_config(&config, "主播好"));
        assert_eq!(got, "[弹幕] 主播好", "trim 后进渲染，prefix 生效");
    }

    /// 模板不含占位符时与端点一样按「字面前缀 + 文本」处理。
    #[test]
    fn template_without_placeholder_matches_endpoint() {
        let config = serde_json::json!({"text_template": "【外部消息】"});
        let got = prepare_injection(&config, &serde_json::json!({"text": "hi"})).unwrap();
        assert_eq!(got, render_from_config(&config, "hi"));
        assert_eq!(got, "【外部消息】hi");
    }

    /// 显式 \`prefix: false\`：文本已是最终形态，不套用配置前缀/模板
    /// （面板发送本地渲染结果时走这条，避免双重前缀）。
    #[test]
    fn prefix_false_injects_verbatim() {
        let got = prepare_injection(
            &cfg(),
            &serde_json::json!({"text": "[弹幕] 主播好", "prefix": false}),
        )
        .unwrap();
        assert_eq!(got, "[弹幕] 主播好", "不再叠加第二层 prefix");
    }

    /// \`prefix\` 非 bool 时回落默认 \`true\`（不 panic、不静默改变口径）。
    #[test]
    fn non_bool_prefix_falls_back_to_true() {
        let got =
            prepare_injection(&cfg(), &serde_json::json!({"text": "hi", "prefix": "no"})).unwrap();
        assert_eq!(got, "[弹幕] hi");
    }

    /// 空 / 缺失 / 非字符串 text → 同一句可读中文错误。
    #[test]
    fn blank_or_missing_text_is_readable_error() {
        for args in [
            serde_json::json!({}),
            serde_json::json!({"text": ""}),
            serde_json::json!({"text": "   "}),
            serde_json::json!({"text": 42}),
        ] {
            let err = prepare_injection(&cfg(), &args).unwrap_err();
            match err {
                ModError::Other(msg) => assert_eq!(msg, "text 必填且非空"),
                other => panic!("应为 Other，实为 {other:?}"),
            }
        }
    }

    /// 长度按**渲染后**判定：模板膨胀到 2001 → 错误文案含实际长度与上限；
    /// 恰好 2000 → 放行（边界与 HTTP 端点的 \`>\` 判据一致）。
    #[test]
    fn length_checked_after_render_with_actual_and_limit() {
        let config = serde_json::json!({"text_template": "x{text}"});
        let too_long = "a".repeat(MAX_INJECT_TEXT_LEN);
        let err = prepare_injection(&config, &serde_json::json!({"text": too_long})).unwrap_err();
        match err {
            ModError::Other(msg) => {
                assert!(
                    msg.contains(&(MAX_INJECT_TEXT_LEN + 1).to_string()),
                    "必须含实际长度：{msg}"
                );
                assert!(msg.contains("2000"), "必须含上限：{msg}");
            }
            other => panic!("应为 Other，实为 {other:?}"),
        }
        // 恰好等于上限：放行（判据是 \`>\`，不是 \`>=\`）。
        let ok =
            prepare_injection(&config, &serde_json::json!({"text": "a".repeat(1999)})).unwrap();
        assert_eq!(ok.chars().count(), MAX_INJECT_TEXT_LEN);
    }

    /// 上限常量与 HTTP handler 同值（跨 crate 的私有常量在这里被钉住）。
    #[test]
    fn limit_matches_http_endpoint_contract() {
        assert_eq!(MAX_INJECT_TEXT_LEN, 2000);
    }

    /// 命令名与 note 文本是面板/文档引用的稳定契约。
    #[test]
    fn command_name_and_note_are_stable() {
        assert_eq!(COMMAND_TEST_INJECT, "test_inject");
        assert!(INJECT_NOTE.contains("不需要 token"));
    }
}
