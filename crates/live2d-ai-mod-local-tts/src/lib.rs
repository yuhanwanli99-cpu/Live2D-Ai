//! live2d-ai-mod-local-tts（节点 E E5）。
//!
//! v1 进程生命周期管理 Mod：管理本地 TTS 推理进程
//! （kokoroi-rs 或同类 OpenAI 兼容 `/v1/audio/speech` 服务）的
//! spawn / 就绪探测 / 回收 / endpoint 自动探测。
//!
//! # v1 范围声明
//!
//! - **不内置推理**：本 Mod 仅负责 spawn 外部 TTS 进程、探测端口就绪、
//!   回收子进程，并在就绪后自动记录 endpoint；不绑定任何推理后端。
//! - **进程生命周期**：`start`（auto_start）→ spawn + 后台就绪探测线程；
//!   `shutdown` → `child.kill()` + `wait()` 回收 + 复位。
//! - **配置**：`command` / `args` / `port` / `voice` / `auto_start` / `health_timeout_ms`，
//!   均有缺省值。
//! - **事件**：订阅 `VoiceStarted`（v1 仅记录日志）。
//!
//! # 配置字段
//!
//! - `command`（String，缺省 `"kokoroi-rs"`）：TTS 服务可执行命令。
//! - `args`（String 数组，缺省 `["serve","--port","8000"]`）：启动参数
//!   （示例；真实参数以后端为准）。
//! - `port`（u16，缺省 `8000`）：探测就绪的 TCP 端口。
//! - `voice`（String，缺省 `"af_heart"`）：Kokoro 声线 id。
//! - `auto_start`（bool，缺省 `true`）：start 时自动 spawn。
//! - `health_timeout_ms`（u64，缺省 `15000`）：就绪探测超时。
//!
//! # 版本
//!
//! `api_version` 声明为 1（对齐 `live2d-ai-mod-system` API 版本）。

use live2d_ai_mod_system::*;
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// 默认配置常量（与 `config` 字段缺省一致）。
const DEFAULT_COMMAND: &str = "kokoroi-rs";
const DEFAULT_ARGS: &[&str] = &["serve", "--port", "8000"];
const DEFAULT_PORT: u16 = 8000;
#[allow(dead_code)]
const DEFAULT_VOICE: &str = "af_heart";
const DEFAULT_HEALTH_TIMEOUT_MS: u64 = 15000;

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

    #[allow(dead_code)]
    fn voice(&self) -> String {
        self.config
            .get("voice")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(DEFAULT_VOICE)
            .to_string()
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

    /// spawn 子进程 + 后台就绪探测线程（F8 方式：TcpStream::connect_timeout）。
    fn spawn_and_probe(&mut self) -> Result<(), ModError> {
        let command = self.command();
        let args = self.args();
        let port = self.port();
        let timeout = self.health_timeout_ms();
        let ready = self.ready.clone();
        let child_shared = self.child.clone();
        let logger = self.services.logger.clone();

        let child = std::process::Command::new(&command)
            .args(&args)
            .spawn()
            .map_err(|e| {
                let msg = format!("spawn TTS 进程失败 ({command} {args:?}): {e}");
                self.last_error = Some(msg.clone());
                ModError::Init {
                    mod_id: "local-tts".to_string(),
                    message: msg,
                }
            })?;

        // 存入共享句柄。
        {
            let mut guard = self.child.lock().unwrap();
            *guard = Some(child);
        }

        // 后台线程探测端口就绪（不阻塞 start）。
        std::thread::spawn(move || {
            let addr = format!("127.0.0.1:{port}");
            let parsed = addr
                .parse()
                .unwrap_or_else(|_| "127.0.0.1:8000".parse().unwrap());
            let deadline = Instant::now() + timeout;
            loop {
                let res = TcpStream::connect_timeout(&parsed, Duration::from_millis(500));
                if res.is_ok() {
                    ready.store(true, Ordering::SeqCst);
                    logger.info("local-tts 服务就绪");
                    return;
                }
                if Instant::now() >= deadline {
                    logger.warn("local-tts 服务就绪探测超时");
                    // 超时未就绪，杀掉子进程。
                    let mut guard = child_shared.lock().unwrap();
                    if let Some(mut child) = guard.take() {
                        let _ = child.kill();
                        let _ = child.wait();
                    }
                    return;
                }
                std::thread::sleep(Duration::from_millis(500));
            }
        });

        Ok(())
    }

    /// 是否就绪（端口探测成功）。
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
        // 4 个字段：command String / port Number 1024..65535 / voice String / auto_start Bool。
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
        // 1 个 settings spec，4 个字段。
        assert_eq!(mock.specs.len(), 1);
        let spec = &mock.specs[0];
        assert_eq!(spec.mod_id, "local-tts");
        assert_eq!(spec.fields.len(), 4);
        assert_eq!(spec.fields[0].key(), "command");
        assert_eq!(spec.fields[1].key(), "port");
        assert_eq!(spec.fields[2].key(), "voice");
        assert_eq!(spec.fields[3].key(), "auto_start");
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
        assert_eq!(rt.voice(), DEFAULT_VOICE);
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
}
