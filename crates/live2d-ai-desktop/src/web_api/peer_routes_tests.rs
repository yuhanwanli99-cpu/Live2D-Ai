//! `peer_routes` 单测（0.4.5 rc.1，Unix）：HTTP 契约面，**不起网络端口**。
//!
//! 直接调 [`super::handle_peer_route`]（Origin / Content-Type / body 显式传入），
//! `Deferred` 在测试里同步 `job.run()` 执行——与 worker 线程里跑的是同一句。
//! 真 `pi` 不进测试：全部用临时目录里的假二进制。
//!
//! 覆盖：路径 / method、Origin + Content-Type 纪律、空白 text 400 且不 spawn、
//! session_id 校验、claude 404 `peer_unavailable` 且**无子进程**、忙 409、
//! 缺失 404 / 超时 504 / 坏 stdout 502、成功 200 只含助手正文。

use super::*;
use crate::web_api::peer_pi::{MAX_SESSION_ID_LEN, RunOverrides};
use crate::web_api::security::SecurityContext;
use live2d_ai_runtime::AppSettings;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const ORIGIN: &str = "http://127.0.0.1:18080";
const PATH_PI: &str = "/api/v1/peers/pi/message";
const PATH_CLAUDE: &str = "/api/v1/peers/claude/message";
const CT: &str = "application/json";

const OK_STDOUT: &str = r#"{"type":"session","version":3,"id":"s","timestamp":"t","cwd":"/tmp"}
{"type":"message_end","message":{"role":"assistant","stopReason":"stop","content":[{"type":"thinking","thinking":"内心"},{"type":"text","text":"只有这句"}]}}
"#;

// =====================================================================
// 夹具
// =====================================================================

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn new(tag: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!(
            "live2d-ai-peer-routes-{tag}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("建临时目录");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn write_script(dir: &Path, name: &str, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("写假二进制");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod 755");
    path
}

/// 写一个打印固定 stdout 的假 pi；返回 (程序路径, marker 目录)。
fn fake_pi_ok(tmp: &TempRoot, tag: &str) -> (PathBuf, PathBuf) {
    let marker = tmp.path().join(format!("marker-{tag}"));
    std::fs::create_dir_all(&marker).unwrap();
    std::fs::write(marker.join("stdout.json"), OK_STDOUT).unwrap();
    let m = marker.display();
    let program = write_script(
        tmp.path(),
        &format!("pi-{tag}"),
        &format!("echo ran > '{m}/ran.txt'\ncat '{m}/stdout.json'"),
    );
    (program, marker)
}

fn ctx(program: Option<PathBuf>, timeout: Duration, root: &Path) -> ServerContext {
    let ctx = ServerContext::new(
        AppSettings::default(),
        "/tmp/peers-route-test.toml".into(),
        None,
    )
    .with_security(SecurityContext::new(18080, false));
    ctx.peer_pi.set_run_overrides(RunOverrides {
        program,
        path_env: None,
        timeout: Some(timeout),
        runtime_root: Some(root.to_path_buf()),
    });
    ctx
}

fn call(ctx: &ServerContext, method: Method, path: &str, body: &str) -> Option<PeerOutcome> {
    call_full(ctx, method, path, body, Some(ORIGIN), Some(CT))
}

fn call_full(
    ctx: &ServerContext,
    method: Method,
    path: &str,
    body: &str,
    origin: Option<&str>,
    content_type: Option<&str>,
) -> Option<PeerOutcome> {
    handle_peer_route(ctx, &method, path, body, origin, content_type)
}

fn immediate(outcome: PeerOutcome) -> Response<Cursor<Vec<u8>>> {
    match outcome {
        PeerOutcome::Immediate(resp) => resp,
        PeerOutcome::Deferred(_) => panic!("期望立即响应，实际抢到了单飞名额"),
    }
}

fn execute(outcome: PeerOutcome) -> Response<Cursor<Vec<u8>>> {
    match outcome {
        PeerOutcome::Immediate(resp) => resp,
        PeerOutcome::Deferred(job) => job.run(),
    }
}

fn body_of(resp: Response<Cursor<Vec<u8>>>) -> String {
    let mut text = String::new();
    let _ = resp.into_reader().read_to_string(&mut text);
    text
}

fn status_of(resp: &Response<Cursor<Vec<u8>>>) -> u16 {
    resp.status_code().0
}

fn error_code(body: &str) -> String {
    let value: serde_json::Value = serde_json::from_str(body).expect("错误体是 JSON");
    value["error"]["code"]
        .as_str()
        .expect("error.code 是字符串")
        .to_string()
}

fn valid_body(text: &str, session_id: &str) -> String {
    serde_json::json!({"text": text, "session_id": session_id}).to_string()
}

// =====================================================================
// 路由 / method / 安全纪律
// =====================================================================

#[test]
fn only_peer_message_paths_are_handled() {
    let tmp = TempRoot::new("path");
    let ctx = ctx(None, Duration::from_secs(5), tmp.path());
    for path in [
        "/api/v1/peers",
        "/api/v1/peers/",
        "/api/v1/peers/pi",
        "/api/v1/peers/pi/message/extra",
        "/api/v1/chat",
        "/api/v1/peers//message",
    ] {
        assert!(
            call(&ctx, Method::Post, path, "").is_none(),
            "不应处理 {path}"
        );
    }
    assert!(call(&ctx, Method::Post, PATH_PI, "").is_some());
    assert!(call(&ctx, Method::Post, PATH_CLAUDE, "").is_some());
}

#[test]
fn non_post_methods_are_405() {
    let tmp = TempRoot::new("method");
    let ctx = ctx(None, Duration::from_secs(5), tmp.path());
    for method in [Method::Get, Method::Put, Method::Delete] {
        let resp = immediate(call(&ctx, method, PATH_PI, "").unwrap());
        assert_eq!(status_of(&resp), 405);
        assert_eq!(error_code(&body_of(resp)), "method_not_allowed");
    }
}

#[test]
fn origin_and_content_type_discipline_is_the_shared_one() {
    let tmp = TempRoot::new("security");
    let ctx = ctx(None, Duration::from_secs(5), tmp.path());
    let body = valid_body("问", "");

    // 无 Origin（严格模式）→ 403 origin_required。
    let resp = immediate(call_full(&ctx, Method::Post, PATH_PI, &body, None, Some(CT)).unwrap());
    assert_eq!(status_of(&resp), 403);
    assert_eq!(error_code(&body_of(resp)), "origin_required");

    // 跨域 Origin → 403 origin_denied。
    let resp = immediate(
        call_full(
            &ctx,
            Method::Post,
            PATH_PI,
            &body,
            Some("http://evil.example:18080"),
            Some(CT),
        )
        .unwrap(),
    );
    assert_eq!(status_of(&resp), 403);
    assert_eq!(error_code(&body_of(resp)), "origin_denied");

    // 带 body 且 Content-Type 不是 JSON → 403 content_type_required。
    let resp = immediate(
        call_full(
            &ctx,
            Method::Post,
            PATH_PI,
            &body,
            Some(ORIGIN),
            Some("text/plain"),
        )
        .unwrap(),
    );
    assert_eq!(status_of(&resp), 403);
    assert_eq!(error_code(&body_of(resp)), "content_type_required");
}

// =====================================================================
// 请求体校验（400；全部不 spawn、不占单飞名额）
// =====================================================================

#[test]
fn blank_text_is_400_and_never_spawns() {
    let tmp = TempRoot::new("blank");
    let (program, marker) = fake_pi_ok(&tmp, "blank");
    let ctx = ctx(Some(program), Duration::from_secs(5), tmp.path());
    for body in [
        valid_body("", ""),
        valid_body("   \n\t ", "sess-1"),
        r#"{"session_id":""}"#.to_string(),
        "not json".to_string(),
    ] {
        let resp = immediate(call(&ctx, Method::Post, PATH_PI, &body).unwrap());
        assert_eq!(status_of(&resp), 400, "body={body}");
        assert_eq!(error_code(&body_of(resp)), "invalid_payload");
    }
    assert!(!ctx.peer_pi.is_busy(), "400 不得占用单飞名额");
    // 假二进制若被执行会写 ran.txt；400 路径根本不会 spawn。
    assert!(!marker.join("ran.txt").exists(), "400 不得启动子进程");
}

#[test]
fn session_id_validation_rejects_without_spawning_and_accepts_valid() {
    let tmp = TempRoot::new("session-id");
    let (program, _marker) = fake_pi_ok(&tmp, "sid");
    let root = tmp.path().join("root");
    let ctx = ctx(Some(program), Duration::from_secs(5), &root);

    let too_long = "a".repeat(MAX_SESSION_ID_LEN + 1);
    for bad in ["a/b", "a.b", "a b", "中文", too_long.as_str()] {
        let resp = immediate(call(&ctx, Method::Post, PATH_PI, &valid_body("问", bad)).unwrap());
        assert_eq!(status_of(&resp), 400, "session_id={bad:?}");
        assert_eq!(error_code(&body_of(resp)), "invalid_payload");
        assert!(!ctx.peer_pi.is_busy(), "非法 session_id 不得抢名额");
    }

    // 合法：64 字符、下划线 / 连字符、空串（一次性会话）都进入 Deferred。
    let longest = "a".repeat(MAX_SESSION_ID_LEN);
    for ok in ["sess-1", "a_b-c", longest.as_str(), ""] {
        let outcome = call(&ctx, Method::Post, PATH_PI, &valid_body("问", ok)).unwrap();
        match outcome {
            PeerOutcome::Deferred(job) => drop(job), // 只验证校验通过，不跑子进程
            PeerOutcome::Immediate(_) => panic!("合法 session_id {ok:?} 不应被 400"),
        }
        assert!(!ctx.peer_pi.is_busy(), "drop job 必须释放单飞名额");
    }
}

// =====================================================================
// claude / 未知 peer：404 peer_unavailable，且不 spawn
// =====================================================================

#[test]
fn claude_is_404_peer_unavailable_without_lookup_or_spawn() {
    let tmp = TempRoot::new("claude");
    let (program, marker) = fake_pi_ok(&tmp, "claude");
    let ctx = ctx(Some(program), Duration::from_secs(5), tmp.path());

    // body 故意非法：证明 claude 在**读 body 之前**就被拒（更别说找二进制）。
    let resp = immediate(call(&ctx, Method::Post, PATH_CLAUDE, "not json").unwrap());
    assert_eq!(status_of(&resp), 404);
    let body = body_of(resp);
    assert_eq!(error_code(&body), "peer_unavailable");
    assert!(
        body.contains("这一版不接 Claude Code"),
        "message 锁定：{body}"
    );
    let _ = &marker;
    assert!(!ctx.peer_pi.is_busy(), "claude 请求不得占用单飞名额");
    // 假 pi 若被执行会打印 stdout，marker 里没有它写的任何东西（脚本只 cat 固定文件）。
    assert!(
        call(&ctx, Method::Post, PATH_CLAUDE, &valid_body("问", "")).is_some(),
        "claude 路径要被本模块接住（而不是掉回 501）"
    );
    assert_eq!(
        status_of(&immediate(
            call(&ctx, Method::Post, PATH_CLAUDE, &valid_body("问", "")).unwrap()
        )),
        404
    );
}

#[test]
fn unknown_peer_id_is_peer_unavailable() {
    let tmp = TempRoot::new("unknown");
    let ctx = ctx(None, Duration::from_secs(5), tmp.path());
    let resp = immediate(
        call(
            &ctx,
            Method::Post,
            "/api/v1/peers/other/message",
            &valid_body("问", ""),
        )
        .unwrap(),
    );
    assert_eq!(status_of(&resp), 404);
    assert_eq!(error_code(&body_of(resp)), "peer_unavailable");
}

// =====================================================================
// 忙 / 缺失 / 超时 / 坏输出 / 成功
// =====================================================================

#[test]
fn busy_is_409_immediately_and_recovers_after_release() {
    let tmp = TempRoot::new("busy");
    let (program, _marker) = fake_pi_ok(&tmp, "busy");
    let ctx = ctx(Some(program), Duration::from_secs(5), tmp.path());
    let guard = ctx.peer_pi.try_acquire().expect("先占住名额");
    let resp = immediate(call(&ctx, Method::Post, PATH_PI, &valid_body("第二条", "s")).unwrap());
    assert_eq!(status_of(&resp), 409);
    let body = body_of(resp);
    assert_eq!(error_code(&body), "peer_busy");
    assert!(body.contains("上一条还在等 Pi"), "message 锁定：{body}");
    drop(guard);
    let outcome = call(&ctx, Method::Post, PATH_PI, &valid_body("第三条", "s")).unwrap();
    assert!(
        matches!(outcome, PeerOutcome::Deferred(_)),
        "释放后应能再抢"
    );
}

#[test]
fn second_request_during_real_child_in_flight_is_409_not_queued() {
    let tmp = TempRoot::new("inflight");
    // 真起一个假子进程（sleep 到注入的短超时），第二个请求必须**立即** 409。
    let program = write_script(tmp.path(), "pi-sleepy", "sleep 30");
    let root = tmp.path().join("root");
    let ctx = ctx(Some(program), Duration::from_millis(800), &root);
    let outcome = call(&ctx, Method::Post, PATH_PI, &valid_body("第一条", "")).unwrap();
    let worker = std::thread::spawn(move || execute(outcome));
    // 等子进程真的起来 / 名额被占住（最多 1s），避免用固定 sleep 靠运气。
    let wait_until = Instant::now() + Duration::from_secs(1);
    while !ctx.peer_pi.is_busy() {
        assert!(Instant::now() < wait_until, "第一条没有占住名额");
        std::thread::sleep(Duration::from_millis(5));
    }
    let started = Instant::now();
    let resp = immediate(call(&ctx, Method::Post, PATH_PI, &valid_body("第二条", "")).unwrap());
    assert_eq!(status_of(&resp), 409);
    assert_eq!(error_code(&body_of(resp)), "peer_busy");
    assert!(
        started.elapsed() < Duration::from_millis(600),
        "409 必须立即返回，不能被 120s 等待拖住"
    );
    // 第一条超时收口 → 名额释放；第三条可以再抢。
    let first = worker.join().expect("worker 线程");
    assert_eq!(status_of(&first), 504);
    assert!(!ctx.peer_pi.is_busy(), "第一条收口后名额必须释放");
    assert!(matches!(
        call(&ctx, Method::Post, PATH_PI, &valid_body("第三条", "")).unwrap(),
        PeerOutcome::Deferred(_)
    ));
}

#[test]
fn missing_binary_is_404_peer_missing() {
    let tmp = TempRoot::new("missing");
    let root = tmp.path().join("root");
    let ctx = ctx(
        Some(tmp.path().join("no-such-pi")),
        Duration::from_secs(5),
        &root,
    );
    let resp = execute(call(&ctx, Method::Post, PATH_PI, &valid_body("问", "")).unwrap());
    assert_eq!(status_of(&resp), 404);
    assert_eq!(error_code(&body_of(resp)), "peer_missing");
    assert!(!ctx.peer_pi.is_busy(), "失败必须释放名额");
}

#[test]
fn success_is_200_with_assistant_text_only() {
    let tmp = TempRoot::new("success");
    let (program, _marker) = fake_pi_ok(&tmp, "ok");
    let root = tmp.path().join("root");
    let ctx = ctx(Some(program), Duration::from_secs(30), &root);
    let resp = execute(
        call(
            &ctx,
            Method::Post,
            PATH_PI,
            &valid_body("只问一句", "sess-1"),
        )
        .unwrap(),
    );
    assert_eq!(status_of(&resp), 200);
    let has_json_ct = resp.headers().iter().any(|header| {
        header.field.equiv("Content-Type")
            && header.value.as_str() == "application/json; charset=utf-8"
    });
    assert!(has_json_ct, "成功响应必须是 JSON");
    let body = body_of(resp);
    let value: serde_json::Value = serde_json::from_str(&body).expect("200 body 是 JSON");
    assert_eq!(value["text"], "只有这句");
    assert_eq!(
        value.as_object().map(|object| object.len()),
        Some(1),
        "响应体只允许 text 一个字段: {body}"
    );
    assert!(!body.contains("内心"), "思考不得进响应: {body}");
    assert!(!ctx.peer_pi.is_busy(), "成功必须释放名额");
}

#[test]
fn timeout_is_504_and_releases_the_slot() {
    let tmp = TempRoot::new("timeout");
    let program = write_script(tmp.path(), "pi-slow", "sleep 30");
    let root = tmp.path().join("root");
    let ctx = ctx(Some(program), Duration::from_millis(150), &root);
    let started = Instant::now();
    let resp = execute(call(&ctx, Method::Post, PATH_PI, &valid_body("问", "")).unwrap());
    assert_eq!(status_of(&resp), 504);
    assert_eq!(error_code(&body_of(resp)), "peer_timeout");
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "必须按注入的短超时返回"
    );
    assert!(!ctx.peer_pi.is_busy(), "超时必须释放名额");
}

#[test]
fn bad_stdout_is_502_peer_failed() {
    let tmp = TempRoot::new("badstdout");
    let program = write_script(tmp.path(), "pi-bad", "echo 'not the agreed json'");
    let root = tmp.path().join("root");
    let ctx = ctx(Some(program), Duration::from_secs(30), &root);
    let resp = execute(call(&ctx, Method::Post, PATH_PI, &valid_body("问", "")).unwrap());
    assert_eq!(status_of(&resp), 502);
    assert_eq!(error_code(&body_of(resp)), "peer_failed");
    assert!(!ctx.peer_pi.is_busy(), "失败必须释放名额");
}

#[test]
fn missing_session_id_field_defaults_to_ephemeral() {
    let tmp = TempRoot::new("nosession");
    let (program, _marker) = fake_pi_ok(&tmp, "nosess");
    let root = tmp.path().join("root");
    let ctx = ctx(Some(program), Duration::from_secs(30), &root);
    let resp = execute(call(&ctx, Method::Post, PATH_PI, r#"{"text":"只问一句"}"#).unwrap());
    assert_eq!(status_of(&resp), 200);
    let body = body_of(resp);
    let value: serde_json::Value = serde_json::from_str(&body).expect("200 body 是 JSON");
    assert_eq!(value["text"], "只有这句");
    assert!(value.get("error").is_none(), "成功体不得有 error: {body}");
}
