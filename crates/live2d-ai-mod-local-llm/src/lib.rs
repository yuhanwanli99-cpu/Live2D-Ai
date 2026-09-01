//! live2d-ai-mod-local-llm（节点 E E5，P1 第一个 Mod）。
//!
//! # v1 范围声明
//!
//! 本 Mod 管理**本地 LLM 推理进程的生命周期**（spawn / 健康检查 / 就绪探测 / 回收），
//! **不内置推理**。推理后端为 ollama 或 llama-server 等 OpenAI 兼容端点的子进程；
//! 真实模型执行由用户在外部拉起，本 Mod 仅做管理 + 就绪探测 + 自动记录 base_url。
//!
//! # 职责
//!
//! - `start`：注册 settings schema（6 字段），订阅 `ModelActivated`，
//!   `auto_start` 时 spawn 子进程 + 后台就绪探测线程。
//! - spawn 失败**不致 `start` 失败**（置 `last_error`，`ready=false`，
//!   不 panic——无命令时不能崩）。
//! - 就绪探测：轮询 `127.0.0.1:{port}` TCP 端口直至 `health_timeout_ms` 内通。
//! - `shutdown`：回收子进程（`kill()` + `wait()`），复位状态。
//!
//! # 配置字段
//!
//! - `command`（String，缺省 `"ollama"`）：推理进程命令。
//! - `args`（String 数组，缺省 `["serve"]`）：启动参数。
//! - `port`（u16，缺省 `11434`）：就绪探测端口。
//! - `model`（String，缺省 `"qwen3.5:4b"`）：模型名称。
//! - `base_path`（String，可选）：工作目录。
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
const DEFAULT_COMMAND: &str = "ollama";
const DEFAULT_ARGS: &[&str] = &["serve"];
const DEFAULT_PORT: u16 = 11434;
#[allow(dead_code)]
const DEFAULT_MODEL: &str = "qwen3.5:4b";
const DEFAULT_HEALTH_TIMEOUT_MS: u64 = 15000;

/// Mod 描述符（静态身份）。
const DESCRIPTOR: ModDescriptor = ModDescriptor {
    id: "local-llm",
    name: "本地大模型",
    version: "0.1.0",
    api_version: 1,
};

/// local-llm 运行时状态。
pub struct LocalLlmRuntime {
    services: ModServices,
    /// 当前生效配置（host 传入的 namespaced JSON）。
    config: serde_json::Value,
    /// settings schema 是否已注册（start 成功标志）。
    registered: bool,
    /// 子进程句柄（跨线程共享，spawn 写入 + shutdown 回收）。
    child: Arc<Mutex<Option<std::process::Child>>>,
    /// 端口就绪探测结果（就绪 = true）。
    ready: Arc<AtomicBool>,
    /// 最近一次错误原因。
    last_error: Option<String>,
}

impl Default for LocalLlmRuntime {
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

/// local-llm 工厂。
pub struct LocalLlmFactory;

impl ModFactory for LocalLlmFactory {
    fn descriptor(&self) -> &'static ModDescriptor {
        &DESCRIPTOR
    }

    fn create(
        &self,
        services: ModServices,
        config: serde_json::Value,
    ) -> Result<Box<dyn ModRuntime>, ModError> {
        Ok(Box::new(LocalLlmRuntime {
            services,
            config,
            registered: false,
            child: Arc::new(Mutex::new(None)),
            ready: Arc::new(AtomicBool::new(false)),
            last_error: None,
        }))
    }
}

impl LocalLlmRuntime {
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
            .and_then(|v| u16::try_from(v).ok())
            .unwrap_or(DEFAULT_PORT)
    }

    #[allow(dead_code)]
    fn model(&self) -> String {
        self.config
            .get("model")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(DEFAULT_MODEL)
            .to_string()
    }

    fn base_path(&self) -> Option<String> {
        self.config
            .get("base_path")
            .and_then(serde_json::Value::as_str)
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

    /// spawn 子进程 + 后台就绪探测线程（F8 方式：TcpStream::connect_timeout）。
    ///
    /// spawn 失败**不返回 Err**（仅记录 `last_error`），符合 v1 约定：
    /// 无命令时不能 crash。
    fn spawn_and_probe(&mut self) {
        let command = self.command();
        let args = self.args();
        let port = self.port();
        let timeout = self.health_timeout_ms();
        let base_path = self.base_path();

        let mut cmd = std::process::Command::new(&command);
        cmd.args(&args);
        if let Some(bp) = &base_path {
            cmd.current_dir(bp);
        }

        match cmd.spawn() {
            Ok(child) => {
                let pid = child.id();
                {
                    let mut guard = self.child.lock().unwrap();
                    *guard = Some(child);
                }
                self.services.logger.info(&format!(
                    "local-llm 推理进程已 spawn (command={}, pid={}, port={})",
                    command, pid, port
                ));
                self.start_probe_thread(port, timeout);
            }
            Err(e) => {
                let msg = format!(
                    "spawn local-llm 推理进程失败 (command={}, args={:?}): {}",
                    command, args, e
                );
                self.last_error = Some(msg.clone());
                self.services.logger.error(&msg);
            }
        }
    }

    /// 后台线程探测 TCP 端口就绪，直至超时。
    fn start_probe_thread(&self, port: u16, timeout: Duration) {
        let ready = self.ready.clone();
        let logger = self.services.logger.clone();
        let socket_addr: std::net::SocketAddr = format!("127.0.0.1:{port}")
            .parse()
            .unwrap_or_else(|_| "127.0.0.1:11434".parse().unwrap());

        std::thread::spawn(move || {
            let deadline = Instant::now() + timeout;
            loop {
                match TcpStream::connect_timeout(&socket_addr, Duration::from_millis(500)) {
                    Ok(_) => {
                        ready.store(true, Ordering::SeqCst);
                        logger.info("local-llm 服务就绪");
                        return;
                    }
                    Err(_) => {
                        if Instant::now() >= deadline {
                            logger.warn("local-llm 服务就绪探测超时");
                            return;
                        }
                        std::thread::sleep(Duration::from_millis(500));
                    }
                }
            }
        });
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

impl ModRuntime for LocalLlmRuntime {
    fn start(&mut self, registrar: &mut dyn ModRegistrar) -> Result<(), ModError> {
        // 注册 settings schema（前端统一渲染 “本地大模型” 面板）：6 字段。
        let spec = ModSettingsSpec {
            mod_id: "local-llm".to_string(),
            title: "本地大模型".to_string(),
            version: 1,
            fields: vec![
                ModSettingField::String {
                    key: "command".to_string(),
                    label: "推理进程命令".to_string(),
                    secret: false,
                },
                ModSettingField::String {
                    key: "args".to_string(),
                    label: "启动参数（JSON 数组字符串）".to_string(),
                    secret: false,
                },
                ModSettingField::Number {
                    key: "port".to_string(),
                    label: "服务端口".to_string(),
                    min: 1024.0,
                    max: 65535.0,
                },
                ModSettingField::String {
                    key: "model".to_string(),
                    label: "模型名称".to_string(),
                    secret: false,
                },
                ModSettingField::Bool {
                    key: "auto_start".to_string(),
                    label: "自动启动推理进程".to_string(),
                    default: true,
                },
                ModSettingField::Number {
                    key: "health_timeout_ms".to_string(),
                    label: "就绪探测超时（毫秒）".to_string(),
                    min: 1000.0,
                    max: 60000.0,
                },
            ],
        };
        registrar.register_settings(spec)?;

        // 订阅 ModelActivated（v1 仅记录日志）。
        registrar.subscribe(ModEventTopic::ModelActivated)?;

        self.registered = true;

        // auto_start → spawn + 后台就绪探测。
        if self.auto_start() {
            self.spawn_and_probe();
        }

        self.services.logger.info("local-llm Mod 已启动");
        Ok(())
    }

    fn on_event(&mut self, topic: ModEventTopic, payload: &str) -> Result<(), ModError> {
        if let ModEventTopic::ModelActivated = topic {
            self.services
                .logger
                .info(&format!("local-llm 收到 ModelActivated: {}", payload));
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
        self.services.logger.info("local-llm Mod 已关闭");
        Ok(())
    }
}

/// 供 host 装配 `AVAILABLE_MOD_FACTORIES` 的工厂单例。
pub static FACTORY: LocalLlmFactory = LocalLlmFactory;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::Mutex;

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
        let d = FACTORY.descriptor();
        assert_eq!(d.id, "local-llm");
        assert_eq!(d.name, "本地大模型");
        assert_eq!(d.version, "0.1.0");
        assert_eq!(d.api_version, 1);
    }

    #[test]
    fn start_registers_spec_and_subscription() {
        let config = serde_json::json!({ "auto_start": false });
        let mut rt = FACTORY.create(test_services(), config).unwrap();
        let mut mock = MockRegistrar::default();
        rt.start(&mut mock).unwrap();
        // 1 个 settings spec，6 个字段。
        assert_eq!(mock.specs.len(), 1);
        let spec = &mock.specs[0];
        assert_eq!(spec.mod_id, "local-llm");
        assert_eq!(spec.fields.len(), 6);
        assert_eq!(spec.fields[0].key(), "command");
        assert_eq!(spec.fields[1].key(), "args");
        assert_eq!(spec.fields[2].key(), "port");
        assert_eq!(spec.fields[3].key(), "model");
        assert_eq!(spec.fields[4].key(), "auto_start");
        assert_eq!(spec.fields[5].key(), "health_timeout_ms");
        // 校验 Number 字段的范围（枚举字段需模式匹配访问）。
        let port_field = match &spec.fields[2] {
            ModSettingField::Number { min, max, .. } => (*min, *max),
            _ => (0.0, 0.0),
        };
        assert_eq!(port_field, (1024.0, 65535.0));
        let timeout_field = match &spec.fields[5] {
            ModSettingField::Number { min, max, .. } => (*min, *max),
            _ => (0.0, 0.0),
        };
        assert_eq!(timeout_field, (1000.0, 60000.0));
        // 1 个订阅：ModelActivated。
        assert_eq!(mock.subs.len(), 1);
    }

    #[test]
    fn config_defaults() {
        let rt = LocalLlmRuntime {
            services: test_services(),
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
        assert_eq!(rt.model(), DEFAULT_MODEL);
        assert!(rt.auto_start());
        assert_eq!(
            rt.health_timeout_ms(),
            Duration::from_millis(DEFAULT_HEALTH_TIMEOUT_MS)
        );
    }

    #[test]
    fn auto_start_false_no_spawn() {
        // 直接构造具体类型，便于访问私有字段断言。
        let mut rt = LocalLlmRuntime {
            services: test_services(),
            config: serde_json::json!({ "auto_start": false }),
            registered: false,
            child: Arc::new(Mutex::new(None)),
            ready: Arc::new(AtomicBool::new(false)),
            last_error: None,
        };
        let mut mock = MockRegistrar::default();
        // auto_start=false → 不 spawn，不 panic。
        rt.start(&mut mock).unwrap();
        assert!(!rt.is_ready());
        assert!(rt.child.lock().unwrap().is_none());
    }

    #[test]
    fn invalid_config_falls_back_without_panic() {
        let rt = LocalLlmRuntime {
            services: test_services(),
            config: serde_json::json!({
                "command": null,
                "args": "not-an-array",
                "port": 70000,
                "model": null,
                "auto_start": "yes",
                "health_timeout_ms": -1,
            }),
            registered: false,
            child: Arc::new(Mutex::new(None)),
            ready: Arc::new(AtomicBool::new(false)),
            last_error: None,
        };
        // 非法/缺失值 → 缺省，不 panic。
        assert_eq!(rt.command(), DEFAULT_COMMAND);
        assert_eq!(rt.port(), DEFAULT_PORT);
        assert_eq!(rt.model(), DEFAULT_MODEL);
        assert!(rt.auto_start());
        assert_eq!(
            rt.args(),
            DEFAULT_ARGS
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            rt.health_timeout_ms(),
            Duration::from_millis(DEFAULT_HEALTH_TIMEOUT_MS)
        );
    }
}
