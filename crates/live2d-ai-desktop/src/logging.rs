//! 日志初始化（W3 任务：CLI 启动附带 — 任何模式都落盘，不只开发者模式）。
//!
//! # 关键不变量
//! - **D1 契约 §P0-1**：日志不得含 `api_key` 明文。脱敏**双保险**：
//!   1. **主动调用**：[`redact`] 是公开纯函数；调用方在打日志前可用
//!      `tracing::info!(key = logging::redact(&secret), "...")` 主动脱敏；
//!   2. **文件层兜底**：[`SanitizingWriter`] 包装 [`tracing_appender::rolling::RollingFileAppender`]，
//!      任何 `key=…` / `api_key=…` / `token=…` / `secret=…` 形式
//!      `field=value` / `field: value` 都会在写入前把 value 替换为 `[REDACTED]`
//!      （仅 file layer；stdout 不重写，避免控制台延迟）。
//! - **降级不致命**：[`init_logging`] 失败仅返回 `Err`；`main.rs` 接到
//!   `Err` 时打 warn 后继续（仅 stdout），**不**退出码 1（日志落盘
//!   不应阻止应用启动）。
//!
//! # 双 layer 装配
//! - **stdout layer**：`tracing_subscriber::fmt::layer().with_writer(std::io::stdout)`，
//!   保留现有 `EnvFilter`（`RUST_LOG` 优先，缺省 `info`）。
//! - **file layer**：`tracing_subscriber::fmt::layer().with_writer(SanitizingWriter::new(...))`，
//!   单独用 file 专用 filter（默认 `info`；`LIVE2D_AI_LOG_FILE` 可调）。
//!
//! # 滚动策略
//! - 选 `tracing_appender::rolling::daily(log_dir, "live2d-ai.log")`：
//!   - 自动 `live2d-ai.log.YYYY-MM-DD` 后缀；
//!   - 进程内单 writer 锁，安全；
//!   - 与 `docs/architecture/observability.md` §3 「按天滚动」语义一致（PC Python
//!     版是 10MB 5 份，Rust v0 选 daily 简单可靠；按大小可在后续批次升级）。
//!
//! # 日志目录约定（D0 / `docs/architecture/directory.md`）
//! - `log_dir` = `None`：走 [`default_log_dir`] — Linux `$XDG_DATA_HOME/live2d-ai/logs/`
//!   （无 `XDG_DATA_HOME` 时回退 `$HOME/.local/share/live2d-ai/logs/`）；
//! - 写盘失败（含目录创建失败）→ [`init_logging`] 返回 `Err`，调用方降级仅 stdout。

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

/// 全局非阻塞 file writer 守卫：drop 时 flush 剩余日志。
///
/// `static` 持有——保证进程退出前不丢日志。
static FILE_GUARD: once_cell::sync::Lazy<Mutex<Option<WorkerGuard>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(None));

/// 当前激活的日志目录（用于 `/api/v1/logs` 端点读取最近 N 行）。
///
/// 暴露为 `pub`，让 web 模块可以读取。
pub static LOG_DIR: once_cell::sync::Lazy<Mutex<Option<PathBuf>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(None));

/// 启动日志双 sink（stdout + file）。
///
/// 参数：
/// - `log_dir`：自定义日志目录；传 `None` 走 [`default_log_dir`]。
///
/// 失败语义：写盘失败（目录创建失败 / appender 构造失败）→ `Err(msg)`，
/// `main.rs` 应仅 warn 而**不**退出（保证日志问题不阻塞启动）。
pub fn init_logging(log_dir: Option<&Path>) -> Result<(), String> {
    // 1. 计算日志目录；写盘失败 → Err。
    let dir = match log_dir {
        Some(p) => p.to_path_buf(),
        None => default_log_dir().map_err(|e| format!("解析默认日志目录失败: {e}"))?,
    };
    fs::create_dir_all(&dir).map_err(|e| format!("创建日志目录 {} 失败: {e}", dir.display()))?;

    // 2. file appender：`daily` 自动按天滚动。
    let file_appender = tracing_appender::rolling::daily(&dir, "live2d-ai.log");
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);
    let sanitizing = SanitizingMakeWriter::new(Arc::new(Mutex::new(file_writer)));

    // 3. 双 EnvFilter：
    //    - stdout 走 `RUST_LOG`（缺省 info）—— 保留现有语义；
    //    - file 走 `LIVE2D_AI_LOG_FILE`（缺省 info）—— 独立可调，
    //      默认至少 info 落盘（排障可改 debug）。
    let stdout_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let file_filter =
        EnvFilter::try_from_env("LIVE2D_AI_LOG_FILE").unwrap_or_else(|_| EnvFilter::new("info"));

    // 4. 装配双 layer；`boxed()` 抹平类型差。
    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_writer(io::stdout)
        .with_filter(stdout_filter)
        .boxed();
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(sanitizing)
        .with_ansi(false)
        .with_target(true)
        .with_filter(file_filter)
        .boxed();

    tracing_subscriber::registry()
        .with(stdout_layer)
        .with(file_layer)
        .try_init()
        .map_err(|e| format!("设置全局 subscriber 失败: {e}"))?;

    // 5. 记录 guard 与 log_dir（供 web 端点读取）。
    if let Ok(mut g) = FILE_GUARD.lock() {
        *g = Some(guard);
    }
    if let Ok(mut d) = LOG_DIR.lock() {
        *d = Some(dir.clone());
    }

    tracing::info!(log_dir = %dir.display(), "logging: 已启动（stdout + file 双 sink）");
    Ok(())
}

/// 返回当前激活的日志目录（None = 未初始化）。
pub fn current_log_dir() -> Option<PathBuf> {
    LOG_DIR.lock().ok().and_then(|g| g.clone())
}

/// 解析默认日志目录：Linux `$XDG_DATA_HOME/live2d-ai/logs/` →
/// 回退 `$HOME/.local/share/live2d-ai/logs/` → 最后 `logs/`（cwd 相对，
/// 兜底，让首次启动至少能写盘）。
fn default_log_dir() -> io::Result<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
        let mut p = PathBuf::from(xdg);
        p.push("live2d-ai");
        p.push("logs");
        return Ok(p);
    }
    if let Some(home) = std::env::var_os("HOME") {
        let mut p = PathBuf::from(home);
        p.push(".local");
        p.push("share");
        p.push("live2d-ai");
        p.push("logs");
        return Ok(p);
    }
    // 最后兜底：cwd 相对 `logs/`，与 `directory.md` 「根级日志放 logs/」一致。
    Ok(PathBuf::from("logs"))
}

/// 公开的脱敏纯函数：把字符串中任何 `key=...` / `key: ...` / `"key": "..."` 形式
/// 的 value 替换为 `[REDACTED]`（大小写不敏感）。
///
/// 规则：
/// - 字段名 `api_key` / `apikey` / `password` / `token` / `auth` 命中 →
///   替换该字段后的 value（直到下个 `,` / `}` / `"` / 空格 / `\n` / 终止符）。
/// - 故意不包含通用短词 `key` / `secret`：作为子串命中率太高（`monkey=`
///   `secret-thing=` 等），易误伤；日志里若真出现裸 `key=...`，由调用方
///   在打日志前用 `logging::redact(&value)` 显式脱敏。
/// - 支持：
///   - `key=value`（tracing 默认 field 格式）
///   - `key: value`（带空格的 YAML/JSON 友好）
///   - `"key": "value"`（JSON，含外侧 `"`）
///
/// 主要给调用方在打日志前用（主动脱敏）；文件层兜底见 [`SanitizingWriter`]。
pub fn redact(s: &str) -> String {
    if s.is_empty() {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    let sensitive = |name: &str| {
        let lower = name.to_ascii_lowercase();
        matches!(
            lower.as_str(),
            "api_key" | "apikey" | "password" | "token" | "auth"
        )
    };
    let is_kv_terminator = |b: u8| b == b',' || b == b'}' || b == b' ' || b == b'\n' || b == b'\t';
    while i < bytes.len() {
        // 找 key 起点：跳过前导 `"`、空白。
        let start = i;
        let mut j = i;
        // 允许前导 `"` (JSON 风格)。
        if j < bytes.len() && bytes[j] == b'"' {
            j += 1;
        }
        let key_begin = j;
        while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
            j += 1;
        }
        // 真实 key 末尾 = j（j 现在指向 `"` / `=` / `:` / 其它）。
        let key_end = j;
        // 跳过后置 `"`（JSON `"key"` 闭合），寻找 `=` 或 `:`。
        if j > key_begin && j < bytes.len() && bytes[j] == b'"' {
            j += 1;
        }
        if j > key_begin && j < bytes.len() && (bytes[j] == b'=' || bytes[j] == b':') {
            let key = std::str::from_utf8(&bytes[key_begin..key_end]).unwrap_or("");
            // 跳过 key 后的引号 / 空格（任意顺序：JSON 风格是 `" "`、YAML 风格是 ` `）。
            let mut k = j + 1;
            while k < bytes.len() && (bytes[k] == b' ' || bytes[k] == b'\t' || bytes[k] == b'"') {
                k += 1;
            }
            // 抄到 key + 分隔符 + 可能的引号 + 空格。
            out.push_str(&s[start..k]);
            if sensitive(key) {
                out.push_str("[REDACTED]");
                // 跳过原 value。`k` 已经吞掉前导引号（如有），需
                // 检测 value 是否带外侧引号来决定终止条件。
                let mut v = k;
                let value_starts_with_quote = start < k && k > 0 && bytes[k - 1] == b'"';
                if value_starts_with_quote {
                    // 闭合引号：写到输出再跳过。
                    while v < bytes.len()
                        && bytes[v] != b'"'
                        && bytes[v] != b','
                        && bytes[v] != b'}'
                    {
                        v += 1;
                    }
                    if v < bytes.len() && bytes[v] == b'"' {
                        out.push('"');
                        v += 1;
                    }
                } else {
                    // 无引号 value：到下个 `,` / `}` / 空格 / `\n` 之前。
                    while v < bytes.len() && !is_kv_terminator(bytes[v]) {
                        v += 1;
                    }
                }
                i = v;
                continue;
            }
            i = k;
            continue;
        }
        // 普通字符：原样抄。
        let ch_end = if bytes[i].is_ascii() {
            i + 1
        } else {
            // UTF-8 多字节：抄到下一 ASCII 边界。
            let mut e = i;
            while e < bytes.len() && !bytes[e].is_ascii() {
                e += 1;
            }
            e
        };
        out.push_str(&s[i..ch_end]);
        i = ch_end;
    }
    out
}

/// 写包装：在写入前调用 [`redact`]，兜底任何调用方未主动脱敏的字段。
///
/// 内部 `Arc<Mutex<W>>` 跨 layer 共享；非阻塞 `non_blocking` writer
/// 自身线程安全。
pub struct SanitizingWriter {
    inner: Arc<Mutex<dyn Write + Send>>,
}

impl SanitizingWriter {
    pub fn new(inner: Arc<Mutex<dyn Write + Send>>) -> Self {
        Self { inner }
    }
}

/// `MakeWriter` 工厂：每次 `make_writer` 都克隆 `Arc` 得到独立
/// `SanitizingWriter`（`tracing-subscriber` 要求 `for<'w> MakeWriter<'w>`）。
pub struct SanitizingMakeWriter {
    inner: Arc<Mutex<dyn Write + Send>>,
}

impl SanitizingMakeWriter {
    pub fn new(inner: Arc<Mutex<dyn Write + Send>>) -> Self {
        Self { inner }
    }
}

impl<'a> MakeWriter<'a> for SanitizingMakeWriter {
    type Writer = SanitizingWriter;
    fn make_writer(&'a self) -> Self::Writer {
        SanitizingWriter::new(Arc::clone(&self.inner))
    }
}

impl Write for SanitizingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // 一次性复制避免持锁时再分配。
        let s = std::str::from_utf8(buf)
            .map(|s| s.to_string())
            .unwrap_or_else(|_| String::from_utf8_lossy(buf).to_string());
        let redacted = redact(&s);
        let mut g = self
            .inner
            .lock()
            .map_err(|e| io::Error::other(e.to_string()))?;
        g.write_all(redacted.as_bytes())?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut g = self
            .inner
            .lock()
            .map_err(|e| io::Error::other(e.to_string()))?;
        g.flush()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    /// module-scope 静态 buffer 收集（让 `impl Write for TestSink` 能引用）。
    static TEST_BUF: std::sync::Mutex<Vec<u8>> = std::sync::Mutex::new(Vec::new());

    struct TestSink;
    impl Write for TestSink {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            TEST_BUF.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// 端到端共用 helper：装 `SanitizingMakeWriter → File`，跑 `tracing::info!`
    /// 后读回文件 body。
    fn run_with_sanitizing_file<F: FnOnce()>(dir_name: &str, f: F) -> String {
        use std::sync::{Arc, Mutex};
        use tracing_subscriber::layer::SubscriberExt;
        struct FileSink(std::fs::File);
        impl Write for FileSink {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                self.0.write(buf)
            }
            fn flush(&mut self) -> io::Result<()> {
                self.0.flush()
            }
        }
        let dir = std::env::temp_dir().join(dir_name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("live2d-ai.log");
        let file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)
            .unwrap();
        let sanitizing = SanitizingMakeWriter::new(Arc::new(Mutex::new(FileSink(file))));
        let subscriber = tracing_subscriber::registry().with(
            tracing_subscriber::fmt::layer()
                .with_writer(sanitizing)
                .with_ansi(false)
                .with_target(true),
        );
        tracing::subscriber::with_default(subscriber, f);
        let body = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        body
    }

    #[test]
    fn redact_replaces_known_sensitive_fields() {
        assert_eq!(redact("api_key=sk-abc123"), "api_key=[REDACTED]");
        assert_eq!(redact("apikey=sk-abc123"), "apikey=[REDACTED]");
        assert_eq!(redact("token=eyJhbGc"), "token=[REDACTED]");
        assert_eq!(redact("password=hunter2"), "password=[REDACTED]");
        assert_eq!(redact("auth=BearerXxx"), "auth=[REDACTED]");
    }

    #[test]
    fn redact_handles_json_style_key_value() {
        let r = redact(r#"{"api_key": "sk-xyz", "name": "ok"}"#);
        assert!(r.contains(r#""api_key": "[REDACTED]""#), "got: {r}");
        assert!(!r.contains("sk-xyz"), "leaked: {r}");
        assert!(r.contains(r#""name": "ok""#), "non-sensitive mutated: {r}");
    }

    #[test]
    fn redact_leaves_non_sensitive_alone() {
        let s = "model=qwen2.5:7b base_url=http://x/v1";
        assert_eq!(redact(s), s);
    }

    #[test]
    fn redact_handles_empty_plain_and_utf8() {
        assert_eq!(redact(""), "");
        assert_eq!(redact("hello world"), "hello world");
        let r = redact("用户=alice token=abc 中文");
        assert!(r.contains("用户=alice"));
        assert!(r.contains("token=[REDACTED]"));
        assert!(r.contains("中文"));
    }

    #[test]
    fn redact_does_not_match_generic_substrings() {
        // 短词 `key` / `secret` 不作为敏感字段（避免误伤 `monkey=` 等）。
        assert_eq!(
            redact("not-a-secret=ok monkey=loose"),
            "not-a-secret=ok monkey=loose"
        );
    }

    #[test]
    fn sanitizing_writer_strips_keys_before_disk() {
        TEST_BUF.lock().unwrap().clear();
        let mut writer = SanitizingWriter::new(Arc::new(Mutex::new(TestSink)));
        let payload = b"api_key=sk-leak-me token=ok name=bob";
        let n = writer.write(payload).unwrap();
        assert_eq!(n, payload.len());
        let mut s = String::new();
        TEST_BUF
            .lock()
            .unwrap()
            .as_slice()
            .read_to_string(&mut s)
            .unwrap();
        assert!(!s.contains("sk-leak-me"), "secret leaked: {s}");
        assert!(s.contains("api_key=[REDACTED]"));
        assert!(s.contains("token=[REDACTED]"));
        assert!(s.contains("name=bob"));
    }

    #[test]
    fn default_log_dir_ends_with_live2d_ai_logs() {
        let p = default_log_dir().unwrap();
        assert!(!p.as_os_str().is_empty());
        assert!(p.to_string_lossy().contains("live2d-ai"));
    }

    /// 端到端：含 `api_key=sk-xxx` 的日志经 SanitizingWriter 后无明文；
    /// 同时验证非敏感字段保留 + 文件含日志内容。
    #[test]
    fn end_to_end_file_layer_redacts_and_persists() {
        let body = run_with_sanitizing_file("live2d-ai-logging-e2e", || {
            tracing::info!(api_key = "sk-LEAK-ME-1234567890", "测试密钥脱敏");
            tracing::info!("plain api_key=sk-ANOTHER-LEAK");
            tracing::info!("token=hunter2 not_a_secret=ok");
            tracing::info!("hello-persist-marker");
        });
        // P0-1：原密钥字串不在落盘文件里。
        assert!(!body.contains("sk-LEAK-ME-1234567890"), "明文泄漏: {body}");
        assert!(!body.contains("sk-ANOTHER-LEAK"), "明文泄漏: {body}");
        assert!(!body.contains("hunter2"), "明文泄漏: {body}");
        assert!(body.contains("[REDACTED]"), "未脱敏: {body}");
        // 非敏感字段保留 + 持久化。
        assert!(body.contains("not_a_secret=ok"), "非敏感字段被误改: {body}");
        assert!(
            body.contains("hello-persist-marker"),
            "文件不含日志内容: {body}"
        );
    }
}
