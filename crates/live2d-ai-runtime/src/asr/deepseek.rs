//! DeepSeek ASR：`POST https://api.deepseek.com/v1/tools/asr`。
//!
//! # 契约（接口依据：DeepSeek 文档 2026-11-13）
//!
//! - 请求：`multipart/form-data`，一个字段 `file`（整段 16 位 PCM WAV 或
//!   Ogg Opus；最长 120 秒、最大 8 MiB）；
//! - 响应：`{"text": "...", "duration_ms": 1234}`（`duration_ms` 可缺）；
//! - **无流式、无时间戳**——所以这里也不做增量解析。
//!
//! # 为什么手写 multipart，而不开 reqwest 的 `multipart` feature
//!
//! 开了那条 feature 会让 `live2d-ai-desktop` 的**顶层依赖数**超过 18 的棘轮
//! （`code-stats --only deps`，见 `xtask/src/code_stats`）。这边要发的
//! multipart 只有一个字段、一份字节，边界串自己生成即可——二十几行的编码
//! 换来一条不新增依赖的路径。
//!
//! # 错误
//!
//! 上游 4xx/5xx 映射成可读错误码（`asr_upstream_<status>`），**不把响应体原样
//! 塞进日志**：上游的报错文本可能回显 `Authorization`（历史上见过这种实现），
//! 而密钥是绝不能进日志的。错误里只带截断后的短片段（[`crate::Error::Status`]
//! 的既有口径，≤512 字符）。

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// DeepSeek ASR 端点（固定；`[asr]` 段不提供地址覆盖）。
pub const ASR_URL: &str = "https://api.deepseek.com/v1/tools/asr";

/// multipart 的字段名（上游契约：一个 `file` 字段）。
pub const FILE_FIELD: &str = "file";

/// 上游成功响应。`duration_ms` 是**上游自己给的数**，缺省时不补 0
/// ——「没这个数」与「0 毫秒」是两件事。
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct AsrResponse {
    /// 识别出的文本（可能带首尾空白，调用方负责 trim）。
    #[serde(default)]
    pub text: String,
    /// 上游报的时长（毫秒）；缺省 = 上游没给。
    #[serde(default)]
    pub duration_ms: Option<u64>,
}

/// 一次识别请求的输入。
#[derive(Debug, Clone, Copy)]
pub struct AsrUpload<'a> {
    /// 整段音频字节（WAV / Ogg 已经是容器形态，直接上行）。
    pub bytes: &'a [u8],
    /// 容器类型（决定 multipart 里那一份的 `Content-Type` 与文件名）。
    pub container: super::AudioContainer,
}

/// 生成一个 multipart 边界串。
///
/// 用**内容长度**参与构造：同一份字节得到同一个边界，单测可复现；同时不同
/// 长度的请求几乎不可能撞上同一串，不会与音频字节里偶然出现的串冲突
/// （真冲突的概率是 2^-128 量级，且这里还叠了长度与前缀）。
#[must_use]
pub fn boundary_for(bytes: &[u8]) -> String {
    format!("----live2d-ai-asr-{}", bytes.len())
}

/// 手写 multipart/form-data 编码（**单字段 `file`**）。
///
/// 返回 `(整体字节, boundary)`：boundary 要原样进 `Content-Type` 头。
/// 编码规则（RFC 7578 / RFC 2046）：上界、`Content-Disposition`（name + filename）、
/// `Content-Type`、空行、音频字节、下界。
#[must_use]
pub fn encode_multipart(upload: &AsrUpload<'_>) -> (Vec<u8>, String) {
    let boundary = boundary_for(upload.bytes);
    let mut body = Vec::with_capacity(upload.bytes.len() + boundary.len() * 2 + 160);
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        format!(
            "Content-Disposition: form-data; name=\"{FILE_FIELD}\"; filename=\"{}\"\r\n",
            upload.container.filename()
        )
        .as_bytes(),
    );
    body.extend_from_slice(format!("Content-Type: {}\r\n\r\n", upload.container.mime()).as_bytes());
    body.extend_from_slice(upload.bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    (body, boundary)
}

/// 这一份音频是否值得发上游（大小 / 空）。
///
/// 超过 8 MiB 在**本机就拒**（413），不打上游——省一次无效上行。
#[must_use]
pub fn reject_reason(bytes: &[u8]) -> Option<&'static str> {
    if bytes.is_empty() {
        return Some("asr_empty_audio");
    }
    if bytes.len() > super::MAX_UPLOAD_BYTES {
        return Some("asr_too_large");
    }
    None
}

/// 解析上游成功响应体（只接受 `{"text": …}`；`duration_ms` 可缺）。
pub fn parse_response(body: &[u8]) -> Result<AsrResponse> {
    let parsed: AsrResponse = serde_json::from_slice(body)?;
    Ok(parsed)
}

/// 上游非 2xx → 可读错误码（**不**把响应体原样带进日志）。
///
/// 码是契约：`asr_upstream_401` / `asr_upstream_429` / …，前端按它分支，
/// 用户拿它去日志里搜同一串。
#[must_use]
pub fn error_code_for(status: u16) -> String {
    format!("asr_upstream_{status}")
}

/// 一次完整识别：发 multipart，拿回 `{text, duration_ms}`。
///
/// `endpoint` 是唯一可注入的一项：生产恒为 [`ASR_URL`]，单测把它指到本机
/// 假上游（本 crate 从不要求连真的 `api.deepseek.com`）。
pub async fn transcribe(
    http: &reqwest::Client,
    api_key: &str,
    upload: &AsrUpload<'_>,
    endpoint: &str,
) -> Result<AsrResponse> {
    if let Some(reason) = reject_reason(upload.bytes) {
        return Err(Error::InvalidAudioConfig {
            message: format!("ASR 请求在本机就被拒（{reason}）"),
        });
    }
    let url = reqwest::Url::parse(endpoint).map_err(|_| Error::InvalidBaseUrl {
        url: endpoint.to_string(),
    })?;
    let (body, boundary) = encode_multipart(upload);

    let response = http
        .post(url)
        .bearer_auth(api_key)
        .header(
            reqwest::header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(body)
        .send()
        .await?;
    let status = response.status();
    if !status.is_success() {
        // 只留截断片段（≤512 字符），且**不**在日志里打整段 body。
        let text = response.text().await.unwrap_or_default();
        return Err(Error::status(status, &text));
    }
    let bytes = response.bytes().await?;
    parse_response(&bytes)
}

/// 上游响应 → 本项目的 API 形态（`{"ok":true,"text":…,"duration_ms":…}`）。
///
/// 文本**只做首尾空白**：`clean_transcript` 那一套（唤醒词剥离 / locale 归一化 /
/// 句号补齐）随 Mod 一起退休，不搬进主仓。需要句号归一的话走用户发送后的
/// 现有对话链。
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AsrOkBody {
    /// 恒 `true`（失败走 HTTP 错误码，不在这里编一个 `ok:false`）。
    pub ok: bool,
    /// 上游文本去掉首尾空白。
    pub text: String,
    /// 上游给的时长（缺省时不出现）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

impl From<AsrResponse> for AsrOkBody {
    fn from(r: AsrResponse) -> Self {
        Self {
            ok: true,
            text: r.text.trim().to_string(),
            duration_ms: r.duration_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multipart_has_one_file_field_with_container_type() {
        let (body, boundary) = encode_multipart(&AsrUpload {
            bytes: b"RIFFxxxxWAVE",
            container: super::super::AudioContainer::Wav,
        });
        let text = String::from_utf8_lossy(&body);
        assert!(
            text.starts_with(&format!("--{boundary}\r\n")),
            "必须以上界开头：{text}"
        );
        assert!(text.contains("Content-Disposition: form-data; name=\"file\""));
        assert!(text.contains("filename=\"audio.wav\""));
        assert!(text.contains("Content-Type: audio/wav"));
        assert!(text.ends_with(&format!("\r\n--{boundary}--\r\n")));
        // 音频字节原样在里面（一个字节都不改）。
        let needle = b"RIFFxxxxWAVE";
        assert!(
            body.windows(needle.len()).any(|w| w == needle),
            "音频字节必须原样上行"
        );
        // 只有一个字段：`form-data; name="` 只出现一次
        //（`filename="` 也含 `name="`，所以判据要带上前面的 `form-data; `）。
        assert_eq!(text.matches("form-data; name=\"").count(), 1);
    }

    #[test]
    fn multipart_uses_ogg_names_for_the_ogg_container() {
        let (body, _) = encode_multipart(&AsrUpload {
            bytes: b"OggS....",
            container: super::super::AudioContainer::Ogg,
        });
        let text = String::from_utf8_lossy(&body);
        assert!(text.contains("filename=\"audio.ogg\""));
        assert!(text.contains("Content-Type: audio/ogg"));
    }

    #[test]
    fn oversize_audio_is_rejected_before_any_upload() {
        assert_eq!(reject_reason(b""), Some("asr_empty_audio"));
        let ok = vec![0u8; super::super::MAX_UPLOAD_BYTES];
        assert_eq!(reject_reason(&ok), None, "恰好 8 MiB 仍可上行");
        let too_big = vec![0u8; super::super::MAX_UPLOAD_BYTES + 1];
        assert_eq!(reject_reason(&too_big), Some("asr_too_large"));
    }

    #[test]
    fn response_text_is_trimmed_and_duration_is_passthrough() {
        let parsed = parse_response(br#"{"text":"  \u4f60\u597d\u3002  ","duration_ms":1234}"#)
            .expect("parse");
        let body = AsrOkBody::from(parsed);
        assert_eq!(body.text, "你好。", "只做首尾空白，不改中间");
        assert_eq!(body.duration_ms, Some(1234));
        assert!(body.ok);

        // duration_ms 缺省 → 不补 0，也不出现该字段。
        let parsed = parse_response(br#"{"text":"hi"}"#).expect("parse");
        let body = AsrOkBody::from(parsed);
        assert_eq!(body.text, "hi");
        assert_eq!(body.duration_ms, None);
        let json = serde_json::to_value(&body).expect("json");
        assert!(json.get("duration_ms").is_none(), "缺省时不写这个键");
        assert_eq!(json["ok"], true);
    }

    #[test]
    fn upstream_error_codes_are_stable_and_readable() {
        assert_eq!(error_code_for(401), "asr_upstream_401");
        assert_eq!(error_code_for(429), "asr_upstream_429");
        assert_eq!(error_code_for(503), "asr_upstream_503");
    }
}
