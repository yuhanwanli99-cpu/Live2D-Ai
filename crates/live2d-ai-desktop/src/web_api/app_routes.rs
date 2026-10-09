//! 应用信息路由（`/api/v1/app/capabilities` + `/api/v1/app/status`，D1 §1.1）。
//!
//! # 行数（AGENTS.md「源码 ≤500 行，豁免 ≤1000 需头注理由」）
//!
//! 本文件 >500 行（含 `#[cfg(test)]` 的内联测试模块）。生产部分是一个
//! **不变量密集**的整体：`StatusContext` 的晚绑定读源（epoch / 表演层计数）
//! + `build_status` 的 DTO 对齐；拆开会让「加一个 status 字段要改两处」。
//! 仍在 1000 行豁免上限内。
//!
//! 设计：
//! - `capabilities` **不**依赖写盘/网络（§1.1）；「常驻无配置」模式下也必须可用；
//! - `status` 只读 [`AppStatus`] 镜像（不持可变状态——D1 阶段 v1）。
//! - 返回内容**严格**走 DTO（[`dto::AppInfo`] / [`dto::AppStatus`]），
//!   字段顺序/形态 = §6 字段对齐表。

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use serde::Serialize;
use tiny_http::{Header, Response, StatusCode};

use live2d_ai_runtime::AppSettings;

use crate::web_api::dto;

/// `GET /api/v1/app/capabilities` 共享上下文（启动时构造；无 IO）。
#[derive(Debug, Clone)]
pub struct CapabilitiesContext {
    /// 进程启动时刻（SystemTime，Unix epoch 秒）。
    /// 当前未直接读取（capabilities 端点无 uptime 字段）——保留为可观测钩子。
    #[allow(dead_code)]
    pub started_at: SystemTime,
}

impl CapabilitiesContext {
    /// 启动时调用一次。
    pub fn new() -> Self {
        Self {
            started_at: SystemTime::now(),
        }
    }
}

impl Default for CapabilitiesContext {
    fn default() -> Self {
        Self::new()
    }
}

/// `/api/v1/app/status` 路由上下文：
/// - `settings`：当前生效的 `AppSettings`（启动时 + 后续 PATCH 后替换）；
/// - `config_path`：配置文件路径；
/// - `epoch`：当前 epoch（v1 = 0；P1WS-1 接入 supervisor 原子镜像后为真值）。
/// - `dev_mode` 原子可变：PATCH `/api/v1/settings` 切 dev_mode 时热更新
///   （日志端点门控取的是新值，不需重启）。
pub struct StatusContext {
    /// 当前 `AppSettings`（Mutex 保护；PATCH 写回时换新实例）。
    settings: std::sync::Mutex<AppSettings>,
    /// 配置文件绝对路径。
    pub config_path: String,
    /// dev-mode 开关（W7 任务：可配置运行时开关；PATCH 写盘后热更新）。
    /// 用 `AtomicBool` 替代裸 `bool`——`PATCH` handler 在 `swap_settings`
    /// 之后需调用 `set_dev_mode` 同步本字段。
    dev_mode: std::sync::atomic::AtomicBool,
    /// 启动时刻（用于 `uptime_s`）。
    pub started_at: SystemTime,
    /// P1WS-1：当前 epoch 读源（`Fn() -> u64`）。默认 = `|_| 0`（v1 占位）；
    /// 装配 supervisor 时由 [`StatusContext::set_epoch_source`] 替换为
    /// 读 [`crate::supervisor::SupervisorHandle::current_epoch`] 的闭包。
    /// 保留 `current_epoch` 字段（`AtomicU64`）作为旧调用路径的兼容
    /// fallback——老测试 `swap_settings_replaces_arc` 等不需要新签名。
    epoch_source: std::sync::Mutex<Box<dyn Fn() -> u64 + Send + Sync>>,
    /// 兼容 fallback：旧路径读这个（`build_status` 优先用 `epoch_source`，
    /// 否则回退到这里）。**P1WS-1 后**推荐改用 `epoch_source`。
    pub current_epoch: AtomicU64,
    /// **表演层计数读源**（2026-09-22）：`Fn() -> Option<Arc<PerformanceStats>>`。
    ///
    /// 装配 supervisor 时由 [`StatusContext::set_performance_source`] 注入
    /// （读 supervisor 当前那份运行时计数）；无 supervisor 时恒 `None`，
    /// status 里计数全 0 而配置段照常显示。**只读计数，不含正文与密钥。**
    #[allow(clippy::type_complexity)]
    performance_source: std::sync::Mutex<
        Box<
            dyn Fn() -> Option<std::sync::Arc<live2d_ai_runtime::performance::PerformanceStats>>
                + Send
                + Sync,
        >,
    >,
}

impl StatusContext {
    /// 构造 status 上下文。
    ///
    /// `dev_mode_override` = `Some(b)` 时**强制**用 `b`（CLI `--dev-mode`
    /// 路径，优先级最高）；`None` 时从 `settings.dev_mode` 派生。
    /// 装配路径（`cli_entry::run_web_mode`）就是用 CLI flag 决定这个参数；
    /// settings 缺省 + CLI 未传 = `settings.dev_mode` = `false`（默认关闭）。
    pub fn new(
        settings: AppSettings,
        config_path: String,
        dev_mode_override: Option<bool>,
    ) -> Self {
        let dev_mode = dev_mode_override.unwrap_or(settings.dev_mode);
        Self {
            settings: std::sync::Mutex::new(settings),
            config_path,
            dev_mode: std::sync::atomic::AtomicBool::new(dev_mode),
            started_at: SystemTime::now(),
            // P1WS-1：默认 epoch 读源 = 常 0；装配 supervisor 后由
            // `set_epoch_source` 覆盖。
            epoch_source: std::sync::Mutex::new(Box::new(|| 0)),
            // 表演层计数读源：默认恒 None（无 supervisor 装配时）。
            performance_source: std::sync::Mutex::new(Box::new(|| None)),
            current_epoch: AtomicU64::new(0),
        }
    }

    /// P1WS-1：注入 epoch 读源（一般 = `move || handle.current_epoch()`）。
    ///
    /// 装配 supervisor 后由 `cli_entry::run_web_mode` 在 `with_supervisor`
    /// 之后调用本方法；无 supervisor 装配时保持默认 0 读源。
    /// 锁短暂持锁（仅 `Box<dyn Fn>` 替换，**不**捕获 StatusContext 本身）。
    pub fn set_epoch_source(&self, f: Box<dyn Fn() -> u64 + Send + Sync>) {
        if let Ok(mut g) = self.epoch_source.lock() {
            *g = f;
        }
    }

    /// 注入**表演层计数读源**（一般 = `move || handle.performance_stats()`）。
    ///
    /// 装配 supervisor 后由 `cli_entry::run_web_mode` 调用；与 `epoch_source`
    /// 同一套晚绑定思路（`StatusContext` 先构造、supervisor 后装配）。
    pub fn set_performance_source(
        &self,
        f: Box<
            dyn Fn() -> Option<std::sync::Arc<live2d_ai_runtime::performance::PerformanceStats>>
                + Send
                + Sync,
        >,
    ) {
        if let Ok(mut g) = self.performance_source.lock() {
            *g = f;
        }
    }

    /// 替换当前 settings（PATCH 写回成功后调用）。
    ///
    /// Mutex 短暂持锁（仅 `AppSettings` clone 代价，无 IO）。若锁被毒化（极少）
    /// 走 `unwrap_or_default()`——status 端点必须**始终**可用（§1.1）。
    pub fn swap_settings(&self, new: AppSettings) {
        if let Ok(mut g) = self.settings.lock() {
            *g = new;
        }
    }

    /// 读出当前 settings 克隆（廉价 clone：`AppSettings` 字段都是 String）。
    pub fn settings_snapshot(&self) -> AppSettings {
        self.settings.lock().map(|g| g.clone()).unwrap_or_default()
    }

    /// 更新 dev_mode 开关（W7 任务：PATCH 写盘后热重载的同步入口）。
    ///
    /// 由 `PATCH /api/v1/settings` 路由在写盘成功后调用——`StatusContext`
    /// 的 `dev_mode` 必须跟随 `AppSettings.dev_mode` 翻转，否则日志端点
    /// 门控会出现「settings 已落盘但日志仍 403」的脏状态。
    pub fn set_dev_mode(&self, on: bool) {
        self.dev_mode
            .store(on, std::sync::atomic::Ordering::Release);
    }

    /// 读当前 dev_mode（日志端点门控与 status 字段均走此）。
    pub fn dev_mode(&self) -> bool {
        self.dev_mode.load(std::sync::atomic::Ordering::Acquire)
    }

    /// 从磁盘重读配置并刷新快照；`true` = 确实刷新了。
    ///
    /// # 为什么需要（2026-09-11 实测发现）
    ///
    /// 用户直接在编辑器里改 `live2d-ai.toml` 时，`file_watcher` 只调了
    /// `supervisor.reload()`（重建 LLM/TTS client），**没有**刷新
    /// `StatusContext` 里的快照。后果有两个，都是静默的：
    ///
    /// 1. `GET /api/v1/settings` 继续返回**改之前**的值——设置面板显示的和
    ///    磁盘上的不是一回事（实测：磁盘已改回人设，接口仍回上一次 PATCH 的值）；
    /// 2. 更糟的是 `PATCH` 以**快照**为基准做三态合并后整份写回——用户手改的
    ///    提示词会在下一次「保存」时被旧值覆盖回去，看起来就像「改了提示词就
    ///    被吃掉 / 改崩了」。
    ///
    /// 与 PATCH 路径共用同一套口径（换快照 + `dev_mode` 跟随），避免两条更新
    /// 路径各写一份而漂移。
    pub fn refresh_from_disk(&self, config_path: &str) -> bool {
        let updated = match AppSettings::load_from_path(config_path) {
            Ok(u) => u,
            Err(e) => {
                // 解析失败（编辑器保存到一半 / 语法错误）：保留旧快照——把损坏的
                // 配置换上去只会让界面跟着一起坏。但必须留痕：用户会问
                // 「为什么我改了没生效」。
                tracing::warn!(path = config_path, error = %e, "配置文件重读失败，保留旧快照");
                return false;
            }
        };
        if updated.dev_mode != self.dev_mode() {
            self.set_dev_mode(updated.dev_mode);
        }
        self.swap_settings(updated);
        true
    }
}

/// `GET /api/v1/app/capabilities` 处理器（纯函数 + context）。
///
/// 永不变动；无配置时也可正常返回。
pub fn handle_capabilities() -> Response<std::io::Cursor<Vec<u8>>> {
    let info = dto::capabilities_info();
    json_response(StatusCode(200), &info)
}

/// `GET /api/v1/app/status` 处理器。
///
/// `has_api_key` 的来源是**密钥真源**（`.env` 快照 > 进程环境，见
/// `live2d_ai_runtime::secrets`）：用 `std::env::var` 直接读会让「界面上刚写了
/// key、状态栏还说未配置」。
/// `active_model_id` 由调用方从**真实 registry** 读出（见
/// [`crate::web_api::models_routes::active_model_id`]）——这里不再有写死的 id。
pub fn handle_status(
    ctx: &StatusContext,
    active_model_id: String,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let settings = ctx.settings_snapshot();
    let status = build_status(
        ctx,
        &settings,
        &live2d_ai_runtime::secrets::lookup,
        active_model_id,
    );
    json_response(StatusCode(200), &status)
}

/// 把 [`StatusContext`] 序列化为 [`dto::AppStatus`]。
///
/// `active_model_id` 是**入参**而不是这里算出来的：它来自模型 registry
/// （`ModelStore`），而 `StatusContext` 只持配置与 epoch，看不到 registry。
pub fn build_status(
    ctx: &StatusContext,
    settings: &AppSettings,
    env_lookup: &dyn Fn(&str) -> Option<String>,
    active_model_id: String,
) -> dto::AppStatus {
    let uptime_s = SystemTime::now()
        .duration_since(ctx.started_at)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // P1WS-1：优先用 epoch_source（注入 = 读 supervisor 原子镜像）。
    // 锁短暂持锁；锁被毒化时回退到 current_epoch（保持 status 端点
    // **始终**可用——§1.1 红线）。
    let current_epoch = ctx
        .epoch_source
        .lock()
        .map(|g| g())
        .unwrap_or_else(|_| ctx.current_epoch.load(Ordering::Relaxed));
    dto::AppStatus {
        started_at: system_time_iso8601(ctx.started_at),
        uptime_s,
        config_path: ctx.config_path.clone(),
        active_model_id,
        // 2026-10-09：这里的两个数**读 `[tts]`**（出声链路的真实规格），
        // 不再回播放侧的调度常量 24000——那只是「没配音频时按多少赫兹理解」，
        // 把它显示成「当前语音采样率」就是在说一件用户以为的事
        //（本地出声是 44100）。常量本身留着，口径见 audio/spec.rs。
        audio: dto::AudioStatus {
            backend: "none",
            available: false,
            sample_rate: settings.tts.sample_rate,
            channels: settings.tts.channels,
        },
        llm: dto::llm_status_from(settings, env_lookup),
        tts: dto::tts_status_from(settings, env_lookup),
        dev_mode: ctx.dev_mode(),
        current_epoch,
        performance: performance_status(settings, ctx),
    }
}

/// 表演层摘要：**配置来自 `[performance]` 段，计数来自当前运行时**。
///
/// 锁毒化 / 未装配 → 计数全 0（status 端点必须始终可用，§1.1 红线）。
fn performance_status(settings: &AppSettings, ctx: &StatusContext) -> dto::PerformanceStatus {
    let stats = ctx.performance_source.lock().ok().and_then(|g| g());
    let p = &settings.performance;
    dto::PerformanceStatus {
        enabled: p.enabled,
        wired: p.is_wired(),
        base_url: p.base_url.clone(),
        model: p.model.clone(),
        has_api_key: p
            .api_key_env
            .as_deref()
            .is_some_and(|n| !n.trim().is_empty()),
        mode: live2d_ai_runtime::performance::StructuredMode::parse(&p.structured)
            .as_str()
            .to_string(),
        timeout_ms: p.effective_timeout_ms(),
        plans: stats.as_ref().map_or(0, |s| s.plans()),
        fallbacks: stats.as_ref().map_or(0, |s| s.fallbacks()),
        noops: stats.as_ref().map_or(0, |s| s.noops()),
        speak_turns: stats.as_ref().map_or(0, |s| s.speak_turns()),
        cue_turns: stats.as_ref().map_or(0, |s| s.cue_turns()),
        last_fallback: stats.as_ref().map_or_else(
            || "performance_ok".to_string(),
            |s| s.last_fallback().to_string(),
        ),
        last_structured: stats
            .as_ref()
            .and_then(|s| s.last_structured())
            .map(str::to_string),
    }
}

/// SystemTime → ISO 8601 UTC 毫秒精度。
///
/// 进程启动时刻的格式化，错误路径 = 占位 `1970-01-01T00:00:00.000Z`。
fn system_time_iso8601(t: SystemTime) -> String {
    let dur = t.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default();
    let total_ms = dur.as_millis();
    let secs = (total_ms / 1000) as i64;
    let ms = (total_ms % 1000) as u32;
    // 简化：UTC 直出 + 毫秒（避免引入 chrono；D2 可升级）。
    let (year, month, day, hour, min, sec) = unix_secs_to_ymdhms(secs);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}.{ms:03}Z")
}

/// Unix 秒 → (年, 月, 日, 时, 分, 秒)（UTC，简化算法；不依赖 chrono）。
///
/// 范围 [1970, 2100] 足够 D1 阶段；负数 / 越界回退为 1970-01-01 00:00:00。
fn unix_secs_to_ymdhms(secs: i64) -> (i32, u32, u32, u32, u32, u32) {
    if secs < 0 {
        return (1970, 1, 1, 0, 0, 0);
    }
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let hour = (rem / 3600) as u32;
    let min = ((rem % 3600) / 60) as u32;
    let sec = (rem % 60) as u32;
    // 1970-01-01 起的累计天数 → 年/月/日（Gregorian 简化）。
    let mut y = 1970i32;
    let mut d = days;
    loop {
        let leap = is_leap(y);
        let yd = if leap { 366 } else { 365 };
        if d < yd {
            break;
        }
        d -= yd;
        y += 1;
        if y > 2100 {
            return (1970, 1, 1, 0, 0, 0);
        }
    }
    let mdays = if is_leap(y) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut m = 0usize;
    while m < 12 && d >= mdays[m] as i64 {
        d -= mdays[m] as i64;
        m += 1;
    }
    (y, (m as u32) + 1, (d as u32) + 1, hour, min, sec)
}

fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// 通用：把 `Serialize` 包装成 200 + `Content-Type: application/json`。
pub fn json_response<T: Serialize>(
    status: StatusCode,
    body: &T,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let bytes = serde_json::to_vec(body).unwrap_or_else(|e| {
        format!("{{\"error\":{{\"code\":\"internal_error\",\"message\":\"{e}\"}}}}").into_bytes()
    });
    Response::from_data(bytes)
        .with_status_code(status)
        .with_header(
            Header::from_bytes(
                &b"Content-Type"[..],
                &b"application/json; charset=utf-8"[..],
            )
            .expect("Content-Type 头常量"),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn env_no_keys(_name: &str) -> Option<String> {
        None
    }

    fn env_with_keys(name: &str) -> Option<String> {
        if name == "LIVE2D_AI_LLM_API_KEY" {
            Some("sk-LIVE2D_AI_LLM_API_KEY-INJECTED".into())
        } else {
            None
        }
    }

    #[test]
    fn capabilities_handler_always_returns_200() {
        let resp = handle_capabilities();
        assert_eq!(resp.status_code().0, 200);
    }

    #[test]
    fn status_unconfigured_shows_zero_or_empty() {
        let settings = AppSettings::default();
        let ctx = StatusContext::new(settings, "/tmp/cfg.toml".into(), None);
        let st = build_status(&ctx, &AppSettings::default(), &env_no_keys, String::new());
        assert_eq!(st.config_path, "/tmp/cfg.toml");
        assert!(!st.llm.configured);
        // 2026-10-09（0.2.3-rc.1）：出厂 `[tts]` **有**端点（仓库自带的
        // MeloTTS 垫片，`settings::DEFAULT_TTS_BASE_URL`）——所以 TTS 不再
        // 「未配置」，这正是开箱出声的一半；LLM 仍要用户自己配端点与 Key。
        assert!(
            st.tts.configured,
            "缺省 [tts] 指本机 8091 的 MeloTTS 垫片，应当 configured"
        );
        assert!(!st.llm.has_api_key);
        assert!(!st.tts.has_api_key);
        // P0-1：JSON 序列化结果不含任何 env 变量名。
        let json = serde_json::to_string(&st).unwrap();
        assert!(!json.contains("api_key_env"));
        assert!(!json.contains("LIVE2D_AI_LLM"));

        // **显式**清空 `[tts].base_url` 才是「未配置」（空句路径）。
        let mut blank = AppSettings::default();
        blank.tts.base_url = String::new();
        let st_blank = build_status(&ctx, &blank, &env_no_keys, String::new());
        assert!(!st_blank.tts.configured, "空 base_url = 未配置");
    }

    #[test]
    fn status_reports_has_api_key_from_env_lookup() {
        let mut s = AppSettings::default();
        s.llm.base_url = "http://127.0.0.1:11434/v1".into();
        s.llm.model = "qwen2.5:7b".into();
        s.llm.api_key_env = Some("LIVE2D_AI_LLM_API_KEY".into());
        let ctx = StatusContext::new(s.clone(), "/tmp/cfg.toml".into(), None);
        // env 注入 → true。
        let st = build_status(&ctx, &s, &env_with_keys, String::new());
        assert!(st.llm.has_api_key);
        // env 未注入 → false。
        let st = build_status(&ctx, &s, &env_no_keys, String::new());
        assert!(!st.llm.has_api_key);
    }

    #[test]
    fn swap_settings_replaces_arc() {
        let s0 = AppSettings::default();
        let ctx = StatusContext::new(s0, "/tmp/cfg.toml".into(), None);
        let mut s1 = AppSettings::default();
        s1.llm.base_url = "http://x/v1".into();
        ctx.swap_settings(s1.clone());
        let snap = ctx.settings_snapshot();
        assert_eq!(snap.llm.base_url, "http://x/v1");
    }

    #[test]
    fn uptime_is_monotonic_after_creation() {
        let ctx = StatusContext::new(AppSettings::default(), "/tmp/cfg.toml".into(), None);
        let u0 = SystemTime::now()
            .duration_since(ctx.started_at)
            .unwrap()
            .as_secs();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let u1 = SystemTime::now()
            .duration_since(ctx.started_at)
            .unwrap()
            .as_secs();
        assert!(u1 >= u0);
    }

    #[test]
    fn iso8601_zero_is_1970_01_01() {
        assert_eq!(
            system_time_iso8601(SystemTime::UNIX_EPOCH),
            "1970-01-01T00:00:00.000Z"
        );
    }

    #[test]
    fn iso8601_known_instant() {
        // 2024-01-01T00:00:00Z = 1704067200 秒
        let t = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_704_067_200);
        assert_eq!(system_time_iso8601(t), "2024-01-01T00:00:00.000Z");
    }

    #[test]
    fn json_response_content_type_header() {
        let resp = handle_capabilities();
        let headers = resp.headers();
        let ct = headers
            .iter()
            .find(|h| h.field.equiv("Content-Type"))
            .expect("Content-Type header");
        assert!(ct.value.as_str().starts_with("application/json"));
    }

    /// P1WS-1：默认 epoch 读源 = 常 0；`set_epoch_source` 替换为注入值。
    #[test]
    fn status_current_epoch_defaults_to_zero_and_overridable() {
        let ctx = StatusContext::new(AppSettings::default(), "/tmp/cfg.toml".into(), None);
        // 默认读源 = 常 0。
        let s0 = build_status(&ctx, &AppSettings::default(), &env_no_keys, String::new());
        assert_eq!(s0.current_epoch, 0, "默认应 = 0");

        // 注入读源（v1 占位：返回固定 42）。
        ctx.set_epoch_source(Box::new(|| 42));
        let s1 = build_status(&ctx, &AppSettings::default(), &env_no_keys, String::new());
        assert_eq!(s1.current_epoch, 42, "注入后应 = 42");

        // 二次注入：返回 7。
        ctx.set_epoch_source(Box::new(|| 7));
        let s2 = build_status(&ctx, &AppSettings::default(), &env_no_keys, String::new());
        assert_eq!(s2.current_epoch, 7, "二次注入覆盖前值");

        // 兜底：清空到常量 0。
        ctx.set_epoch_source(Box::new(|| 0));
        let s3 = build_status(&ctx, &AppSettings::default(), &env_no_keys, String::new());
        assert_eq!(s3.current_epoch, 0, "再覆回 0");
    }

    /// P1WS-1：注入的 epoch 读源是**动态**的——多次 build_status 调用都走
    /// 最新读源（不缓存）。模拟 supervisor 推进 epoch 的时序。
    #[test]
    fn status_current_epoch_source_is_called_each_time() {
        use std::sync::atomic::AtomicU64;

        let ctx = StatusContext::new(AppSettings::default(), "/tmp/cfg.toml".into(), None);
        let counter = Arc::new(AtomicU64::new(10));
        let counter_for_closure = Arc::clone(&counter);
        ctx.set_epoch_source(Box::new(move || {
            counter_for_closure.load(Ordering::Relaxed)
        }));

        // 第一次读：counter=10。
        let s0 = build_status(&ctx, &AppSettings::default(), &env_no_keys, String::new());
        assert_eq!(s0.current_epoch, 10);

        // counter 推进到 99，**不**重新注入；下次 build_status 仍走读源。
        counter.store(99, Ordering::Release);
        let s1 = build_status(&ctx, &AppSettings::default(), &env_no_keys, String::new());
        assert_eq!(s1.current_epoch, 99, "读源应每次调用（不缓存）");
    }
}
