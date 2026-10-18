//! LLM 云 / 本地（`[llm].mode`，2026-10-18 0.3.7 T1）的端点级回归。
//!
//! 本文件守两类**行为**（不是实现细节）：
//!
//! 1. **连通性自检只探连通**：`POST /api/v1/settings/test/llm` 内部只发
//!    `GET {base_url}/models`，**不跑一次对话**（用一次性 HTTP 服务看真实请求行）；
//!    分类：404 = 可达 + note、401/403 = `auth_failed`、5xx = `protocol_error`、
//!    网络不可达：local = `llm_local_unreachable`、cloud = `network_failed`；
//! 2. **PATCH 落盘**：未知 mode → 400 `invalid_payload` 且文件不动；
//!    合法 mode → 写进 `[llm]`、GET 视图回同一个稳定字符串。
//!
//! 单列文件：`tests.rs` 已经 774 行（测试文件 ≤800 行纪律）。

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use tiny_http::Response;

use live2d_ai_runtime::settings::LlmMode;

use super::tests::{no_keys, sample};
use super::*;

/// 只处理**一次**请求的 HTTP 探针服务。
///
/// 返回 `(base_url, 收到的请求行, join handle)`：请求行让「只 GET /models、
/// 不 POST /chat/completions」成为**可失败断言**，而不是读代码猜。
fn serve_once(
    status: u16,
    response_body: &'static str,
) -> (String, Arc<Mutex<Vec<String>>>, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind 探针端口");
    let addr = listener.local_addr().expect("local_addr");
    let seen = Arc::new(Mutex::new(Vec::<String>::new()));
    let seen_in_thread = Arc::clone(&seen);
    let handle = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 8192];
            let n = stream.read(&mut buf).unwrap_or(0);
            let request = String::from_utf8_lossy(&buf[..n]).to_string();
            if let Some(line) = request.lines().next() {
                seen_in_thread
                    .lock()
                    .expect("seen lock")
                    .push(line.to_string());
            }
            let head = format!(
                "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                response_body.len()
            );
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(response_body.as_bytes());
            let _ = stream.flush();
        }
    });
    (format!("http://{addr}/v1"), seen, handle)
}

/// 本地模式的样本（无密钥；地址 = 传进来的探针地址）。
fn local_settings(base_url: &str) -> AppSettings {
    let mut s = sample();
    s.llm.mode = LlmMode::Local;
    s.llm.base_url = base_url.to_string();
    s.llm.api_key_env = None;
    s
}

fn cloud_settings(base_url: &str) -> AppSettings {
    let mut s = local_settings(base_url);
    s.llm.mode = LlmMode::Cloud;
    s
}

fn failure_code(out: &TestOutcome) -> &'static str {
    out.error.as_ref().map(|e| e.code).unwrap_or("<无错误详情>")
}

fn read_body(resp: Response<std::io::Cursor<Vec<u8>>>) -> String {
    let mut s = String::new();
    let _ = resp.into_reader().read_to_string(&mut s);
    s
}

fn temp_path(name: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static C: AtomicU64 = AtomicU64::new(0);
    let mut p = std::env::temp_dir();
    p.push(format!(
        "live2d_ai_test_llm_mode_{name}_{}_{}.toml",
        std::process::id(),
        C.fetch_add(1, Ordering::SeqCst)
    ));
    p
}

// ───────────────────────── 自检：只探 /models，不跑对话

#[test]
fn llm_self_check_probes_models_and_never_runs_a_chat() {
    let (base, seen, handle) = serve_once(200, r#"{"data":[]}"#);
    let out = run_llm_test(&local_settings(&base), Some(2_000));
    handle.join().expect("探针线程");

    assert!(out.ok, "2xx 必须通过：{out:?}");
    assert_eq!(out.model_echo.as_deref(), Some("qwen2.5:7b"));
    let lines = seen.lock().expect("seen").clone();
    assert_eq!(
        lines,
        vec!["GET /v1/models HTTP/1.1".to_string()],
        "自检必须只打 GET /models（不真跑一次对话）"
    );
    assert!(
        lines.iter().all(|l| !l.contains("chat/completions")),
        "不得向 /chat/completions 发任何请求：{lines:?}"
    );
}

/// **404 算可达**（附 note）：上游回话了就证明「可达」；`/models` 不是所有
/// OpenAI-compatible 实现都提供。判失败 = 又造一个「自检说没通、实际能用」。
#[test]
fn llm_self_check_404_counts_as_reachable_with_note() {
    let (base, _seen, handle) = serve_once(404, "not found");
    let out = run_llm_test(&cloud_settings(&base), Some(2_000));
    handle.join().expect("探针线程");

    assert!(out.ok, "404 必须算可达：{out:?}");
    assert!(
        out.note.as_deref().is_some_and(|n| n.contains("可达")),
        "必须说明为什么算通过：{out:?}"
    );
}

#[test]
fn llm_self_check_401_and_403_are_auth_failed() {
    for status in [401u16, 403] {
        let (base, _seen, handle) = serve_once(status, "no key");
        let out = run_llm_test(&cloud_settings(&base), Some(2_000));
        handle.join().expect("探针线程");
        assert!(!out.ok);
        assert_eq!(
            failure_code(&out),
            "auth_failed",
            "status {status} 必须归为鉴权失败：{out:?}"
        );
    }
}

/// 其余 4xx/5xx：**可达但坏了** → `protocol_error`，带上响应片段。
#[test]
fn llm_self_check_5xx_is_protocol_error_with_detail() {
    let (base, _seen, handle) = serve_once(500, "model crashed");
    let out = run_llm_test(&cloud_settings(&base), Some(2_000));
    handle.join().expect("探针线程");
    assert!(!out.ok);
    assert_eq!(failure_code(&out), "protocol_error", "{out:?}");
    assert!(
        out.error
            .as_ref()
            .is_some_and(|e| e.message.contains("model crashed")),
        "错误体要带响应片段：{out:?}"
    );
}

// ───────────────────────── 不可达：本地明确状态、不自动回退

/// **没启动的本地端点**：`llm_local_unreachable`（明确状态，不得显示 ready）。
#[test]
fn llm_self_check_local_unreachable_is_explicit() {
    // 先绑一个端口拿号，再释放：连接会被拒（稳定、不依赖外部服务）。
    let dead_addr = {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        listener.local_addr().expect("addr")
    };
    let base = format!("http://{dead_addr}/v1");
    let settings = local_settings(&base);
    let before = settings.clone();

    let out = run_llm_test(&settings, Some(500));
    assert!(!out.ok, "连不上时不得显示 ready：{out:?}");
    assert_eq!(failure_code(&out), "llm_local_unreachable", "{out:?}");
    let message = out.error.as_ref().map(|e| e.message.as_str()).unwrap_or("");
    assert!(
        message.contains("本地"),
        "文案要指出这是本地端点：{message}"
    );
    assert!(
        message.contains("不会切回云端"),
        "必须说明失败不自动回退 cloud：{message}"
    );

    // **不自动回退**的结构证据：配置一个字没动（仍是 Local + 同一个地址）。
    assert_eq!(settings, before, "自检不得改写配置、不得回退模式");
    assert_eq!(settings.llm.mode, LlmMode::Local);
}

/// 云端沿用既有分类：连不上 = `network_failed`（云端的 401 报错路径不受影响）。
#[test]
fn llm_self_check_cloud_unreachable_keeps_network_failed() {
    let dead_addr = {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        listener.local_addr().expect("addr")
    };
    let base = format!("http://{dead_addr}/v1");
    let out = run_llm_test(&cloud_settings(&base), Some(500));
    assert!(!out.ok);
    assert_eq!(failure_code(&out), "network_failed", "{out:?}");
}

/// **没配置**（空 base_url）：local 必须给 `llm_local_unreachable`（不是 ready，
/// 也不是笼统的 `llm_unreachable`）；cloud 保持旧码 `llm_unreachable`。
#[test]
fn llm_self_check_unconfigured_local_is_explicit() {
    let mut local = local_settings("");
    local.llm.base_url.clear();
    let out = run_llm_test(&local, Some(100));
    assert!(!out.ok);
    assert_eq!(failure_code(&out), "llm_local_unreachable", "{out:?}");

    let mut cloud = cloud_settings("");
    cloud.llm.base_url.clear();
    let out = run_llm_test(&cloud, Some(100));
    assert!(!out.ok);
    assert_eq!(
        failure_code(&out),
        "llm_unreachable",
        "云端旧码不变：{out:?}"
    );
}

// ───────────────────────── PATCH：未知 mode 拒绝，合法 mode 落盘

/// 未知 mode → 400 `invalid_payload`，且**文件一个字没动**。
#[test]
fn patch_unknown_llm_mode_is_rejected_without_writing() {
    let tmp = temp_path("unknown");
    std::fs::write(&tmp, sample().to_toml_string()).expect("写初始配置");
    let before = std::fs::read_to_string(&tmp).expect("读初始配置");
    let current = AppSettings::load_from_path(&tmp).expect("load");

    let resp = handle_patch(
        &current,
        r#"{"llm":{"mode":"bogus"}}"#,
        &tmp.to_string_lossy(),
        None,
        &no_keys,
        "",
    );
    assert_eq!(resp.status_code().0, 400, "未知 mode 必须 400");
    let body = read_body(resp);
    assert!(body.contains("invalid_payload"), "{body}");
    assert_eq!(
        std::fs::read_to_string(&tmp).expect("读回配置"),
        before,
        "400 不得写盘"
    );
    let _ = std::fs::remove_file(&tmp);
}

/// 合法 mode → 200 + persisted，写进 `[llm]`，GET 视图回稳定字符串；
/// 其它字段原样保留（模式只是标签）。
#[test]
fn patch_llm_mode_local_persists_and_view_echoes_mode() {
    let tmp = temp_path("persist");
    std::fs::write(&tmp, sample().to_toml_string()).expect("写初始配置");
    let current = AppSettings::load_from_path(&tmp).expect("load");
    let before = current.clone();

    let resp = handle_patch(
        &current,
        r#"{"llm":{"mode":"local"}}"#,
        &tmp.to_string_lossy(),
        None,
        &no_keys,
        "",
    );
    assert_eq!(resp.status_code().0, 200);
    let body = read_body(resp);
    assert!(body.contains(r#""persisted":true"#), "{body}");
    assert!(
        body.contains(r#""mode":"local""#),
        "PATCH 视图要回 mode：{body}"
    );

    let after = AppSettings::load_from_path(&tmp).expect("reload");
    assert_eq!(after.llm.mode, LlmMode::Local);
    assert_eq!(after.llm.base_url, before.llm.base_url, "地址不被模式改写");
    assert_eq!(after.llm.model, before.llm.model);
    assert_eq!(after.llm.api_key_env, before.llm.api_key_env);
    assert_eq!(
        settings_to_view_with_keys_and_model(&after, &no_keys, "")
            .llm
            .mode,
        "local",
        "GET 视图与落盘同口径"
    );
    let _ = std::fs::remove_file(&tmp);
}
