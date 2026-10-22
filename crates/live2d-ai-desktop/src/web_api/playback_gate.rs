//! 浏览器播放忙上报（0.3.14）：POST /api/v1/playback + 宿主侧「播放忙」闸门。
//!
//! # 为什么需要它
//!
//! 0.3.1 之前 turn_in_flight 只表示「生成结束」，不表示「浏览器播完」：生成一
//! 结束外部注入就被放进来，于是后到的弹幕和还在播的上一轮抢 say。0.3.14 起
//! 外部注入的放行条件变成两条**同时**成立——本轮生成已结束 **且** 播放不忙
//! （见 external_queue 的 pump）。播放忙由浏览器（Flutter 壳）在「有音频在播 /
//! 播完」的翻边处静默上报：POST /api/v1/playback，body 只有 {"busy":true} /
//! {"busy":false}。
//!
//! # 契约
//!
//! - **缺省闲**：没有任何上报时 [PlaybackGate::is_busy] 恒 false——无头调用
//!   （curl / 脚本 / 测试）不会因为「浏览器没上报过」而把外部队列永久卡住；
//! - busy=true 之后若有**超过 [BUSY_STALE_AFTER]（约 120 秒）**没有新上报，
//!   视为闲，并记**一条** warn：浏览器标签页崩了 / 被挂起时队列不会被永久锁死。
//!   **不丢队列**——闸门只回答忙闲，「丢哪一条」只属于 external_queue 的字符
//!   预算规则（超时**不清队列**）；
//! - 非法 body（缺 busy / 不是布尔 / 不是 JSON / 不是对象）→ 400 + 统一错误体
//!   {"error":{"code":"invalid_payload",..}}；**状态不变**；
//! - 安全边界：本路由在 crate::web_api::security::is_mutating_route 里是
//!   mutating——Origin 同源 + Content-Type: application/json 由 dispatch 的
//!   现有校验收口，**不**另做绕过 dispatch 的前置路由；
//! - 只映射 POST：其它 method 不命中本路由，按既有口径落 RouteId::NotImplemented
//!   （501，与 settings/test/* 等 POST-only 端点同款）。
//!
//! # 确定性
//!
//! 时间由 [PlaybackGate::report_at] / [PlaybackGate::is_busy_at] 注入
//! （pub(crate)，测试把 now 拨到 120 秒之后），**测试不真睡**——一条测试跑 120 秒
//! 会把整个门禁拖成不可用的。

use std::io::Cursor;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tiny_http::{Response, StatusCode};

use crate::web_api::ServerContext;
use crate::web_api::app_routes::json_response;
use crate::web_api::dto::{ErrorDetail, ErrorResponse};

/// busy=true 之后超过这段时间没有新上报 → 视为闲（约 120 秒）。
///
/// 选 120 秒而不是几秒：正常一轮 TTS 音频可能几十秒；太短会把「还在播」误判成闲，
/// 太快放行外部队列就又变成抢话。
pub const BUSY_STALE_AFTER: Duration = Duration::from_secs(120);

/// 闸门内部状态：忙标志 + 最后一次上报时刻。
///
/// 两者是**一对**，必须一起读写（见 [PlaybackGate] 上「为什么用 Mutex」）。
#[derive(Debug)]
struct GateState {
    busy: bool,
    last_report: Instant,
}

/// 浏览器播放忙闸门：挂在 [ServerContext] 上，缺省闲。
///
/// # 为什么用 Mutex 而不是两个原子
///
/// 判据是「busy 为真**且**距 last_report 不足 120 秒」。若拆成两个独立原子，
/// 读到「标志仍是旧值、时刻已被新上报覆盖」这种撕裂组合，会把一次刚到的
/// busy=true 误判成超时。状态只有这一小对，锁的代价可以忽略。
#[derive(Debug)]
pub struct PlaybackGate {
    state: Mutex<GateState>,
}

impl PlaybackGate {
    /// 构造闸门：缺省**闲**（last_report 只是占位；busy=false 时从不读它）。
    pub fn new() -> Self {
        Self {
            state: Mutex::new(GateState {
                busy: false,
                last_report: Instant::now(),
            }),
        }
    }

    /// 上报播放忙闲（POST /api/v1/playback 的唯一写入口）。
    pub fn report(&self, busy: bool) {
        self.report_at(busy, Instant::now());
    }

    /// 当前是否「播放忙」。
    ///
    /// 缺省闲；busy=true 超时（[BUSY_STALE_AFTER]）视为闲并 warn 一次。
    #[allow(dead_code)] // 挂回 external-input 后是 external_queue::pump 的 ready 判据；当前无生产调用方。
    pub fn is_busy(&self) -> bool {
        self.is_busy_at(Instant::now())
    }

    /// 时间注入版写入（pub(crate)：测试把 now 拨到未来，**不真睡**）。
    pub(crate) fn report_at(&self, busy: bool, now: Instant) {
        let mut st = self.lock();
        st.busy = busy;
        st.last_report = now;
    }

    /// 时间注入版读取（见 [Self::report_at]；生产路径走 [Self::is_busy]）。
    pub(crate) fn is_busy_at(&self, now: Instant) -> bool {
        let mut st = self.lock();
        if !st.busy {
            return false;
        }
        if now.saturating_duration_since(st.last_report) > BUSY_STALE_AFTER {
            // 只 warn 一次：这里顺手把 busy 清零，同一窗口不会再进这条分支；
            // 下一次 report(true) 重新开窗。**不动任何队列**（队列归 external_queue）。
            tracing::warn!(
                stale_after_s = BUSY_STALE_AFTER.as_secs(),
                "播放忙上报超时：视为闲（不丢外部队列）"
            );
            st.busy = false;
            return false;
        }
        true
    }

    /// 取锁；互斥量中毒也继续用（这里只有「忙标志 + 时刻」两个 Copy 字段，
    /// 没有需要放弃维护的不变量——比在请求路径上 panic 好）。
    fn lock(&self) -> std::sync::MutexGuard<'_, GateState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl Default for PlaybackGate {
    fn default() -> Self {
        Self::new()
    }
}

/// POST /api/v1/playback 的请求体：只认 {"busy": true|false}。
#[derive(Debug, serde::Deserialize)]
struct PlaybackBody {
    /// 缺字段 → None → 400。**不**把「缺字段」当 false 静默放过：那样前端
    /// 拼错字段名会静默变成「播放闲」，队列立刻放行。
    busy: Option<bool>,
}

/// POST /api/v1/playback 的 200 响应体：回显写入的忙闲（便于调用方自检）。
#[derive(Debug, serde::Serialize)]
struct PlaybackAck {
    busy: bool,
}

/// 处理 POST /api/v1/playback（method / 安全校验已在 dispatch 入口完成）。
///
/// 成功 = 200 {"busy":<bool>}；非法 body = 400 invalid_payload，状态不变。
pub fn handle_playback(ctx: &ServerContext, body_str: &str) -> Response<Cursor<Vec<u8>>> {
    let Ok(parsed) = serde_json::from_str::<PlaybackBody>(body_str) else {
        return invalid_busy_response();
    };
    let Some(busy) = parsed.busy else {
        return invalid_busy_response();
    };
    ctx.playback_gate.report(busy);
    // 只记一个布尔（不记 body / 不含用户内容）；请求级日志由 dispatch 统一落。
    tracing::debug!(busy, "浏览器播放忙上报");
    json_response(StatusCode(200), &PlaybackAck { busy })
}

/// 缺 busy / 非布尔 / 非法 JSON 的统一 400（稳定码 invalid_payload）。
fn invalid_busy_response() -> Response<Cursor<Vec<u8>>> {
    let detail = ErrorDetail::with_details(
        "invalid_payload",
        "播放忙上报 body 必须是 {\"busy\":true} 或 {\"busy\":false}",
        serde_json::json!({"field": "busy"}),
    );
    json_response(StatusCode(400), &ErrorResponse::new(detail))
}

#[cfg(test)]
mod tests {
    use std::io::Read;
    use std::sync::{Arc, Mutex};

    use live2d_ai_runtime::AppSettings;
    use tiny_http::Method;

    use super::*;
    use crate::web_api::RouteId;
    use crate::web_api::security::SecurityContext;

    /// 严格安全模式（port=18080，要求同源 Origin + application/json）。
    fn strict_ctx() -> ServerContext {
        ServerContext::new(AppSettings::default(), "/tmp/cfg.toml".into(), None)
            .with_security(SecurityContext::new(18080, false))
    }

    /// 读响应体并解析成 JSON（本模块的成功/失败响应都必须是 JSON）。
    fn body_json(resp: Response<Cursor<Vec<u8>>>) -> serde_json::Value {
        let mut reader = resp.into_reader();
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf).expect("读响应体");
        serde_json::from_slice(&buf).expect("响应体必须是 JSON")
    }

    // ---------- 线程局部 tracing 采集（warn 只记一次） ----------

    /// 线程局部收集日志字节的 sink（只捕获**本线程**在这次闭包里的事件）。
    #[derive(Clone)]
    struct Sink(Arc<Mutex<Vec<u8>>>);

    impl std::io::Write for Sink {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Sink {
        type Writer = Sink;
        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    /// 在 with_default 下跑闭包并取回日志文本（与 logging.rs 测试同一手法）。
    fn capture_logs<F: FnOnce()>(f: F) -> String {
        use tracing_subscriber::layer::SubscriberExt;
        let buf = Arc::new(Mutex::new(Vec::new()));
        let subscriber = tracing_subscriber::registry().with(
            tracing_subscriber::fmt::layer()
                .with_writer(Sink(Arc::clone(&buf)))
                .with_ansi(false),
        );
        tracing::subscriber::with_default(subscriber, f);
        let bytes = buf
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        String::from_utf8_lossy(&bytes).into_owned()
    }

    // ---------- 闸门语义 ----------

    /// 缺省闲：没有任何上报时 is_busy 恒 false（无头调用不被卡住）。
    #[test]
    fn gate_defaults_to_idle_without_any_report() {
        let gate = PlaybackGate::new();
        assert!(!gate.is_busy(), "缺省必须是闲");
        assert!(
            !gate.is_busy_at(Instant::now() + Duration::from_secs(3600)),
            "从未上报过 busy 的闸门在任何时刻都应是闲"
        );
    }

    /// busy → idle 往返：窗口内读忙，report(false) 立即转闲。
    #[test]
    fn report_busy_then_idle_round_trip_within_window() {
        let gate = PlaybackGate::new();
        let t0 = Instant::now();
        gate.report_at(true, t0);
        assert!(
            gate.is_busy_at(t0 + Duration::from_secs(119)),
            "119s < 120s 仍忙"
        );
        gate.report_at(false, t0 + Duration::from_secs(119));
        assert!(
            !gate.is_busy_at(t0 + Duration::from_secs(119)),
            "report(false) 立即闲"
        );
        assert!(!gate.is_busy(), "生产读法同样闲");
    }

    /// busy=true 超过 120 秒无新上报 → 视为闲，且 warn **只记一条**。
    #[test]
    fn busy_older_than_120s_is_idle_and_warns_exactly_once() {
        let gate = PlaybackGate::new();
        let t0 = Instant::now();
        gate.report_at(true, t0);
        assert!(
            gate.is_busy_at(t0 + Duration::from_secs(60)),
            "窗口内仍是忙"
        );

        let logs = capture_logs(|| {
            assert!(
                !gate.is_busy_at(t0 + Duration::from_secs(121)),
                "超过 120s 没有新上报必须视为闲"
            );
            // 同一窗口再读一次：仍是闲，且不得再 warn 一遍。
            assert!(!gate.is_busy_at(t0 + Duration::from_secs(122)));
        });
        assert_eq!(
            logs.matches("播放忙上报超时").count(),
            1,
            "warn 只记一次；实际日志 = {logs}"
        );
    }

    /// 新的 busy=true 上报会从新时刻重新开窗（不会沿用旧窗口的剩余寿命）。
    #[test]
    fn fresh_busy_report_reopens_the_stale_window() {
        let gate = PlaybackGate::new();
        let t0 = Instant::now();
        gate.report_at(true, t0);
        gate.report_at(true, t0 + Duration::from_secs(100));
        assert!(
            gate.is_busy_at(t0 + Duration::from_secs(210)),
            "距最近上报 110s < 120s，仍忙"
        );
        assert!(
            !gate.is_busy_at(t0 + Duration::from_secs(221)),
            "距最近上报 121s > 120s，视为闲"
        );
    }

    // ---------- handler / 安全校验 / 路由 ----------

    /// handler：busy true/false 都 200，回显且真的写进 ServerContext 的闸门。
    #[test]
    fn handle_post_busy_true_false_updates_gate() {
        let ctx = strict_ctx();
        let resp = handle_playback(&ctx, r#"{"busy":true}"#);
        assert_eq!(resp.status_code().0, 200);
        assert!(ctx.playback_gate.is_busy(), "busy=true 必须写入闸门");
        assert_eq!(body_json(resp)["busy"], true);

        let resp = handle_playback(&ctx, r#"{"busy":false}"#);
        assert_eq!(resp.status_code().0, 200);
        assert!(!ctx.playback_gate.is_busy(), "busy=false 必须写入闸门");
        assert_eq!(body_json(resp)["busy"], false);
    }

    /// 缺布尔 / 非法 JSON / 非对象 → 400 invalid_payload，且**不改变**闸门状态。
    #[test]
    fn invalid_body_returns_400_invalid_payload_and_keeps_gate() {
        let ctx = strict_ctx();
        handle_playback(&ctx, r#"{"busy":true}"#);
        assert!(ctx.playback_gate.is_busy());

        for body in [
            "{}",
            r#"{"busy":null}"#,
            r#"{"busy":"yes"}"#,
            r#"{"busy":1}"#,
            "",
            "not json",
            "true",
            r#"{"busi":true}"#,
        ] {
            let resp = handle_playback(&ctx, body);
            assert_eq!(resp.status_code().0, 400, "body={body:?} 必须 400");
            let json = body_json(resp);
            assert_eq!(json["error"]["code"], "invalid_payload", "body={body:?}");
            assert_eq!(json["error"]["details"]["field"], "busy", "body={body:?}");
        }
        assert!(ctx.playback_gate.is_busy(), "400 不得改动闸门状态");
    }

    /// 安全校验来自 dispatch 的现有 mutating 通道（Origin 同源 + application/json）。
    #[test]
    fn dispatch_post_playback_enforces_same_origin_and_json_content_type() {
        let ctx = strict_ctx();
        let body = r#"{"busy":true}"#;

        let resp = crate::web_api::dispatch_with_security(
            &ctx,
            &Method::Post,
            "/api/v1/playback",
            body,
            None,
            Some("application/json"),
        );
        assert_eq!(resp.status_code().0, 403, "无 Origin 必须被拒");
        assert_eq!(body_json(resp)["error"]["code"], "origin_required");

        let resp = crate::web_api::dispatch_with_security(
            &ctx,
            &Method::Post,
            "/api/v1/playback",
            body,
            Some("http://evil.example:18080"),
            Some("application/json"),
        );
        assert_eq!(resp.status_code().0, 403, "跨源 Origin 必须被拒");
        assert_eq!(body_json(resp)["error"]["code"], "origin_denied");

        let resp = crate::web_api::dispatch_with_security(
            &ctx,
            &Method::Post,
            "/api/v1/playback",
            body,
            Some("http://127.0.0.1:18080"),
            Some("text/plain"),
        );
        assert_eq!(resp.status_code().0, 403, "非 JSON Content-Type 必须被拒");
        assert_eq!(body_json(resp)["error"]["code"], "content_type_required");

        let resp = crate::web_api::dispatch_with_security(
            &ctx,
            &Method::Post,
            "/api/v1/playback",
            body,
            Some("http://127.0.0.1:18080"),
            Some("application/json"),
        );
        assert_eq!(resp.status_code().0, 200, "同源 + JSON 必须通过");
        assert!(ctx.playback_gate.is_busy(), "通过 dispatch 也必须真的写入");
    }

    /// 路由只认 POST：非 POST 不命中 → 501 not_implemented（不误报 404/200）；
    /// is_mutating_route 把它分类为 mutating；路由字符串稳定。
    #[test]
    fn dispatch_playback_is_post_only_and_mutating_classified() {
        use crate::web_api::match_route;
        use crate::web_api::security::is_mutating_route;

        assert_eq!(
            match_route(&Method::Post, "/api/v1/playback"),
            RouteId::Playback
        );
        for method in [Method::Get, Method::Put, Method::Patch, Method::Delete] {
            assert_eq!(
                match_route(&method, "/api/v1/playback"),
                RouteId::NotImplemented,
                "{method} 不命中播放上报（与 POST-only 端点同款口径）"
            );
        }
        assert!(is_mutating_route(RouteId::Playback, &Method::Post));
        assert_eq!(RouteId::Playback.as_str(), "playback");

        let resp = crate::web_api::dispatch_with_security(
            &strict_ctx(),
            &Method::Get,
            "/api/v1/playback",
            "",
            None,
            None,
        );
        assert_eq!(resp.status_code().0, 501);
        assert_eq!(body_json(resp)["error"]["code"], "not_implemented");
    }
}
