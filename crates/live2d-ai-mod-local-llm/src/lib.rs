//! live2d-ai-mod-local-llm（节点 E E5，P1 第一个 Mod）。
//!
//! # ⚠️ 已废除启动（0.2.0-rc.1，2026-09-14）— DEPRECATED，勿在新代码引用
//!
//! 本 crate **已移出 `AVAILABLE_MOD_FACTORIES`**，不再注册进 Mod 运行时、不再
//! 编译进 `live2d-ai-desktop` binary。理由：默认 `external-input` 之后，「本地推理
//! 进程管理 / 探活 / 自动写 base_url」不再是产品路径——LLM 端点由
//! `live2d-ai.toml` 的 `[llm]` 显式配置，用户自己的 OpenAI 兼容服务直接填
//! `base_url` 即可，不需要一个替用户 spawn 进程的中间层。
//!
//! 保留 crate 只是**避免一次性大爆炸**（计划原文 `PLAN-0.2.0-rc1`：crate 可暂留
//! 仓库不删）。`cargo test --workspace` 仍会编译/测试本 crate；但**禁止**把它
//! 挂回 `main.rs::AVAILABLE_MOD_FACTORIES`（`mod_count_is_three` 会红）。
//! 下面的 v1 范围声明是**历史记录**，描述的是废除前的行为。
//!
//! # v1 范围声明（历史）
//!
//! 本 Mod 管理**本地 LLM 推理进程的生命周期**（spawn / 健康检查 / 就绪探测 / 回收），
//! **不内置推理**。推理后端为 ollama 或 llama-server 等 OpenAI 兼容端点的子进程；
//! 真实模型执行由用户在外部拉起，本 Mod 仅做管理 + 就绪探测 + 自动记录 base_url。
//!
//! # 职责
//!
//! - `start`：注册 settings schema，订阅 `ModelActivated`，
//!   `auto_start` 时 spawn 子进程 + 后台就绪探测线程。
//! - spawn 失败**不致 `start` 失败**（置 `last_error`，`ready=false`，
//!   不 panic——无命令时不能崩）。
//! - 就绪探测：TCP 端口通后进行真实 HTTP 探测（`/api/tags` for ollama /
//!   `/v1/models` for OpenAI 兼容）；探测通过 → 就绪 + 自动写回 settings。
//! - **P0-4 真实闭环（rc.4 M4 起走一等 API）**：就绪后通过
//!   `ModServices.apply_settings` 提交 namespaced patch，host 复用 settings PATCH
//!   内核写盘 + `supervisor.reload()` —— 形成「spawn → 探活 → 写配置 → 热重载」闭环。
//! - `shutdown`：回收子进程（`kill()` + `wait()`），复位状态。
//!
//! # 配置字段
//!
//! - `command`（String，缺省 `"ollama"`）：推理进程命令。
//! - `args`（String 数组，缺省 `["serve"]`）：启动参数。
//! - `port`（u16，缺省 `11434`）：就绪探测端口。
//! - `model`（String，缺省空）：模型名；**只有显式配置才写回 settings**（不覆盖 toml 选择）。
//! - `base_path`（String，可选）：工作目录。
//! - `auto_start`（bool，缺省 `true`）：start 时自动 spawn。
//! - `health_timeout_ms`（u64，缺省 `15000`）：就绪探测超时。
//! - `externally_managed`（bool，缺省 `true`）：为 `true` 时不 spawn 子进程，
//!   仅探活 + 写 settings（用户在外部启动 Windows Ollama 等服务）。
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
const DEFAULT_COMMAND: &str = "ollama";
const DEFAULT_ARGS: &[&str] = &["serve"];
const DEFAULT_PORT: u16 = 11434;
const DEFAULT_HEALTH_TIMEOUT_MS: u64 = 15000;
/// 缺省**外部管理**：只探活 + 写 settings，不 spawn 子进程。
///
/// 2026-09-10 点火轮：本地实现由用户自带（任何 OpenAI 兼容端点），
/// Mod 不再默认去 spawn 一个可能并不存在的进程。
const DEFAULT_EXTERNALLY_MANAGED: bool = true;

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

/// local-llm 的 settings schema（**静态**；factory 与 runtime.start 共用同一份）。
///
/// 7 字段，纯数据——前端按类型渲染，Mod 不注入 HTML/JS。
fn local_llm_settings_spec() -> ModSettingsSpec {
    ModSettingsSpec {
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
            ModSettingField::Bool {
                key: "externally_managed".to_string(),
                label: "外部管理（不 spawn 子进程，仅探活 + 写 settings）".to_string(),
                default: false,
            },
        ],
    }
}

/// local-llm 工厂。
pub struct LocalLlmFactory;

impl ModFactory for LocalLlmFactory {
    fn descriptor(&self) -> &'static ModDescriptor {
        &DESCRIPTOR
    }

    /// M2：未启用也能拿到 schema（见 trait 文档）。
    fn settings_spec(&self) -> Option<ModSettingsSpec> {
        Some(local_llm_settings_spec())
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

    /// Mod 配置里**显式**给出的模型名；空/缺失 = 不写回 settings。
    ///
    /// 只写用户明确配置的值，避免 Mod 覆盖 `live2d-ai.toml` 里既有的模型选择
    /// （用户自带实现时，模型名应由用户决定）。
    fn model_explicit(&self) -> Option<String> {
        self.config
            .get("model")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
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
    /// （用户在外部启动服务，如 Windows Ollama）。
    ///
    /// spawn 失败**不返回 Err**（仅记录 `last_error`），符合 v1 约定：
    /// 无命令时不能 crash。
    fn spawn_and_probe(&mut self) {
        let command = self.command();
        let args = self.args();
        let port = self.port();
        let timeout = self.health_timeout_ms();
        let base_path = self.base_path();

        if !self.externally_managed() {
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
                }
                Err(e) => {
                    let msg = format!(
                        "spawn local-llm 推理进程失败 (command={}, args={:?}): {}",
                        command, args, e
                    );
                    self.last_error = Some(msg.clone());
                    self.services.logger.error(&msg);
                    // 即使 spawn 失败仍探测端口（服务可能已在外部运行）。
                }
            }
        } else {
            self.services
                .logger
                .info("local-llm externally_managed=true，跳过 spawn，仅探活 + 写 settings");
        }

        self.start_probe_thread(port, timeout);
    }

    /// 后台线程探测 TCP 端口就绪 + 真实 HTTP 探测，直至超时。
    ///
    /// 探测策略：
    /// 1. TCP `connect_timeout` 直至端口开放；
    /// 2. TCP 通后立即做真实 HTTP GET（手写 HTTP/1.1，30 行 std）探测 API 就绪：
    ///    ollama 用 `/api/tags`，OpenAI 兼容用 `/v1/models`；
    /// 3. HTTP 返回 2xx → `ready=true` + 通过 `services.apply_settings` 提交
    ///    `{"llm":{"base_url":...,"model":...}}` 触发 host 写盘 + reload（P0-4 闭环）；
    /// 4. HTTP 探测失败 → `ready=false`（TCP 通但 API 不通 = 未就绪）。
    ///
    /// `externally_managed=true` 时同样探测 —— 用户自己起服务，Mod 探活 + 写配置。
    fn start_probe_thread(&self, port: u16, timeout: Duration) {
        let ready = self.ready.clone();
        let logger = self.services.logger.clone();
        let apply_settings = self.services.apply_settings.clone();
        let model = self.model_explicit();
        let base_url = format!("http://127.0.0.1:{port}/v1");
        let socket_addr: std::net::SocketAddr = format!("127.0.0.1:{port}")
            .parse()
            .unwrap_or_else(|_| "127.0.0.1:11434".parse().unwrap());

        std::thread::spawn(move || {
            let deadline = Instant::now() + timeout;
            // 阶段 1：TCP 端口探活。
            loop {
                if TcpStream::connect_timeout(&socket_addr, Duration::from_millis(500)).is_ok() {
                    break;
                }
                if Instant::now() >= deadline {
                    logger.warn("local-llm 服务就绪探测超时");
                    return;
                }
                std::thread::sleep(Duration::from_millis(500));
            }
            logger.info("local-llm TCP 端口已通，开始 HTTP 探测");

            // 阶段 2：真实 HTTP 探测 —— 手写 HTTP/1.1 GET。
            // 优先尝试 ollama 端点 `/api/tags`，失败则尝试 OpenAI 兼容 `/v1/models`。
            let probe = |_port: u16| {
                Self::http_get(&base_url, "/api/tags", Duration::from_secs(2))
                    .or_else(|_| Self::http_get(&base_url, "/v1/models", Duration::from_secs(2)))
            };
            // rc.4 M4：一等配置写回（原 `event_tx` + `__apply_settings` 走私已删）。
            let apply = |patch: serde_json::Value| apply_settings.apply(patch);
            probe_and_configure(
                port,
                &ready,
                &logger,
                &probe,
                &apply,
                &base_url,
                model.as_deref(),
            );
        });
    }

    /// 手写超轻量 HTTP/1.1 GET —— 纯 std，无额外依赖。
    ///
    /// 返回 `Ok(())` 若收到 2xx 状态码，`Err` 否则。
    /// 用途：local-llm Mod 的就绪探测（ollama `/api/tags` 或 OpenAI `/v1/models`），
    /// 避免引入 reqwest 等 heavy 依赖到 mod crate。
    fn http_get(base_url: &str, path: &str, timeout: Duration) -> Result<(), String> {
        // 解析 host:port。
        let host = base_url
            .trim_start_matches("http://")
            .trim_start_matches("https://")
            .trim_end_matches('/')
            .split('/')
            .next()
            .unwrap_or("127.0.0.1:11434");
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
}

/// 探测 + 写 settings 的纯逻辑函数（P0-4 真实闭环）。
///
/// `probe`：探测函数（生产路径为 `http_get`，测试路径为 mock）。
/// `apply`：settings patch 应用函数（生产路径为 `services.apply_settings`，
/// 测试路径为 mock）。
///
/// 探测成功 → `ready=true` + 构造 namespaced patch `{"llm":{...}}`，
/// 交给 `apply` 回调完成写盘 + reload（rc.4 M4 一等 API）。
/// 探测失败 → `ready=false`，记录日志。
fn probe_and_configure(
    port: u16,
    ready: &Arc<AtomicBool>,
    logger: &ModLogger,
    probe: &dyn Fn(u16) -> Result<(), String>,
    apply: &dyn Fn(serde_json::Value) -> bool,
    base_url: &str,
    model: Option<&str>,
) {
    match probe(port) {
        Ok(()) => {
            ready.store(true, Ordering::SeqCst);
            logger.info("local-llm 探测通过，服务就绪");
            // `base_url` 必写（这就是 Mod 的全部价值：把端点接到主链路）；
            // `model` 仅在 Mod 显式配置时写，避免覆盖用户在 toml 里的选择。
            let mut llm = serde_json::json!({ "base_url": base_url });
            if let Some(m) = model {
                llm["model"] = serde_json::Value::String(m.to_string());
            }
            // rc.4 M4：一等 API——直接就是 namespaced patch，不再包 `__apply_settings` 信封。
            let patch = serde_json::json!({ "llm": llm });
            if !apply(patch) {
                logger.warn("local-llm apply_settings 调用失败");
            }
        }
        Err(e) => {
            ready.store(false, Ordering::SeqCst);
            let msg = format!("local-llm 探测失败: {}", e);
            logger.warn(&msg);
        }
    }
}

impl LocalLlmRuntime {
    /// Whether the service is ready (HTTP probe passed).
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::SeqCst)
    }

    /// Most recent error.
    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    /// Current config.
    pub fn config(&self) -> &serde_json::Value {
        &self.config
    }
}

impl ModRuntime for LocalLlmRuntime {
    fn start(&mut self, registrar: &mut dyn ModRegistrar) -> Result<(), ModError> {
        // 注册 settings schema（与 factory.settings_spec() 同源：静态 schema）。
        // 未启用时 host 也能从 factory 拿到同一份（M2：开关还关着也能渲染表单）。
        registrar.register_settings(local_llm_settings_spec())?;

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
        // 1 个 settings spec，7 个字段。
        assert_eq!(mock.specs.len(), 1);
        let spec = &mock.specs[0];
        assert_eq!(spec.mod_id, "local-llm");
        assert_eq!(spec.fields.len(), 7);
        assert_eq!(spec.fields[0].key(), "command");
        assert_eq!(spec.fields[1].key(), "args");
        assert_eq!(spec.fields[2].key(), "port");
        assert_eq!(spec.fields[3].key(), "model");
        assert_eq!(spec.fields[4].key(), "auto_start");
        assert_eq!(spec.fields[5].key(), "health_timeout_ms");
        assert_eq!(spec.fields[6].key(), "externally_managed");
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
        assert_eq!(rt.model_explicit(), None);
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
        assert_eq!(rt.model_explicit(), None);
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

    #[test]
    fn externally_managed_defaults_true() {
        let rt = LocalLlmRuntime {
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

    /// externally_managed=true 时不 spawn 子进程，但探测线程仍会启动
    /// （探测 + 写 settings 逻辑由 probe 完成）。本测试通过
    /// `spawn_and_probe` 间接验证：child 槽位保持 None（未 spawn），
    /// 但不会 panic。
    #[test]
    fn externally_managed_true_does_not_spawn() {
        let mut rt = LocalLlmRuntime {
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
        rt.spawn_and_probe();
        // child 槽位仍为空（externally_managed 模式下不 spawn）。
        assert!(
            rt.child.lock().unwrap().is_none(),
            "externally_managed=true 不应 spawn 子进程"
        );
    }

    /// HTTP 探测函数单测：无法连接时返回 Err。
    #[test]
    fn http_get_fails_on_unreachable_port() {
        // 端口 1 上几乎不会有服务。
        let result = LocalLlmRuntime::http_get(
            "http://127.0.0.1:1/v1",
            "/v1/models",
            Duration::from_millis(500),
        );
        assert!(result.is_err(), "不可达端口应返回 Err");
    }

    /// probe_and_configure 逻辑分离测试：mock probe + mock apply，
    /// 验证就绪时发送正确的 settings patch payload。
    #[test]
    fn probe_and_configure_sends_correct_patch_on_ready() {
        let ready = Arc::new(AtomicBool::new(false));
        let captured: Arc<Mutex<Option<serde_json::Value>>> = Arc::new(Mutex::new(None));
        let captured_c = captured.clone();

        // mock probe：直返回 Ok（模拟服务已就绪）。
        let probe = |_port: u16| Ok(());
        // mock apply：捕获发出的 patch。
        let apply = move |patch: serde_json::Value| {
            *captured_c.lock().unwrap() = Some(patch);
            true
        };

        let base_url = "http://127.0.0.1:11434/v1";
        let model = Some("qwen3:4b");
        let logger = ModLogger::new(|_l, _m| {});

        super::probe_and_configure(11434, &ready, &logger, &probe, &apply, base_url, model);

        // 验证 ready 被置位。
        assert!(ready.load(Ordering::SeqCst), "探测成功后 ready 应为 true");

        // 验证 patch payload 含正确的 base_url + model。
        let captured_val = captured.lock().unwrap().take();
        assert!(captured_val.is_some(), "apply_settings payload 应被捕获");
        let patch = captured_val.unwrap();
        // rc.4 M4：patch 就是 namespaced JSON（无 `__apply_settings` 信封）。
        assert_eq!(patch["llm"]["base_url"], "http://127.0.0.1:11434/v1");
        assert_eq!(patch["llm"]["model"], "qwen3:4b");
    }

    /// probe_and_configure：探测失败时 ready 保持 false，不发送 patch。
    #[test]
    fn probe_and_configure_on_failure_does_not_send_patch() {
        let ready = Arc::new(AtomicBool::new(false));
        let captured: Arc<Mutex<Option<serde_json::Value>>> = Arc::new(Mutex::new(None));
        let captured_c = captured.clone();

        // mock probe：返回 Err（模拟服务未就绪）。
        let probe = |_port: u16| Err("模拟探测失败".to_string());
        // mock apply：不应被调用。
        let apply = move |_patch: serde_json::Value| {
            *captured_c.lock().unwrap() = Some(_patch);
            true
        };

        let logger = ModLogger::new(|_l, _m| {});

        super::probe_and_configure(
            11434,
            &ready,
            &logger,
            &probe,
            &apply,
            "http://127.0.0.1:11434/v1",
            Some("qwen3:4b"),
        );

        // 验证 ready 保持 false，apply 不被调用。
        assert!(!ready.load(Ordering::SeqCst), "探测失败后 ready 应为 false");
        assert!(captured.lock().unwrap().is_none(), "探测失败不应发送 patch");
    }
}
