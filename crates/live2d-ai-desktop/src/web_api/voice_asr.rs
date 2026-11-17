//! 语音识别端点（2026-11-16）：`POST /api/v1/voice/asr`。
//!
//! # 做什么
//!
//! 浏览器把**整段**音频（16 位 PCM WAV 或 Ogg Opus）POST 上来，本端点用现有
//! reqwest 转给 DeepSeek `/v1/tools/asr`，把上游的 `{"text", "duration_ms"}`
//! 变成 `{"ok":true,"text":<去首尾空白>,"duration_ms":<可选>}` 回给前端。
//!
//! # 刻意不做（它们随 `voice-input` Mod 一起退休）
//!
//! - **不调用 `supervisor.say`**：识别成功不等于用户要发这句话。文本回到
//!   输入框，由用户自己按发送（回归见 `voice_asr_tests` 的
//!   `asr_route_never_calls_say`）。
//! - 不恢复唤醒词 / sidecar / `run_sidecar` / `backend = mock|sidecar`；
//! - 不搬 `clean_transcript`：只做首尾空白；
//! - 不做流式（上游没有这个能力）。
//!
//! # 安全边界
//!
//! 本 handler 是 `run_request_loop` 里 `flutter_app` 之后、`dispatch` 之前的
//! **静态前置路由**，所以 `dispatch_with_security` 的 mutating 校验**不会**
//! 自动套用；下面每一条都由本函数自己做完：
//!
//! | 判据 | 失败 |
//! | --- | --- |
//! | 方法必须 `POST` | `405 method_not_allowed` |
//! | Content-Type 必须是 `audio/wav` / `audio/ogg` | `415 unsupported_media_type` |
//! | Origin 必须同源 loopback（与 `/api/v1/chat` 同一套） | `403 origin_denied` / `origin_required` |
//! | 正文 ≤ 8 MiB | `413 asr_too_large`（**不打上游**） |
//! | WAV 头能读出的时长 ≤ 120 s | `400 asr_too_long`（**不打上游**） |
//! | `[asr].api_key_env` 有非空值 | `503 asr_unconfigured`（**不打上游**） |
//!
//! 密钥**只**取自 `[asr]` 段自己的变量名——不借用 `[llm]`，免得录音被送到
//! 另一个账号。

use std::io::Read;
use std::time::Duration;

use tiny_http::{Method, Response, StatusCode};

use live2d_ai_runtime::asr::{AudioContainer, MAX_DURATION_MS, MAX_UPLOAD_BYTES, deepseek};
use live2d_ai_runtime::settings::AppSettings;

use crate::web_api::ServerContext;
use crate::web_api::app_routes::json_response;
use crate::web_api::dto::{ErrorDetail, ErrorResponse};

/// 端点路径。
pub const VOICE_ASR_PATH: &str = "/api/v1/voice/asr";

/// 上游请求超时：整段音频 + 一次识别，给足一分钟（超时是**这一句**失败，
/// 不拖住整个 HTTP 循环之外的东西——处理器本身是同步阻塞的）。
const UPSTREAM_TIMEOUT: Duration = Duration::from_secs(60);

/// 路径是否归本模块（`run_request_loop` 用它决定**在 read_body 之前**接手）。
///
/// 必须在读 body 之前判：dispatch 那条通道是 1 MiB + UTF-8 字符串，
/// 而这里是 8 MiB 的二进制。
#[must_use]
pub fn is_asr_path(path: &str) -> bool {
    path == VOICE_ASR_PATH
}

/// 响应类型别名（本文件每处都要写它；与 `env_routes::Resp` 同款）。
pub(crate) type Resp = Response<std::io::Cursor<Vec<u8>>>;

/// 处理一次 ASR 请求（**自行读 body**）。
///
/// 返回 `None` 仅当路径不匹配（交还 `run_request_loop`）。
pub fn handle_voice_asr(
    ctx: &ServerContext,
    method: &Method,
    path: &str,
    body: Vec<u8>,
    origin: Option<&str>,
    content_type: Option<&str>,
) -> Option<Resp> {
    if !is_asr_path(path) {
        return None;
    }
    if *method != Method::Post {
        return Some(error(
            StatusCode(405),
            "method_not_allowed",
            "语音识别只接受 POST（正文是整段音频）",
        ));
    }
    let Some(container) = content_type.and_then(AudioContainer::from_content_type) else {
        return Some(error(
            StatusCode(415),
            "unsupported_media_type",
            "正文必须是 audio/wav（16 位 PCM）或 audio/ogg（Ogg Opus）",
        ));
    };
    if !crate::web_api::security::is_allowed_origin(origin, &ctx.security) {
        let err = match origin {
            Some(o) => crate::web_api::security::MutatingCheckError::BadOrigin {
                got: Some(o.to_string()),
            },
            None => crate::web_api::security::MutatingCheckError::NoOrigin,
        };
        return Some(crate::web_api::security::mutating_check_error_response(
            &err,
        ));
    }
    Some(transcribe(ctx, container, body, deepseek::ASR_URL))
}

/// 从 `tiny_http::Request` 读**二进制** body（最多 [`MAX_UPLOAD_BYTES`] + 1 字节）。
///
/// 多读的那 1 字节是**判超限用的**：读满 `MAX + 1` 就说明超了，不需要把整段
/// 超大音频都收进内存。超过的请求在本机回 413，一个字节都不打上游。
pub fn read_audio_body(request: &mut tiny_http::Request) -> Vec<u8> {
    let mut buf = Vec::with_capacity(4096);
    let mut limited = request.as_reader().take((MAX_UPLOAD_BYTES + 1) as u64);
    let _ = limited.read_to_end(&mut buf);
    buf
}

/// 核心：本机判据 → 转发 → 映射响应。
///
/// `endpoint` 单独成参：生产恒为 [`deepseek::ASR_URL`]，单测指到本机假上游。
fn transcribe(
    ctx: &ServerContext,
    container: AudioContainer,
    body: Vec<u8>,
    endpoint: &str,
) -> Resp {
    // ① 大小：本机就拒，不打上游。
    if body.len() > MAX_UPLOAD_BYTES {
        return error(
            StatusCode(413),
            "asr_too_large",
            "音频超过 8 MiB 上限：本机已拒收，未发送上游",
        );
    }
    if body.is_empty() {
        return error(
            StatusCode(400),
            "asr_empty_audio",
            "正文为空：请上传整段 16 位 PCM WAV 或 Ogg Opus",
        );
    }
    // ② 时长：只有 WAV 头能读出时长时才判（Ogg 不做容器解析门禁，
    //    超长由上游拒，错误码照原样映射）。
    if container == AudioContainer::Wav
        && let Some(ms) = live2d_ai_runtime::asr::wav_duration_ms(&body)
        && ms > MAX_DURATION_MS
    {
        return error(
            StatusCode(400),
            "asr_too_long",
            "音频超过 120 秒上限：本机已拒收，未发送上游",
        );
    }
    // ③ 钥匙：只认 [asr] 段自己的变量名。
    let settings = ctx.status_ctx.settings_snapshot();
    let Some(key) = asr_key(&settings) else {
        return error(
            StatusCode(503),
            "asr_unconfigured",
            "语音识别未配置：[asr].api_key_env 未声明或对应环境变量为空（不借用 [llm] 的钥匙）",
        );
    };

    // ④ 转发。
    let upload = deepseek::AsrUpload {
        bytes: &body,
        container,
    };
    let result = run_blocking(endpoint, &key, &upload);
    match result {
        Ok(parsed) => json_response(StatusCode(200), &deepseek::AsrOkBody::from(parsed)),
        Err(e) => upstream_error(&e),
    }
}

/// 在**本线程**起一个 current_thread runtime 跑一次上游请求。
///
/// HTTP 循环是同步阻塞的，这里不开新线程池（与 `settings/test/*` 同一条做法）。
fn run_blocking(
    endpoint: &str,
    key: &str,
    upload: &deepseek::AsrUpload<'_>,
) -> live2d_ai_runtime::Result<deepseek::AsrResponse> {
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            return Err(live2d_ai_runtime::Error::InvalidAudioConfig {
                message: format!("无法建立 HTTP runtime：{e}"),
            });
        }
    };
    let http = match reqwest::Client::builder().timeout(UPSTREAM_TIMEOUT).build() {
        Ok(c) => c,
        Err(e) => return Err(live2d_ai_runtime::Error::Http(e)),
    };
    rt.block_on(deepseek::transcribe(&http, key, upload, endpoint))
}

/// 上游失败 → 可读错误（**不把响应体里的密钥写进日志**）。
fn upstream_error(err: &live2d_ai_runtime::Error) -> Resp {
    let code = match err.upstream_status() {
        Some(status) => deepseek::error_code_for(status),
        None => "asr_upstream_unreachable".to_string(),
    };
    let status = match err.upstream_status() {
        // 401/403 是「钥匙不对」，对客户端如实回 502 之外的同一个码即可；
        // 这里统一 502 + 稳定 code（前端按 code 分支，不看 HTTP 状态码猜）。
        Some(_) => StatusCode(502),
        None => StatusCode(502),
    };
    // 日志只记 code + 上游状态，不记 body（body 可能回显 Authorization）。
    tracing::warn!(
        target: "asr",
        code = %code,
        upstream_status = err.upstream_status().unwrap_or(0),
        "语音识别上游失败"
    );
    let message = match err.upstream_status() {
        Some(s) => format!("上游语音识别失败（HTTP {s}）；错误码 {code}"),
        None => format!("上游语音识别不可达；错误码 {code}"),
    };
    json_response(
        status,
        &ErrorResponse::new(ErrorDetail::with_details(
            // code 是 String（含状态码），需要 'static —— 用 details 携带真码，
            // 顶层 code 用稳定的族名，前端两处都能拿到。
            "asr_upstream_error",
            message,
            serde_json::json!({"code": code}),
        )),
    )
}

/// 构造 JSON 错误响应（`code` 是 `&'static str`）。
fn error(status: StatusCode, code: &'static str, message: &'static str) -> Resp {
    json_response(status, &ErrorResponse::new(ErrorDetail::new(code, message)))
}

/// 读 `[asr]` 段声明的钥匙（`.env` 快照 > 进程环境，与其它段同一条真源）。
///
/// **不借用 `[llm]`**：变量名只从 `settings.asr` 取。
#[must_use]
pub fn asr_key(settings: &AppSettings) -> Option<String> {
    let name = settings.asr.api_key_env.as_deref()?;
    if name.is_empty() {
        return None;
    }
    live2d_ai_runtime::secrets::lookup(name).filter(|v| !v.is_empty())
}

// 回归测试：见同目录 `voice_asr_tests.rs`（保持源文件 <500 行）。
#[cfg(test)]
#[path = "voice_asr_tests.rs"]
mod tests;
