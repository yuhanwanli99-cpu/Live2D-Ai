//! live2d-ai-mod-local-tts（节点 E E5）。
//!
//! v1 进程生命周期管理 Mod：管理本地 TTS 推理进程
//! （**2026-09-10 起默认面向 CosyVoice 3** 的 OpenAI 兼容层，
//! 如社区 `CosyVoice3-API`；同类 OpenAI 兼容 `/v1/audio/speech` 服务同样适用）的
//! spawn / 就绪探测 / 回收 / endpoint 自动探测。
//!
//! 接入文档：`docs/architecture/cosyvoice3-tts-integration.md`。
//!
//! # v1 范围声明
//!
//! - **不内置推理**：本 Mod 仅负责 spawn 外部 TTS 进程、探测端口就绪、
//!   回收子进程，并在就绪后自动记录 endpoint；不绑定任何推理后端。
//! - **进程生命周期**：`start`（auto_start）→ spawn + 后台就绪探测线程；
//!   `shutdown` → `child.kill()` + `wait()` 回收 + 复位。
//! - **P0-4 真实闭环**：TCP 端口通后进行真实 HTTP 探测（`/v1/models`），
//!   探测通过 → `ready=true` + 通过 `event_tx` 发送
//!   `{"__apply_settings":true,"patch":{"tts":{"base_url":...,"voice":...}}}`
//!   触发 host 写盘 + `supervisor.reload()`。
//! - **externally_managed**：为 `true` 时不 spawn 子进程，仅探活 + 写 settings
//!   （用户在外部启动服务，如 Windows 系统 TTS）。
//! - **配置**：`command` / `args` / `port` / `voice` / `auto_start` /
//!   `health_timeout_ms` / `externally_managed`，均有缺省值。
//! - **事件**：订阅 `VoiceStarted`（v1 仅记录日志）。
//!
//! # 配置字段
//!
//! - `command`（String，缺省 `"cosyvoice3-api"`）：TTS 服务可执行命令
//!   （仅 `externally_managed=false` 时使用；缺省不 spawn）。
//! - `args`（String 数组，缺省 `["--port","8080"]`）：启动参数
//!   （示例；真实参数以你的部署为准）。
//! - `port`（u16，缺省 `8080`）：探测就绪的 TCP 端口（CosyVoice3-API 默认 8080；
//!   官方 FastAPI 默认 50000）。
//! - `voice`（String，缺省空）：**已注册的克隆音色 id**（CosyVoice3 无内置音色，
//!   需先 `POST /v1/voices/register`）；只有显式配置才写回 settings。
//! - `auto_start`（bool，缺省 `true`）：start 时自动 spawn。
//! - `health_timeout_ms`（u64，缺省 `15000`）：就绪探测超时。
//! - `externally_managed`（bool，缺省 `true`）：为 `true` 时不 spawn 子进程，
//!   仅探活 + 写 settings（用户在外部起服务）。
//!
//! # 版本
//!
//! `api_version` 声明为 1（对齐 `live2d-ai-mod-system` API 版本）。

use live2d_ai_mod_system::*;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// 默认配置常量（与 `config` 字段缺省一致）。
const DEFAULT_COMMAND: &str = "cosyvoice3-api";
const DEFAULT_ARGS: &[&str] = &["--port", "8080"];
const DEFAULT_PORT: u16 = 8080;
const DEFAULT_HEALTH_TIMEOUT_MS: u64 = 15000;
/// 缺省**外部管理**：只探活 + 写 settings，不 spawn 子进程。
///
/// 2026-09-10 点火轮：本地实现由用户自带（任何 OpenAI 兼容 `/audio/speech` 端点），
/// Mod 不再默认去 spawn 一个可能并不存在的进程。
const DEFAULT_EXTERNALLY_MANAGED: bool = true;

/// Mod 描述符（静态身份）。
const DESCRIPTOR: ModDescriptor = ModDescriptor {
    id: "local-tts",
    name: "本地语音合成",
    version: "0.1.0",
    api_version: 1,
};

/// local-tts 的运行时状态。
pub struct LocalTtsRuntime {
    services: ModServices,
    /// 当前生效配置（host 传入的 namespaced JSON）。
    config: serde_json::Value,
    /// settings schema 是否已注册（start 成功标志）。
    registered: bool,
    /// 子进程句柄（跨线程共享，spawn 线程 + shutdown 回收安全）。
    child: Arc<Mutex<Option<std::process::Child>>>,
    /// 端口就绪探测结果（就绪=true）。跨线程共享，供后台探测线程更新。
    ready: Arc<AtomicBool>,
    /// 最近一次错误原因。
    last_error: Option<String>,
}

impl Default for LocalTtsRuntime {
    fn default() -> Self {
        Self {
            services: ModServices::new(
                ModActionSender::new(|_| true),
                SaySender::new(|_| true),
                ModEventSender::new(|_topic, _payload| true),
                ModLogger::new(|_lvl, _msg| {}),
            ),
            config: serde_json::Value::Object(Default::default()),
            registered: false,
            child: Arc::new(Mutex::new(None)),
            ready: Arc::new(AtomicBool::new(false)),
            last_error: None,
        }
    }
}

/// local-tts 工厂。
pub struct LocalTtsFactory;

impl ModFactory for LocalTtsFactory {
    fn descriptor(&self) -> &'static ModDescriptor {
        &DESCRIPTOR
    }

    fn create(
        &self,
        services: ModServices,
        config: serde_json::Value,
    ) -> Result<Box<dyn ModRuntime>, ModError> {
        Ok(Box::new(LocalTtsRuntime {
            services,
            config,
            registered: false,
            child: Arc::new(Mutex::new(None)),
            ready: Arc::new(AtomicBool::new(false)),
            last_error: None,
        }))
    }
}

impl LocalTtsRuntime {
    /// 读取 config，带缺省。
    fn command(&self) -> String {
        self.config
            .get("command")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(DEFAULT_COMMAND)
            .to_string()
    }

    fn args(&self) -> Vec<String> {
        self.config
            .get("args")
            .and_then(serde_json::Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(String::from)
                    .collect()
            })
            .filter(|v: &Vec<String>| !v.is_empty())
            .unwrap_or_else(|| DEFAULT_ARGS.iter().map(|s| s.to_string()).collect())
    }

    fn port(&self) -> u16 {
        self.config
            .get("port")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(DEFAULT_PORT as u64) as u16
    }

    /// Mod 配置里**显式**给出的声线；空/缺失 = 不写回 settings。
    ///
    /// 只写用户明确配置的值，避免 Mod 覆盖 `live2d-ai.toml` 里既有的声线选择。
    fn voice_explicit(&self) -> Option<String> {
        self.config
            .get("voice")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
    }

    fn auto_start(&self) -> bool {
        self.config
            .get("auto_start")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true)
    }

    fn health_timeout_ms(&self) -> Duration {
        Duration::from_millis(
            self.config
                .get("health_timeout_ms")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(DEFAULT_HEALTH_TIMEOUT_MS),
        )
    }

    /// 读取 `externally_managed` 配置（缺省 `true`）。
    fn externally_managed(&self) -> bool {
        self.config
            .get("externally_managed")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(DEFAULT_EXTERNALLY_MANAGED)
    }

    /// spawn 子进程 + 后台就绪探测线程（F8 方式：TcpStream::connect_timeout）。
    ///
    /// `externally_managed = true` 时**不** spawn，仅探活 + 写 settings
    /// （用户在外部启动服务）。
    ///
    /// spawn 失败**不返回 Err**（仅记录 `last_error`），符合 v1 约定：
    /// 无命令时不能 crash。
    fn spawn_and_probe(&mut self) -> Result<(), ModError> {
        let command = self.command();
        let args = self.args();
        let port = self.port();
        let timeout = self.health_timeout_ms();
        let ready = self.ready.clone();
        let child_shared = self.child.clone();
        let logger = self.services.logger.clone();
        let event_tx = self.services.event_tx.clone();
        let voice = self.voice_explicit();
        let base_url = format!("http://127.0.0.1:{port}/v1");
        let socket_addr: std::net::SocketAddr = format!("127.0.0.1:{port}")
            .parse()
            .unwrap_or_else(|_| "127.0.0.1:8000".parse().unwrap());

        if !self.externally_managed() {
            match std::process::Command::new(&command).args(&args).spawn() {
                Ok(child) => {
                    // 存入共享句柄。
                    let mut guard = self.child.lock().unwrap();
                    *guard = Some(child);
                }
                Err(e) => {
                    // **M4 统一语义（2026-09-10）**：spawn 失败只记录，不让 Mod start 失败 ——
                    // 服务可能已在外部运行，探测线程仍会尝试探活；与 local-llm 行为对齐。
                    let msg = format!("spawn TTS 进程失败 ({command} {args:?}): {e}");
                    self.last_error = Some(msg.clone());
                    logger.error(&msg);
                }
            }
        } else {
            logger.info("local-tts externally_managed=true，跳过 spawn，仅探活 + 写 settings");
        }

        // 后台线程探测端口就绪（不阻塞 start）。TCP 通后做真实 HTTP 探测。
        std::thread::spawn(move || {
            let deadline = Instant::now() + timeout;
            // 阶段 1：TCP 端口探活。
            loop {
                if TcpStream::connect_timeout(&socket_addr, Duration::from_millis(500)).is_ok() {
                    break;
                }
                if Instant::now() >= deadline {
                    logger.warn("local-tts 服务就绪探测超时");
                    // 超时未就绪，杀掉子进程（externally_managed 时 child 不存在，跳过）。
                    let mut guard = child_shared.lock().unwrap();
                    if let Some(mut child) = guard.take() {
                        let _ = child.kill();
                        let _ = child.wait();
                    }
                    return;
                }
                std::thread::sleep(Duration::from_millis(500));
            }

            // 阶段 2：真实 HTTP 探测 —— `/v1/models`（OpenAI 兼容健康端点）。
            let probe = |_p: u16| Self::http_get(&base_url, "/v1/models", Duration::from_secs(2));
            let apply = |patch: serde_json::Value| {
                event_tx.try_emit(ModEventTopic::VoiceStarted, &patch.to_string())
            };
            probe_and_configure(
                port,
                &ready,
                &logger,
                &probe,
                &apply,
                &base_url,
                voice.as_deref(),
            );
        });

        Ok(())
    }

    /// 手写超轻量 HTTP/1.1 GET —— 纯 std，无额外依赖。
    ///
    /// 返回 `Ok(())` 若收到 2xx 状态码，`Err` 否则。
    fn http_get(base_url: &str, path: &str, timeout: Duration) -> Result<(), String> {
        // 解析 host:port。
        let host = base_url
            .trim_start_matches("http://")
            .trim_start_matches("https://")
            .trim_end_matches('/')
            .split('/')
            .next()
            .unwrap_or("127.0.0.1:8000");
        let (host_str, port_str) = host.split_once(':').unwrap_or((host, "80"));
        let port: u16 = port_str
            .parse()
            .map_err(|e: std::num::ParseIntError| format!("端口解析失败: {e}"))?;
        let addr = format!("{host_str}:{port}")
            .parse::<std::net::SocketAddr>()
            .map_err(|e| format!("地址解析失败: {e}"))?;

        let request =
            format!("GET {path} HTTP/1.1\r\nHost: {host_str}:{port}\r\nConnection: close\r\n\r\n");
        let mut stream = std::net::TcpStream::connect_timeout(&addr, timeout)
            .map_err(|e| format!("连接失败: {e}"))?;
        stream
            .set_read_timeout(Some(timeout))
            .map_err(|e| format!("设置读超时失败: {e}"))?;
        stream
            .write_all(request.as_bytes())
            .map_err(|e| format!("写请求失败: {e}"))?;

        // 读完整响应。
        let mut buf = [0u8; 1024];
        let mut body = String::new();
        loop {
            let n = stream
                .read(&mut buf)
                .map_err(|e| format!("读响应失败: {e}"))?;
            if n == 0 {
                break;
            }
            body.push_str(&String::from_utf8_lossy(&buf[..n]));
            // 只要读完状态行即可。
            if body.contains("\r\n") {
                break;
            }
        }
        // 解析状态行：`HTTP/1.1 200 OK`。
        let status_line = body.lines().next().unwrap_or("");
        let parts: Vec<&str> = status_line.split_whitespace().collect();
        if parts.len() >= 2 {
            let code: u16 = parts[1]
                .parse()
                .map_err(|e: std::num::ParseIntError| format!("状态码解析失败: {e}"))?;
            if (200..300).contains(&code) {
                return Ok(());
            }
            return Err(format!("HTTP 探测返回非 2xx: {code}"));
        }
        Err("HTTP 探测: 无法解析状态行".to_string())
    }

    /// 是否就绪（HTTP 探测成功）。
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::SeqCst)
    }

    /// 最近一次错误。
    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    /// 当前 config。
    pub fn config(&self) -> &serde_json::Value {
        &self.config
    }
}

impl ModRuntime for LocalTtsRuntime {
    fn start(&mut self, registrar: &mut dyn ModRegistrar) -> Result<(), ModError> {
        // 注册 settings schema（前端统一渲染 “本地语音合成” 面板）。
        // 5 个字段：command / port / voice / auto_start / health_timeout_ms / externally_managed。
        let spec = ModSettingsSpec {
            mod_id: "local-tts".to_string(),
            title: "本地语音合成".to_string(),
            version: 1,
            fields: vec![
                ModSettingField::String {
                    key: "command".to_string(),
                    label: "TTS 服务命令".to_string(),
                    secret: false,
                },
                ModSettingField::Number {
                    key: "port".to_string(),
                    label: "服务端口".to_string(),
                    min: 1024.0,
                    max: 65535.0,
                },
                ModSettingField::String {
                    key: "voice".to_string(),
                    label: "声线 id".to_string(),
                    secret: false,
                },
                ModSettingField::Bool {
                    key: "auto_start".to_string(),
                    label: "自动启动服务".to_string(),
                    default: true,
                },
                ModSettingField::Number {
                    key: "health_timeout_ms".to_string(),
                    label: "就绪探测超时（毫秒）".to_string(),
                    min: 1000.0,
                    max: 60000.0,
                },
                ModSettingField::Bool {
                    key: "externally_managed".to_string(),
                    label: "外部管理（不 spawn 子进程，仅探活 + 写 settings）".to_string(),
                    default: false,
                },
            ],
        };
        registrar.register_settings(spec)?;

        // 订阅 VoiceStarted（v1 仅记录日志）。
        registrar.subscribe(ModEventTopic::VoiceStarted)?;

        self.registered = true;

        // auto_start → spawn + 后台就绪探测。
        if self.auto_start()
            && let Err(e) = self.spawn_and_probe()
        {
            self.services
                .logger
                .error(&format!("local-tts 自启动失败: {}", e));
            return Err(e);
        }

        self.services.logger.info("local-tts Mod 已启动");
        Ok(())
    }

    fn on_event(&mut self, topic: ModEventTopic, payload: &str) -> Result<(), ModError> {
        if topic == ModEventTopic::VoiceStarted {
            self.services
                .logger
                .info(&format!("local-tts 收到 VoiceStarted: {}", payload));
        }
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), ModError> {
        // 回收子进程（kill + wait）。
        let mut guard = self.child.lock().unwrap();
        if let Some(mut child) = guard.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        *guard = None;

        self.registered = false;
        self.ready.store(false, Ordering::SeqCst);
        self.last_error = None;
        self.services.logger.info("local-tts Mod 已关闭");
        Ok(())
    }
}

/// 供 host 装配 `AVAILABLE_MOD_FACTORIES` 的工厂单例。
pub static FACTORY: LocalTtsFactory = LocalTtsFactory;

/// 探测 + 写 settings 的纯逻辑函数（P0-4 真实闭环）。
///
/// `probe`：探测函数（生产路径为 `http_get`，测试路径为 mock）。
/// `apply`：settings patch 应用函数（生产路径为 `event_tx.try_emit`，
/// 测试路径为 mock）。
///
/// 探测成功 → `ready=true` + 构造 `{"__apply_settings":true,"patch":{...}}`，
/// 交给 `apply` 回调完成写盘 + reload。
/// 探测失败 → `ready=false`。
fn probe_and_configure(
    port: u16,
    ready: &Arc<AtomicBool>,
    logger: &ModLogger,
    probe: &dyn Fn(u16) -> Result<(), String>,
    apply: &dyn Fn(serde_json::Value) -> bool,
    base_url: &str,
    voice: Option<&str>,
) {
    match probe(port) {
        Ok(()) => {
            ready.store(true, Ordering::SeqCst);
            logger.info("local-tts 探测通过，服务就绪");
            // `base_url` 必写；`voice` 仅在 Mod 显式配置时写（不覆盖用户选择）。
            let mut tts = serde_json::json!({ "base_url": base_url });
            if let Some(v) = voice {
                tts["voice"] = serde_json::Value::String(v.to_string());
            }
            let patch = serde_json::json!({
                "__apply_settings": true,
                "patch": { "tts": tts }
            });
            if !apply(patch) {
                logger.warn("local-tts apply_settings 调用失败");
            }
        }
        Err(e) => {
            ready.store(false, Ordering::SeqCst);
            let msg = format!("local-tts 探测失败: {}", e);
            logger.warn(&msg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试用 ModRegistrar mock。
    #[derive(Default)]
    struct MockRegistrar {
        specs: Vec<ModSettingsSpec>,
        subs: Vec<SubscriptionId>,
    }
    impl ModRegistrar for MockRegistrar {
        fn register_settings(&mut self, spec: ModSettingsSpec) -> Result<(), ModError> {
            self.specs.push(spec);
            Ok(())
        }
        fn subscribe(&mut self, _topic: ModEventTopic) -> Result<SubscriptionId, ModError> {
            let id = SubscriptionId(self.subs.len() as u64 + 1);
            self.subs.push(id);
            Ok(id)
        }
        fn unsubscribe(&mut self, _id: SubscriptionId) -> Result<(), ModError> {
            Ok(())
        }
    }

    fn test_services() -> ModServices {
        ModServices::new(
            ModActionSender::new(|_| true),
            SaySender::new(|_| true),
            ModEventSender::new(|_t, _p| true),
            ModLogger::new(|_l, _m| {}),
        )
    }

    #[test]
    fn descriptor_is_static() {
        let f = LocalTtsFactory;
        let d = f.descriptor();
        assert_eq!(d.id, "local-tts");
        assert_eq!(d.name, "本地语音合成");
        assert_eq!(d.version, "0.1.0");
        assert_eq!(d.api_version, 1);
    }

    #[test]
    fn start_registers_settings_and_one_subscription() {
        let services = ModServices::new(
            ModActionSender::new(|_| true),
            SaySender::new(|_| true),
            ModEventSender::new(|_t, _p| true),
            ModLogger::new(|_l, _m| {}),
        );
        // auto_start=false，避免 spawn。
        let config = serde_json::json!({ "auto_start": false });
        let mut rt = FACTORY.create(services, config).unwrap();
        let mut mock = MockRegistrar::default();
        rt.start(&mut mock).unwrap();
        // 1 个 settings spec，6 个字段。
        assert_eq!(mock.specs.len(), 1);
        let spec = &mock.specs[0];
        assert_eq!(spec.mod_id, "local-tts");
        assert_eq!(spec.fields.len(), 6);
        assert_eq!(spec.fields[0].key(), "command");
        assert_eq!(spec.fields[1].key(), "port");
        assert_eq!(spec.fields[2].key(), "voice");
        assert_eq!(spec.fields[3].key(), "auto_start");
        assert_eq!(spec.fields[4].key(), "health_timeout_ms");
        assert_eq!(spec.fields[5].key(), "externally_managed");
        // 验证 port 字段的 min/max。
        match &spec.fields[1] {
            ModSettingField::Number { min, max, .. } => {
                assert_eq!(*min, 1024.0);
                assert_eq!(*max, 65535.0);
            }
            _ => panic!("port 字段应为 Number"),
        }
        // 1 个订阅：VoiceStarted。
        assert_eq!(mock.subs.len(), 1);
    }

    #[test]
    fn config_defaults() {
        let services = ModServices::new(
            ModActionSender::new(|_| true),
            SaySender::new(|_| true),
            ModEventSender::new(|_t, _p| true),
            ModLogger::new(|_l, _m| {}),
        );
        // 空 config → 使用缺省。
        let rt = LocalTtsRuntime {
            services,
            config: serde_json::json!({}),
            registered: false,
            child: Arc::new(Mutex::new(None)),
            ready: Arc::new(AtomicBool::new(false)),
            last_error: None,
        };
        assert_eq!(rt.command(), DEFAULT_COMMAND);
        assert_eq!(
            rt.args(),
            DEFAULT_ARGS
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        );
        assert_eq!(rt.port(), DEFAULT_PORT);
        assert!(rt.auto_start());
        assert_eq!(rt.voice_explicit(), None);
        assert_eq!(
            rt.health_timeout_ms(),
            Duration::from_millis(DEFAULT_HEALTH_TIMEOUT_MS)
        );
    }

    #[test]
    fn auto_start_false_does_not_spawn() {
        let services = ModServices::new(
            ModActionSender::new(|_| true),
            SaySender::new(|_| true),
            ModEventSender::new(|_t, _p| true),
            ModLogger::new(|_l, _m| {}),
        );
        let config = serde_json::json!({ "auto_start": false });
        let mut rt = LocalTtsRuntime {
            services,
            config,
            registered: false,
            child: Arc::new(Mutex::new(None)),
            ready: Arc::new(AtomicBool::new(false)),
            last_error: None,
        };
        let mut mock = MockRegistrar::default();
        // auto_start=false → 不 spawn，不 panic。
        rt.start(&mut mock).unwrap();
        assert!(!rt.is_ready());
    }

    #[test]
    fn externally_managed_defaults_true() {
        let rt = LocalTtsRuntime {
            services: test_services(),
            config: serde_json::json!({}),
            registered: false,
            child: Arc::new(Mutex::new(None)),
            ready: Arc::new(AtomicBool::new(false)),
            last_error: None,
        };
        // 2026-09-10 点火轮：默认不 spawn（本地实现由用户自带的 OpenAI 兼容端点承担）。
        assert!(rt.externally_managed());
    }

    /// externally_managed=true 时不 spawn 子进程，仅探活 + 写 settings。
    /// 通过 spawn_and_probe 间接验证：child 槽位保持 None（未 spawn），
    /// 但不会 panic。
    #[test]
    fn externally_managed_true_does_not_spawn_but_reports_ready_logic() {
        let mut rt = LocalTtsRuntime {
            services: test_services(),
            config: serde_json::json!({ "externally_managed": true, "auto_start": false }),
            registered: false,
            child: Arc::new(Mutex::new(None)),
            ready: Arc::new(AtomicBool::new(false)),
            last_error: None,
        };
        let mut mock = MockRegistrar::default();
        // auto_start=false，手动调用 spawn_and_probe 验证 externally_managed 分支。
        rt.start(&mut mock).unwrap();
        // 未 auto_start → 未 spawn。
        assert!(rt.child.lock().unwrap().is_none());

        // 直接调用 spawn_and_probe：externally_managed=true → 不 spawn。
        // 后台线程会探测端口（不可达）→ 超时后杀掉 child（None，无操作）。
        // 我们只验证不会 panic + child 槽位仍空。
        rt.spawn_and_probe().unwrap();
        // child 槽位仍为空（externally_managed 模式下不 spawn）。
        assert!(
            rt.child.lock().unwrap().is_none(),
            "externally_managed=true 不应 spawn 子进程"
        );

        // 验证语义：就绪后应通过 apply 发送正确的 settings patch payload。
        let captured: Arc<Mutex<Option<serde_json::Value>>> = Arc::new(Mutex::new(None));
        let captured_c = captured.clone();

        // 模拟就绪探测成功 → 直接构造 patch 并验证。
        let port = 8000u16;
        let base_url = format!("http://127.0.0.1:{port}/v1");
        let voice = Some("af_heart".to_string());

        let probe = |_p: u16| Ok(());
        let apply = move |patch: serde_json::Value| {
            *captured_c.lock().unwrap() = Some(patch);
            true
        };

        let ready = Arc::new(AtomicBool::new(false));
        let logger = ModLogger::new(|_l, _m| {});

        super::probe_and_configure(
            port,
            &ready,
            &logger,
            &probe,
            &apply,
            &base_url,
            voice.as_deref(),
        );

        assert!(ready.load(Ordering::SeqCst));
        let patched = captured.lock().unwrap().clone();
        assert!(patched.is_some(), "apply_settings payload 应被捕获");
        let patch = patched.unwrap();
        // patch 是完整 JSON：{"__apply_settings":true,"patch":{"tts":{...}}}
        assert_eq!(patch["__apply_settings"], true);
        assert_eq!(
            patch["patch"]["tts"]["base_url"],
            "http://127.0.0.1:8000/v1"
        );
        assert_eq!(patch["patch"]["tts"]["voice"], "af_heart");
    }

    /// HTTP 探测函数单测：无法连接时返回 Err。
    #[test]
    fn http_get_fails_on_unreachable_port() {
        let result = LocalTtsRuntime::http_get(
            "http://127.0.0.1:1/v1",
            "/v1/models",
            Duration::from_millis(500),
        );
        assert!(result.is_err(), "不可达端口应返回 Err");
    }
}
