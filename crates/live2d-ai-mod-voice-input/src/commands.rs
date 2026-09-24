//! runtime 的命令分派 / 注入 / sidecar 拉起 / 运行态快照（L1 产品级，2026-09-15）。
//!
//! 拆出来的原因只有一个：`lib.rs` 要保持在 500 行以内，而这些方法都属于
//! [`crate::VoiceInputRuntime`]。impl 块可以放在 crate 内任意模块，子模块也
//! 能访问根模块结构体的私有字段（Rust 隐私规则：私有项对当前模块及其后代可见）。

use serde_json::{Value, json};

use live2d_ai_mod_system::ModError;

use crate::gate::{self, GateOutcome};
use crate::sidecar;
use crate::{VoiceInputRuntime, clean_transcript, normalize_for_locale, token_from_config};

/// 一次**受闸门约束**的注入结果（命令 `inject` 的返回体真源）。
///
/// `accepted == true` ⇔ 通过两把闸 + 唤醒词命中 + 主链接受（`say` 回 true）。
/// 被拒时 `rejected_code` 是稳定码，`message` 是可执行处置——
/// 面板「验证闸门」按钮直接把这两项摊开给用户看。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InjectReport {
    pub accepted: bool,
    pub text: String,
    pub backend: &'static str,
    pub locale: String,
    pub rejected_code: Option<&'static str>,
    pub message: String,
}

impl InjectReport {
    /// 命令返回体（稳定键集，供前端渲染）。
    pub fn to_json(&self) -> Value {
        json!({
            "ok": self.accepted,
            "accepted": self.accepted,
            "text": self.text,
            "backend": self.backend,
            "locale": self.locale,
            "rejected_code": self.rejected_code,
            "message": self.message,
        })
    }
}

impl VoiceInputRuntime {
    /// 生效的唤醒短语（**内部用**；绝不写进响应 / 日志 / state）。
    pub fn wake_phrase(&self) -> String {
        gate::wake_phrase_from_config(&self.config)
    }

    /// 能力总闸是否打开（= 唤醒短语非空）。
    pub fn wake_gate_open(&self) -> bool {
        gate::wake_gate_open(&self.config)
    }

    /// 手动闸是否打开（缺省 true）。
    pub fn manual_enabled(&self) -> bool {
        gate::manual_enabled_from_config(&self.config)
    }

    /// 解析出的官方 sidecar 脚本路径（纯函数，不读盘）。
    pub fn sidecar_script(&self) -> String {
        sidecar::resolve_sidecar_script(&self.config, &self.services.config_path)
    }

    /// **受闸门约束**的语音 → say（端点与命令 `inject` 共用这一条路径）。
    ///
    /// 顺序与 handler 逐字一致：manual → 总闸 → 唤醒词 → 剥短语 → 空文本 →
    /// locale 归一化 → `say_tx`。总闸关时返回**被拒绝的可读结果**（不抛错）。
    pub fn inject_gated(&self, raw: &str) -> InjectReport {
        let backend = self.backend().as_str();
        let locale = self.locale();
        let cleaned = clean_transcript(raw).unwrap_or_default();
        match gate::evaluate(&self.config, &cleaned) {
            GateOutcome::Allow { text } => {
                let normalized = normalize_for_locale(&text, &locale);
                if normalized.is_empty() {
                    let message =
                        "唤醒短语之后没有正文（整句就是唤醒词）：不发空回合".to_string();
                    self.services
                        .logger
                        .warn(&format!("语音转写被闸门拒绝（empty_transcript）：{message}"));
                    return InjectReport {
                        accepted: false,
                        text: normalized,
                        backend,
                        locale,
                        rejected_code: Some("empty_transcript"),
                        message,
                    };
                }
                let accepted = self.services.say_tx.say(normalized.clone());
                let (rejected_code, message) = if accepted {
                    (
                        None,
                        format!("已注入主链（backend={backend}, locale={locale}）"),
                    )
                } else {
                    (
                        Some("busy"),
                        "主链忙碌（pending 缓冲已满），本条被丢弃：等 2 到 5 秒再试".to_string(),
                    )
                };
                if !accepted {
                    self.services.logger.warn(&message);
                }
                InjectReport {
                    accepted,
                    text: normalized,
                    backend,
                    locale,
                    rejected_code,
                    message,
                }
            }
            GateOutcome::ManualOff => self.reject(
                backend,
                locale,
                cleaned,
                "voice_manual_off",
                "手动闸已关闭（manual_enabled=false）：先在 Mod 配置里打开手动开关",
            ),
            GateOutcome::GateClosed => self.reject(
                backend,
                locale,
                cleaned,
                "voice_gate_closed",
                "语音总闸被显式关闭（wake_phrase 为空）：在 Mod 配置里填一个唤醒词再保存（缺省 小可爱）",
            ),
            GateOutcome::WakeRequired => {
                let message = format!(
                    "转写里没有唤醒短语「{}」：请先说唤醒词再说话（大小写不敏感、忽略空白差异）",
                    self.wake_phrase()
                );
                self.reject(backend, locale, cleaned, "wake_phrase_required", &message)
            }
        }
    }

    /// 构造一个「被拒绝」的注入报告（只写 warn 日志，不碰 `say_tx`）。
    fn reject(
        &self,
        backend: &'static str,
        locale: String,
        text: String,
        code: &'static str,
        message: &str,
    ) -> InjectReport {
        self.services
            .logger
            .warn(&format!("语音转写被闸门拒绝（{code}）：{message}"));
        InjectReport {
            accepted: false,
            text,
            backend,
            locale,
            rejected_code: Some(code),
            message: message.to_string(),
        }
    }

    /// **语音 → 文本 → say**（受闸门约束）：返回是否被主链接受。
    ///
    /// 与旧行为的有意差异：**总闸（唤醒短语）缺省 = 关**，所以缺省配置下
    /// 返回 `false` 并留下 `voice_gate_closed` 的 warn —— 见模块头注。
    pub fn inject_transcript(&self, raw: &str) -> bool {
        self.inject_gated(raw).accepted
    }

    /// mock 后端便捷入口：与 [`Self::inject_transcript`] 同路径，多一行日志。
    pub fn inject_mock_transcript(&self, raw: &str) -> bool {
        self.services.logger.info(&format!(
            "mock 后端收到转写（backend={}）",
            self.backend().as_str()
        ));
        self.inject_transcript(raw)
    }

    /// **零 IO** 运行态快照（`GET /api/v1/mods/voice-input/state` 的 `state`）。
    ///
    /// 只读内存里的 `config` 与 [`sidecar::SidecarStatus`]：不读盘、不 spawn、
    /// 不阻塞（web_api 是单线程的）。
    pub fn state_snapshot(&self) -> Value {
        let status = self
            .sidecar
            .lock()
            .map(|g| g.clone())
            .unwrap_or_else(|poisoned| poisoned.into_inner().clone());
        let wake_open = self.wake_gate_open();
        json!({
            "backend": self.backend().as_str(),
            "locale": self.locale(),
            "manual_enabled": self.manual_enabled(),
            // 总闸生效态（缺键走缺省词时也为 true）。
            "wake_gate_open": wake_open,
            // 只回布尔，**绝不回唤醒短语明文**；这里答的是「用户显式设过没有」。
            "wake_phrase_set": gate::wake_phrase_explicitly_set(&self.config),
            "token_set": token_from_config(&self.config).is_some(),
            // 只回解析出的路径（字符串），不 stat（零 IO 契约）。
            "sidecar_script": self.sidecar_script(),
            "sidecar_status": status.to_json(),
        })
    }

    /// 命令分派：`selftest` / `inject` / `run_sidecar`；其它 → 409 契约。
    pub fn dispatch_command(&mut self, command: &str, args: &Value) -> Result<Value, ModError> {
        match command.trim() {
            "selftest" => Ok(self.selftest()),
            "inject" => {
                let text = args.get("text").and_then(Value::as_str).unwrap_or("");
                Ok(self.inject_gated(text).to_json())
            }
            "run_sidecar" => self.run_sidecar(args),
            other => Err(ModError::UnsupportedCommand {
                command: other.to_string(),
            }),
        }
    }

    /// **硬主路径**：一键拉起官方 sidecar（面板按钮 → 本命令）。
    ///
    /// 校验（失败 → `409 command_failed`，消息可读且带路径）：
    /// 1. `args.audio_path` 非空、存在、是普通文件；
    /// 2. URL：`args.url` 优先，其次 `sidecar_url` 配置；
    /// 3. 脚本路径：配置优先，否则 `<config 目录>/docs/examples/...`，必须存在；
    /// 4. transcriber：`args.transcriber` 优先，其次配置，缺省 `fake`。
    ///
    /// spawn **成功**时立刻返回（不等进程）：HTTP 服务器是单线程的，而 sidecar
    /// 会 POST 回 `/api/v1/voice/transcript`——在这里 `wait()` 会**死锁**。
    /// 另起后台线程 `wait()` 并把退出码 / stderr 尾巴写进
    /// [`sidecar::SidecarStatus`]，面板经 `state_json` 观察。
    pub fn run_sidecar(&self, args: &Value) -> Result<Value, ModError> {
        // ① 音频：非空 + 存在 + 普通文件。
        let audio_raw = args.get("audio_path").and_then(Value::as_str).unwrap_or("");
        let audio = sidecar::validate_audio_path(audio_raw).map_err(ModError::Other)?;
        let audio_display = audio.to_string_lossy().into_owned();

        // ② URL：命令参数优先，其次配置。
        let arg_url = args
            .get("url")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let url = match arg_url {
            Some(u) => u.to_string(),
            None => {
                let configured = sidecar::sidecar_url_from_config(&self.config);
                if configured.is_empty() {
                    return Err(ModError::Other(
                        "缺少 sidecar 目标 URL：面板会按浏览器 origin 自动填                          http://<host>/api/v1/voice/transcript；命令行请给 args.url 或配置 sidecar_url"
                            .to_string(),
                    ));
                }
                configured
            }
        };

        // ③ 脚本：配置优先，否则 <config 目录>/docs/examples/voice-sidecar/voice_sidecar.py。
        let script = self.sidecar_script();
        if script.trim().is_empty() {
            return Err(ModError::Other(
                "sidecar 脚本路径无法解析：请填 sidecar_script 配置，或让宿主以仓库根的                  live2d-ai.toml 启动（缺省 <config 目录>/docs/examples/voice-sidecar/voice_sidecar.py）"
                    .to_string(),
            ));
        }
        if !std::path::Path::new(&script).is_file() {
            return Err(ModError::Other(format!(
                "sidecar 脚本不存在：{script}（确认仓库路径，或在 Mod 配置里填 sidecar_script）"
            )));
        }

        // ④ transcriber / python / token。
        let transcriber = args
            .get("transcriber")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| sidecar::sidecar_transcriber_from_config(&self.config));
        let python = sidecar::sidecar_python_from_config(&self.config);
        let token = token_from_config(&self.config);
        let argv = sidecar::build_sidecar_argv(
            &python,
            &script,
            &audio_display,
            &url,
            token.as_deref(),
            &transcriber,
        );

        // ⑤ spawn：**逐参数**，绝不拼 shell 字符串。
        let mut cmd = std::process::Command::new(&argv[0]);
        cmd.args(&argv[1..]);
        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::piped());
        match cmd.spawn() {
            Ok(mut child) => {
                let pid = child.id();
                self.apply_sidecar(|st| st.record_spawn(pid, &audio_display));
                // 后台线程 wait：立刻返回，HTTP 线程绝不等 sidecar。
                let shared = std::sync::Arc::clone(&self.sidecar);
                let spawned = std::thread::Builder::new()
                    .name("voice-sidecar-wait".to_string())
                    .spawn(move || {
                        let mut tail = String::new();
                        if let Some(mut err) = child.stderr.take() {
                            use std::io::Read;
                            let _ = err.read_to_string(&mut tail);
                        }
                        let code = child.wait().ok().and_then(|s| s.code());
                        if let Ok(mut st) = shared.lock() {
                            st.record_finish(code, &tail);
                        }
                    })
                    .is_ok();
                if !spawned {
                    self.services
                        .logger
                        .warn("sidecar wait 线程创建失败；进程仍在跑，但退出码不会回写");
                }
                Ok(json!({
                    "spawned": true,
                    "pid": pid,
                    "script": script,
                    "audio_path": audio_display,
                    "url": url,
                    "transcriber": transcriber,
                    "python": python,
                    "wait_thread": spawned,
                }))
            }
            Err(e) => {
                let reason = format!("拉起 sidecar 失败（python=「{python}」起不来？）：{e}");
                self.apply_sidecar(|st| st.record_spawn_failed(&audio_display, &reason));
                self.services.logger.warn(&reason);
                Ok(json!({
                    "spawned": false,
                    "state": "spawn_failed",
                    "error": reason,
                    "script": script,
                    "audio_path": audio_display,
                    "url": url,
                }))
            }
        }
    }

    /// 拿 sidecar 状态锁改一次（中毒不 panic：快照仍可读写）。
    fn apply_sidecar(&self, f: impl FnOnce(&mut sidecar::SidecarStatus)) {
        match self.sidecar.lock() {
            Ok(mut guard) => f(&mut guard),
            Err(poisoned) => f(&mut poisoned.into_inner()),
        }
    }
}
