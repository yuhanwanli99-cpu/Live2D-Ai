//! 配置自检（`command("selftest")` 的真源）——纯函数、脱敏、可直接单测。
//!
//! 面板「检查配置」按钮 → `POST /api/v1/mods/voice-input/command`
//! body `{"command":"selftest"}` → host → `ModRuntime::command`
//! → [`config_selftest`] → `200 {"ok":true,"result":{…}}`。
//!
//! # 为什么要有它
//!
//! `backend` / `locale` 都是**宽容**配置：写错不会让链路死掉，而是静默回落
//! （未知 backend → `mock`，非法 locale → 拉丁归一化档）。宽容对可用性是好事，
//! 但「界面写着 sidecar、实际在 mock」这种名实不符只能靠一次显式自检抓出来。
//!
//! # 脱敏
//!
//! 返回体**只含结论，不含密钥**：token 只回 `token_set`（bool），
//! 绝不回明文、也不回长度。回归 `selftest_never_leaks_token` 直接断言。
//!
//! # 纯函数
//!
//! 本模块不读环境、不做 IO、不阻塞——`command` 在 web_api 线程上被调用，
//! 这里只做 JSON 计算（见 `live2d-ai-mod-system` 的 `ModRuntime::command` 契约）。

use serde_json::{Value, json};

use crate::normalize::{DEFAULT_LOCALE, LocaleProfile, locale_profile};
use crate::{RustRoute, VoiceBackend, locale_from_config, token_from_config};

/// `backend` 字段是否**已知**（缺省 = 合法，未知 = 会被静默回落）。
///
/// - 缺键 → `true`（缺省就是合法状态）；
/// - `"mock"` / `"sidecar"` → `true`；
/// - 其它字符串 / 非字符串 → `false`（[crate::VoiceBackend::from_config] 会宽容
///   回落 `mock`，但用户以为自己配好了——这正是自检要抓的名实不符）。
pub fn backend_is_known(config: &Value) -> bool {
    match config.get("backend") {
        None => true,
        Some(Value::String(s)) => matches!(s.as_str(), "mock" | "sidecar"),
        Some(_) => false,
    }
}

/// `backend` 是否**走了缺省/回落**（即没有显式写成一个已知值）。
pub fn backend_is_defaulted(config: &Value) -> bool {
    !matches!(
        config.get("backend"),
        Some(Value::String(s)) if matches!(s.as_str(), "mock" | "sidecar")
    )
}

/// `locale` 是否**走了缺省**（键缺失 / 空串 / 纯空白 / 非字符串）。
///
/// 与 [crate::locale_from_config] 的回落条件逐条一致：它把「缺 / 空 / 非字符串」
/// 一律换成 [`DEFAULT_LOCALE`]。
pub fn locale_is_defaulted(config: &Value) -> bool {
    !matches!(
        config.get("locale"),
        Some(Value::String(s)) if !s.trim().is_empty()
    )
}

/// BCP-47-lite 合法性：`语言`（2–3 位 ASCII 字母）+ 可选 `-`/`_` 分隔子标签
///（1–8 位 ASCII 字母数字）。
///
/// 不追求完整 BCP-47（本项目只按**语言族**选归一化档），只挡明显写错的取值
/// （`"!!"` / `"中文"` / `"zh CN"`）——它们会被 [locale_profile] 归到
/// **拉丁档**，于是中文转写里的词间空格不再被删。
pub fn locale_is_wellformed(locale: &str) -> bool {
    let mut parts = locale.split(['-', '_']);
    let Some(lang) = parts.next() else {
        return false;
    };
    if !(2..=3).contains(&lang.len()) || !lang.chars().all(|c| c.is_ascii_alphabetic()) {
        return false;
    }
    parts.all(|p| (1..=8).contains(&p.len()) && p.chars().all(|c| c.is_ascii_alphanumeric()))
}

/// 归一化档的稳定字符串（自检 / 日志共用）。
pub fn locale_profile_str(locale: &str) -> &'static str {
    match locale_profile(locale) {
        LocaleProfile::Cjk => "cjk",
        LocaleProfile::Latin => "latin",
    }
}

/// Rust 侧本地路由的稳定字符串（与 `VoiceBackend::rust_route` 一一对应）。
pub fn route_str(backend: VoiceBackend) -> &'static str {
    match backend.rust_route() {
        RustRoute::LocalInject => "local_inject",
        RustRoute::AcceptPush => "accept_push",
    }
}

/// **配置自检**：当前 `backend` / `locale` / `token` 是否自洽。
///
/// 返回体（脱敏，稳定键集）：
///
/// | key | 语义 |
/// |---|---|
/// | `ok` | `problems` 为空（后端与 locale 都自洽） |
/// | `backend` / `backend_valid` / `backend_defaulted` | 生效后端、是否已知、是否缺省 |
/// | `locale` / `locale_valid` / `locale_defaulted` / `locale_profile` | 生效 locale、是否合法、是否缺省、归一化档（`cjk`/`latin`） |
/// | `token_set` | 是否配了 token（**只回布尔**） |
/// | `route` / `opens_network` | 本地路由（`local_inject`/`accept_push`）；Rust 是否开 socket（恒 `false`） |
/// | `problems` | 硬问题（会让用户名实不符） |
/// | `notes` | 提醒（缺省值 / sidecar 未设 token），不是错误 |
pub fn config_selftest(config: &Value) -> Value {
    let backend = VoiceBackend::from_config(config);
    let backend_known = backend_is_known(config);
    let backend_defaulted = backend_is_defaulted(config);
    let locale = locale_from_config(config);
    let locale_defaulted = locale_is_defaulted(config);
    let locale_valid = locale_is_wellformed(&locale);
    let token_set = token_from_config(config).is_some();

    let mut problems: Vec<String> = Vec::new();
    if !backend_known {
        problems.push(
            "backend 写了 mock / sidecar 之外的值：服务端按 mock 处理（没有 ASR），发送方会以为在推 sidecar"
                .to_string(),
        );
    }
    if !locale_valid {
        problems.push(format!(
            "locale「{locale}」不是合法 BCP-47（如 zh-CN）：归一化落到「拉丁」档（保留空格、补中英边界），中文词间空格不会被删"
        ));
    }

    let mut notes: Vec<String> = Vec::new();
    if backend_defaulted {
        notes.push(
            "backend 未显式设置：当前生效 mock（没有 ASR，由集成方 / 测试直接喂转写文本）"
                .to_string(),
        );
    }
    if locale_defaulted {
        notes.push(format!(
            "locale 未显式设置：当前生效 {DEFAULT_LOCALE}（只影响转写文本的归一化）"
        ));
    }
    if backend == VoiceBackend::Sidecar && !token_set {
        notes.push(
            "backend=sidecar 且未设 token：服务只监听 127.0.0.1，但本机任何进程都能推转写；公用机器建议设一个 token"
                .to_string(),
        );
    }

    json!({
        "ok": problems.is_empty(),
        "backend": backend.as_str(),
        "backend_valid": backend_known,
        "backend_defaulted": backend_defaulted,
        "locale": locale,
        "locale_valid": locale_valid,
        "locale_defaulted": locale_defaulted,
        "locale_profile": locale_profile_str(&locale),
        "token_set": token_set,
        "route": route_str(backend),
        "opens_network": VoiceBackend::opens_network(backend),
        "problems": problems,
        "notes": notes,
    })
}
