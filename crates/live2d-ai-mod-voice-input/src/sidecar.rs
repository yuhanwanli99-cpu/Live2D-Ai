//! 面板一键拉起官方 sidecar（L1 硬主路径，2026-09-15）。
//!
//! 宿主进程用 [`std::process::Command`] **逐参数** spawn 官方脚本
//! （`docs/examples/voice-sidecar/voice_sidecar.py`）：
//! `--audio` / `--url` / `[--token]` / `--transcriber`。
//!
//! # 红线：绝不走 `sh -c`
//!
//! argv 是**拼好的参数数组**，不是一条 shell 字符串：`sidecar_python` 只作
//! `argv[0]`，音频路径 / transcriber 里的任何 shell 元字符都不会被解释。
//! 回归 `build_sidecar_argv_never_shells_out` 直接断言 argv 里没有 `-c` /
//! `sh` / 单条字符串拼接。
//!
//! # 本模块不做 IO
//!
//! spawn / wait 在 [`crate::VoiceInputRuntime::run_sidecar`]（命令通道允许本地
//! 文件 IO 与起进程）；这里的纯函数负责「配置怎么解释 / argv 怎么拼 / 状态怎么
//! 序列化」，全部可单测、零副作用。

use serde_json::{Value, json};

/// 缺省 transcriber（`fake`：读同名 `.txt`，开箱即跑 fixtures）。
pub const DEFAULT_TRANSCRIBER: &str = "fake";

/// 缺省 Python 解释器（**只作 argv[0]**，绝不过 shell）。
pub const DEFAULT_PYTHON: &str = "python3";

/// stderr 尾巴最多保留多少字符（面板只展示摘要，不刷屏）。
pub const STDERR_TAIL_CHARS: usize = 600;

/// 读 `sidecar_script`：非字符串 / 空白 → 空串（= 用缺省路径）。
pub fn sidecar_script_from_config(config: &Value) -> String {
    trimmed_string(config, "sidecar_script")
}

/// 读 `sidecar_url`：非字符串 / 空白 → 空串（= 必须由命令参数给）。
pub fn sidecar_url_from_config(config: &Value) -> String {
    trimmed_string(config, "sidecar_url")
}

/// 读 `sidecar_transcriber`：空白 → [`DEFAULT_TRANSCRIBER`]（`fake`）。
pub fn sidecar_transcriber_from_config(config: &Value) -> String {
    let v = trimmed_string(config, "sidecar_transcriber");
    if v.is_empty() {
        DEFAULT_TRANSCRIBER.to_string()
    } else {
        v
    }
}

/// 读 `sidecar_python`：空白 → [`DEFAULT_PYTHON`]（`python3`）。
pub fn sidecar_python_from_config(config: &Value) -> String {
    let v = trimmed_string(config, "sidecar_python");
    if v.is_empty() {
        DEFAULT_PYTHON.to_string()
    } else {
        v
    }
}

fn trimmed_string(config: &Value, key: &str) -> String {
    config
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("")
        .to_string()
}

/// 解析官方 sidecar 脚本路径（**纯函数，不读盘**）：
///
/// - 配置 `sidecar_script` 非空 → 用它；
/// - 否则 `<config 目录>/docs/examples/voice-sidecar/voice_sidecar.py`
///   （`config_path` = `live2d-ai.toml` 的路径，通常就是仓库根）；
/// - `config_path` 也为空 → 空串（不可解析，调用方给可读错误）。
pub fn resolve_sidecar_script(config: &Value, config_path: &str) -> String {
    let configured = sidecar_script_from_config(config);
    if !configured.is_empty() {
        return configured;
    }
    if config_path.trim().is_empty() {
        return String::new();
    }
    let base = std::path::Path::new(config_path)
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_default();
    base.join("docs/examples/voice-sidecar/voice_sidecar.py")
        .to_string_lossy()
        .into_owned()
}

/// 校验音频路径：非空、存在、是**普通文件**。失败 → 可读错误（带路径）。
pub fn validate_audio_path(raw: &str) -> Result<std::path::PathBuf, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(
            "audio_path 不能为空：请填宿主本机上的音频文件路径（例如 docs/examples/voice-sidecar/fixtures/fake_zh.wav）"
                .to_string(),
        );
    }
    let path = std::path::PathBuf::from(trimmed);
    match std::fs::metadata(&path) {
        Ok(m) if m.is_file() => Ok(path),
        Ok(_) => Err(format!(
            "audio_path「{trimmed}」不是普通文件（目录 / 设备节点？）"
        )),
        Err(e) => Err(format!("audio_path「{trimmed}」不存在或读不到：{e}")),
    }
}

/// 拼 sidecar 的 argv（**禁止 shell**）：
/// `[python, script, --audio, <p>, --url, <u>, (--token, <t>)?, --transcriber, <tr>]`
///
/// `token` 为空 / 纯空白 → 不传 `--token`（与服务端「空 = 不鉴权」同口径）。
pub fn build_sidecar_argv(
    python: &str,
    script: &str,
    audio_path: &str,
    url: &str,
    token: Option<&str>,
    transcriber: &str,
) -> Vec<String> {
    let mut argv: Vec<String> = vec![
        python.to_string(),
        script.to_string(),
        "--audio".to_string(),
        audio_path.to_string(),
        "--url".to_string(),
        url.to_string(),
    ];
    if let Some(t) = token.map(str::trim).filter(|t| !t.is_empty()) {
        argv.push("--token".to_string());
        argv.push(t.to_string());
    }
    argv.push("--transcriber".to_string());
    argv.push(transcriber.to_string());
    argv
}

/// sidecar 进程状态（面板 / `GET /state` 可观察）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidecarState {
    /// 还没拉起过。
    Idle,
    /// 已 spawn，等待退出。
    Running,
    /// 已退出（exit_code 见 [`SidecarStatus::exit_code`]）。
    Exited,
    /// spawn 本身失败（`python` 起不来等）。
    SpawnFailed,
}

impl SidecarState {
    /// 稳定字符串（`state_json` 契约）。
    pub const fn as_str(self) -> &'static str {
        match self {
            SidecarState::Idle => "idle",
            SidecarState::Running => "running",
            SidecarState::Exited => "exited",
            SidecarState::SpawnFailed => "spawn_failed",
        }
    }
}

/// sidecar 运行态快照（零 IO：只由 spawn / wait 线程推进）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidecarStatus {
    pub state: SidecarState,
    pub pid: Option<u32>,
    pub exit_code: Option<i32>,
    /// 退出时刻（Unix 秒）；未退出 = `None`。
    pub finished_at: Option<u64>,
    /// 最近一次拉起的音频路径（面板回显用）。
    pub last_audio: Option<String>,
    /// stderr 尾巴（截断，面板排障用）。
    pub stderr_tail: String,
}

impl Default for SidecarStatus {
    fn default() -> Self {
        Self {
            state: SidecarState::Idle,
            pid: None,
            exit_code: None,
            finished_at: None,
            last_audio: None,
            stderr_tail: String::new(),
        }
    }
}

impl SidecarStatus {
    /// spawn 成功后置 running。
    pub fn record_spawn(&mut self, pid: u32, audio: &str) {
        self.state = SidecarState::Running;
        self.pid = Some(pid);
        self.exit_code = None;
        self.finished_at = None;
        self.last_audio = Some(audio.to_string());
        self.stderr_tail.clear();
    }

    /// spawn 失败：记 spawn_failed + 可读原因（放进 stderr_tail 供面板展示）。
    pub fn record_spawn_failed(&mut self, audio: &str, reason: &str) {
        self.state = SidecarState::SpawnFailed;
        self.pid = None;
        self.exit_code = None;
        self.finished_at = Some(unix_secs());
        self.last_audio = Some(audio.to_string());
        self.stderr_tail = stderr_tail(reason, STDERR_TAIL_CHARS);
    }

    /// 进程退出：置 exited + exit_code + stderr 尾巴。
    pub fn record_finish(&mut self, exit_code: Option<i32>, stderr: &str) {
        self.state = SidecarState::Exited;
        self.exit_code = exit_code;
        self.finished_at = Some(unix_secs());
        self.stderr_tail = stderr_tail(stderr, STDERR_TAIL_CHARS);
    }

    /// `GET /state` 契约 JSON（**零 IO**，只序列化内存快照）。
    pub fn to_json(&self) -> Value {
        json!({
            "state": self.state.as_str(),
            "pid": self.pid,
            "exit_code": self.exit_code,
            "finished_at": self.finished_at,
            "last_audio": self.last_audio,
            "stderr_tail": self.stderr_tail,
        })
    }
}

/// 取字符串尾部至多 `max_chars` 个字符（超长时前置省略号）。
pub fn stderr_tail(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim_end();
    if max_chars == 0 {
        return String::new();
    }
    let chars: Vec<char> = trimmed.chars().collect();
    if chars.len() <= max_chars {
        return trimmed.to_string();
    }
    let mut out = String::from("…");
    out.extend(chars[chars.len() - max_chars..].iter());
    out
}

/// 当前 Unix 秒（退出时刻用；不引第三方依赖）。
fn unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
#[path = "sidecar_tests.rs"]
mod sidecar_tests;
