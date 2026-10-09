//! live2d-ai-mod-local-tts：**拉起外部进程**的 Mod（同一个 crate 导出两个工厂）。
//!
//! | 工厂 | 稳定 id | 界面名 | 引擎 | 缺省声明地址 | 注册 |
//! | --- | --- | --- | --- | --- | --- |
//! | [FACTORY] | local-tts | 本地tts_CosyVoice3-0.5B | Fun-CosyVoice3-0.5B（HF） | 127.0.0.1:8080/v1 | **已封存** |
//! | [MELO_FACTORY] | local-tts-melo | 本地tts_MeloTTS | MeloTTS 中文 | 127.0.0.1:8091/v1 | **缺省启用** |
//!
//! # CosyVoice3 封存（2026-10-09，0.2.3-rc.1）
//!
//! `FACTORY` / [COSYVOICE_ENGINE] / `engine/` **保留在树上**（可编译、可单测），
//! 但它**已移出 `AVAILABLE_MOD_FACTORIES`**：注册面回到 6 个，缺省 manifest 里
//! 没有它，开机也不会拉起。**还要适配**（引擎脚本与权重布局未与本仓对齐），
//! **未适配前不要挂回**。挂回去 = 用户界面出现一个「启用即 Failed」的开关。
//!
//! 现在出厂出声的那一个是 [MELO_FACTORY]（`local-tts-melo`）。
//!
//! 两个工厂共用一套运行时（[LocalTtsRuntime]）：**拉起时机 / argv / 预检 /
//! 快照 / 停止** 一字不差，差别只在 [EngineSpec]（程序、权重、解释器、地址）。
//!
//! # 它是什么 / 不是什么
//!
//! 语音合成仍是主链上的 HTTP 客户端（crates/live2d-ai-runtime/src/tts.rs，
//! POST {base_url}/audio/speech）。这两个 Mod **不写 [tts] 的任何键**
//!（base_url / voice / model / sample_rate / channels / response_format
//! / api_key_env），调用 ModServices.apply_settings 的次数**恒为 0**；
//! 它们只按 argv spawn 一个进程，然后把 pid / 是否在跑 / 退出码如实报进
//! state_json。停用之后，链路继续请求 [tts] 里已经配好的地址。
//!
//! 旧 crate（0.5.0 删除的那个同名 Mod）的「探活 → 改写 base_url」**不恢复**：
//! 见 docs/architecture/tts-is-core.md（文末一节写了本轮边界）。
//!
//! # 挂掉时（维护者原话：不管）
//!
//! 不写看门狗、不在子进程退出后自动拉起、不做健康检查、不把失败改派到别的 URL、
//! 不把 Err 记完日志再返回 Ok。具体：
//!
//! - spawn 前 [LaunchConfig::preflight]：脚本不在 / 权重不在 / 解释器不在 →
//!   start 返回 Err，宿主现有路径把本 Mod 标成 ModStatus::Failed { message }
//!   （不在这里重试、也不换地址）；
//! - **spawn 后子进程立刻退出**（非 0 / 0 / 被信号）→ start Err + Failed，
//!   错误里带脚本 stderr 的尾巴（见上节）；观察失败（try_wait Err）同样 Err；
//! - 该 spawn 时失败 → 同上（Err）；
//! - state_json 只用**非阻塞** try_wait 观察；try_wait 自己出错 → 错误字符串
//!   进快照字段 wait_error，且**不**因此 spawn / 改 [tts] / 把失败写成「在跑」；
//! - launch 已有子进程 → Err（禁止起第二个）；stop 没有子进程 → Err；
//!   未知命令 → ModError::UnsupportedCommand；
//! - shutdown 先停再等；**停止失败时把句柄放回运行时结构**并返回该 Err
//!   （宿主据此把 runtime 放回槽位、不写 enabled=false，见 mod_registry/registry.rs）。
//!
//! # 立刻退出的子进程 = Failed（2026-10-09 修，0.2.3-rc.1）
//!
//! **旧行为**（已改）：spawn 成功就返回 `Ok`，界面显示「已启用」；脚本随后
//! 自己以非 0 退出（venv 建不了 / 依赖装不上 / serve.py 载模型失败）只在
//! state_json 的 `exit_code` 里可见——**界面在说谎**。
//!
//! **现行**：spawn 之后有一个 [`IMMEDIATE_EXIT_GRACE`] 的观察窗口，
//! 子进程在这个窗口内退出 ⇒ `start` 返回 `Err`，宿主把它标成
//! `ModStatus::Failed { message }`，**错误里带脚本印出的原因**
//! （子进程 stderr 的尾巴，见 child.rs）。退出码 0 也算失败：立刻退出的进程
//! 根本没在听端口。观察本身失败（`try_wait` Err）同样 Err 并停掉子进程——
//! 「没法确认它在跑」不能写成「已启用」。
//!
//! **不修**的是 FINDINGS.md 的其余条目（S-000…S-007）：本仓只改这一条。
//!
//! # 缺省启用（0.2.3-rc.1）
//!
//! cli_entry::default_mods_manifest **收录 `local-tts-melo`**（`enabled: true`，
//! `launch_mode = with_app`）：仓库自带 MeloTTS 中文权重（config.json 普通入库、
//! checkpoint.pth 走 Git LFS），克隆后开机就出声。**不收录 CosyVoice3**（已封存）。

mod child;
mod config;
mod spec;

pub use child::{ChildProbe, ChildProcess, SpawnRequest, Spawner, real_spawn};
pub use config::{
    COSYVOICE_ENGINE, COSYVOICE_PROGRAM, COSYVOICE_WEIGHTS_DIR, DEFAULT_BASE_URL, EngineSpec,
    LaunchConfig, LaunchMode, MELO_BASE_URL, MELO_ENGINE, MELO_PROGRAM, MELO_WEIGHTS_DIR,
    parse_args_json, preflight_engine,
};
pub use spec::settings_spec;

use live2d_ai_mod_system::*;

/// 「立刻退出」的观察窗口（2026-10-09）。
///
/// `start` 在 spawn 之后等这么久再判定成败：**窗口内子进程就退出 ⇒ start Err**，
/// 宿主把本 Mod 标 `Failed`，错误里带脚本印出的原因。理由是脚本起不来时
/// （venv 建不了 / 依赖装不上 / serve.py 载模型失败）表现为**很快非 0 退出**；
/// 只报「已启用」就是界面在说谎。
///
/// 取值 1 秒：够覆盖「bash 起来就失败」的全部路径，又不至于拖慢开机明显
/// （缺省只启用一个本地 TTS Mod）。首次装 venv 是分钟级，不会落进窗口。
pub const IMMEDIATE_EXIT_GRACE: std::time::Duration = std::time::Duration::from_millis(1000);

/// 窗口内的轮询间隔。
const SETTLE_POLL: std::time::Duration = std::time::Duration::from_millis(25);

/// CosyVoice3 的 Mod 描述符（静态身份）。界面名是计划书定下的**原文**。
///
/// **2026-10-09 封存（0.2.3-rc.1）**：已移出 `AVAILABLE_MOD_FACTORIES`，
/// 缺省 manifest 不收录，不要挂回（见模块头注）。
pub const DESCRIPTOR: ModDescriptor = ModDescriptor {
    id: "local-tts",
    name: "本地tts_CosyVoice3-0.5B",
    version: "0.1.0",
    api_version: MOD_API_VERSION,
};

/// MeloTTS 的 Mod 描述符。id 用 ASCII（中文/下划线/点不进 id）。
pub const MELO_DESCRIPTOR: ModDescriptor = ModDescriptor {
    id: "local-tts-melo",
    name: "本地tts_MeloTTS",
    version: "0.1.0",
    api_version: MOD_API_VERSION,
};

/// 运行时：持描述符、引擎配方、配置、可注入的 spawner、**至多一个**子进程句柄。
pub struct LocalTtsRuntime {
    services: ModServices,
    descriptor: &'static ModDescriptor,
    engine: EngineSpec,
    config: LaunchConfig,
    /// 当前子进程（None = 没有；**不会有两个**）。
    child: Option<Box<dyn ChildProcess>>,
    spawner: Spawner,
    /// spawn 后判定「是否立刻退出」的观察窗口（生产 = [IMMEDIATE_EXIT_GRACE]）。
    grace: std::time::Duration,
}

impl LocalTtsRuntime {
    /// 生产构造（真实 std::process::Command）：CosyVoice3 引擎。
    pub fn new(services: ModServices, config: LaunchConfig) -> Self {
        Self::with_engine(
            services,
            &DESCRIPTOR,
            COSYVOICE_ENGINE,
            config,
            Box::new(real_spawn),
        )
    }

    /// 指定描述符 + 引擎的生产构造。
    pub fn with_engine(
        services: ModServices,
        descriptor: &'static ModDescriptor,
        engine: EngineSpec,
        config: LaunchConfig,
        spawner: Spawner,
    ) -> Self {
        Self {
            services,
            descriptor,
            engine,
            config,
            child: None,
            spawner,
            grace: IMMEDIATE_EXIT_GRACE,
        }
    }

    /// 注入 spawner 的构造（**单测专用路径**：不 exec 真进程）。CosyVoice3 身份。
    ///
    /// **观察窗口缺省为 0**：单测里假句柄「一直 Running」，等满 1 秒只是白等。
    /// 仍会做**一次** `try_wait`（所以「一出生就已退出」照样能被抓到）；
    /// 要覆盖真实窗口的用例用 [Self::with_immediate_exit_grace] 显式给时长。
    pub fn with_spawner(services: ModServices, config: LaunchConfig, spawner: Spawner) -> Self {
        Self::with_engine(services, &DESCRIPTOR, COSYVOICE_ENGINE, config, spawner)
            .with_immediate_exit_grace(std::time::Duration::ZERO)
    }

    /// 覆盖「立刻退出」的观察窗口（**单测用**；生产一律走 [IMMEDIATE_EXIT_GRACE]）。
    #[must_use]
    pub fn with_immediate_exit_grace(mut self, grace: std::time::Duration) -> Self {
        self.grace = grace;
        self
    }

    /// 拉起一个子进程。**已有子进程时返回 Err**（不覆盖、不起第二个）。
    ///
    /// 预检与非法 args_json 都在这里（而非 create）才报错——用户自带 program 时
    /// 不做预检（见 [LaunchConfig::engine]）。
    fn spawn_child(&mut self) -> Result<(), ModError> {
        if self.child.is_some() {
            return Err(ModError::Other(format!(
                "{} 已有子进程在跑：不重复拉起（先 stop）",
                self.descriptor.id
            )));
        }
        let argv = self.config.argv().map_err(ModError::Other)?;
        self.config.preflight().map_err(ModError::Other)?;
        let request = SpawnRequest {
            argv,
            workdir: self.config.workdir_path(),
        };
        let child = (self.spawner)(&request)
            .map_err(|e| ModError::Other(format!("拉起 {} 的进程失败：{e}", self.descriptor.id)))?;
        self.services.logger.info(&format!(
            "{} 已拉起 pid={:?}",
            self.descriptor.id,
            child.pid()
        ));
        self.child = Some(child);
        // 2026-10-09：立刻退出的子进程**不许**被记成「已启用」——见 [IMMEDIATE_EXIT_GRACE]。
        self.settle_immediately()
    }

    /// spawn 之后的观察窗口：子进程**立刻退出** ⇒ Err（宿主据此标 Failed）。
    ///
    /// 三种结局都写清楚：
    /// - 窗口内退出（非 0 / 0 / 信号）→ 收掉句柄，Err，**带上脚本 stderr 的尾巴**；
    /// - 窗口内一直 Running → Ok（这是正常路径：脚本在装 venv / 载模型）；
    /// - 观察自己失败（try_wait Err）→ **停掉子进程**再 Err：不能确认它在跑，
    ///   就不能对外说「已启用」。
    ///
    /// 窗口为 0 时仍做**一次**探测（单测路径，见 [Self::with_spawner]）。
    fn settle_immediately(&mut self) -> Result<(), ModError> {
        let deadline = std::time::Instant::now() + self.grace;
        loop {
            let Some(child) = self.child.as_mut() else {
                return Ok(());
            };
            match child.try_wait() {
                Ok(ChildProbe::Running) => {
                    if std::time::Instant::now() >= deadline {
                        return Ok(());
                    }
                    std::thread::sleep(SETTLE_POLL);
                }
                Ok(ChildProbe::Exited { code }) => {
                    // 先取原因再丢句柄：try_wait 已经收过尸，drop 不会留僵尸。
                    let reason = self.stderr_reason();
                    let how = match code {
                        Some(0) => "退出码 0".to_string(),
                        Some(n) => format!("退出码 {n}"),
                        None => "被信号终止".to_string(),
                    };
                    self.child = None;
                    return Err(ModError::Other(format!(
                        "{} 的进程立刻退出（{how}）：没有在监听端口，判为 Failed{reason}",
                        self.descriptor.id
                    )));
                }
                Err(e) => {
                    let reason = self.stderr_reason();
                    let stop = self.stop_child();
                    let tail = match stop {
                        Ok(()) => String::new(),
                        Err(err) => format!("；停止该进程也没成功：{err}"),
                    };
                    return Err(ModError::Other(format!(
                        "{} 的进程起来后无法观察（{e}）：不能确认它在跑，判为 Failed{reason}{tail}",
                        self.descriptor.id
                    )));
                }
            }
        }
    }

    /// 子进程 stderr 的尾巴 → 附在 Err 后面的一句（没有就返回空串）。
    ///
    /// **不编**：脚本一个字都没印就是空串，不写「未知错误」冒充原话。
    fn stderr_reason(&self) -> String {
        let Some(child) = self.child.as_ref() else {
            return String::new();
        };
        match child.stderr_tail() {
            Some(text) if !text.trim().is_empty() => {
                format!("；脚本输出：{}", text.trim())
            }
            _ => String::new(),
        }
    }

    /// 停掉并收尸。停止失败 → **把句柄放回**再返回 Err（宿主据此回滚）。
    ///
    /// 继续沿用既有实现：本轮**不**新写一种「wait 失败就把句柄丢掉、对外却像
    /// 停干净了」的路径（计划书对该边界的口径见 FINDINGS.md S-000）。
    fn stop_child(&mut self) -> Result<(), ModError> {
        let Some(mut child) = self.child.take() else {
            return Ok(());
        };
        if let Err(e) = child.stop() {
            self.child = Some(child);
            return Err(ModError::Other(format!(
                "停止 {} 的进程失败：{e}",
                self.descriptor.id
            )));
        }
        child.wait().map_err(|e| {
            ModError::Other(format!("等待 {} 的进程退出失败：{e}", self.descriptor.id))
        })?;
        Ok(())
    }

    /// 只读快照（**非阻塞**）：launch_mode / 声明地址 / pid / 是否在跑 / 退出码。
    fn snapshot(&mut self) -> serde_json::Value {
        let mut out = serde_json::Map::new();
        out.insert(
            "mod_id".to_string(),
            serde_json::Value::String(self.descriptor.id.to_string()),
        );
        out.insert(
            "launch_mode".to_string(),
            serde_json::Value::String(self.config.mode.as_str().to_string()),
        );
        out.insert(
            "base_url".to_string(),
            serde_json::Value::String(self.config.base_url.clone()),
        );
        let Some(child) = self.child.as_mut() else {
            out.insert("pid".to_string(), serde_json::Value::Null);
            out.insert("child_running".to_string(), serde_json::Value::Bool(false));
            out.insert("exit_code".to_string(), serde_json::Value::Null);
            return serde_json::Value::Object(out);
        };
        let pid = child.pid();
        match child.try_wait() {
            Ok(ChildProbe::Running) => {
                out.insert("child_running".to_string(), serde_json::Value::Bool(true));
                out.insert("exit_code".to_string(), serde_json::Value::Null);
            }
            Ok(ChildProbe::Exited { code }) => {
                // 退出码拿不到（被信号终止）时是 null，**不编一个数**。
                out.insert("child_running".to_string(), serde_json::Value::Bool(false));
                out.insert(
                    "exit_code".to_string(),
                    code.map_or(serde_json::Value::Null, |c| {
                        serde_json::Value::Number(c.into())
                    }),
                );
            }
            Err(e) => {
                // 观察失败**不得**改写成「在跑」：child_running=false 表示
                // 「没有拿到仍在跑的确证」，真相在 wait_error。
                out.insert("child_running".to_string(), serde_json::Value::Bool(false));
                out.insert("exit_code".to_string(), serde_json::Value::Null);
                out.insert(
                    "wait_error".to_string(),
                    serde_json::Value::String(e.to_string()),
                );
            }
        }
        if let Some(pid) = pid {
            out.insert("pid".to_string(), serde_json::Value::Number(pid.into()));
        } else {
            out.insert("pid".to_string(), serde_json::Value::Null);
        }
        serde_json::Value::Object(out)
    }
}

impl ModRuntime for LocalTtsRuntime {
    fn start(&mut self, registrar: &mut dyn ModRegistrar) -> Result<(), ModError> {
        registrar.register_settings(spec::settings_spec(self.descriptor, self.engine))?;
        // 唯一读 StartCause 的地方：with_app（缺省）在 Boot/Enable/Apply 都拉起。
        if self
            .config
            .mode
            .spawns_on(live2d_ai_mod_system::start_cause())
        {
            self.spawn_child()?;
        }
        Ok(())
    }

    fn state_json(&mut self) -> Option<serde_json::Value> {
        Some(self.snapshot())
    }

    fn command(
        &mut self,
        command: &str,
        _args: &serde_json::Value,
    ) -> Result<serde_json::Value, ModError> {
        match command {
            "launch" => {
                self.spawn_child()?;
                Ok(serde_json::json!({ "launched": true }))
            }
            "stop" => {
                if self.child.is_none() {
                    return Err(ModError::Other(format!(
                        "{} 没有子进程：无可停止",
                        self.descriptor.id
                    )));
                }
                self.stop_child()?;
                Ok(serde_json::json!({ "stopped": true }))
            }
            other => Err(ModError::UnsupportedCommand {
                command: other.to_string(),
            }),
        }
    }

    fn shutdown(&mut self) -> Result<(), ModError> {
        self.stop_child()
    }
}

/// CosyVoice3 的静态工厂（注册进 AVAILABLE_MOD_FACTORIES 时用 &FACTORY）。
pub struct LocalTtsFactory;

impl ModFactory for LocalTtsFactory {
    fn descriptor(&self) -> &'static ModDescriptor {
        &DESCRIPTOR
    }

    /// 静态 schema：本 Mod **缺省停用**，用户要先能填配置再启用。
    fn settings_spec(&self) -> Option<ModSettingsSpec> {
        Some(spec::settings_spec(&DESCRIPTOR, COSYVOICE_ENGINE))
    }

    fn create(
        &self,
        services: ModServices,
        config: serde_json::Value,
    ) -> Result<Box<dyn ModRuntime>, ModError> {
        let config =
            LaunchConfig::from_json_with_engine(&config, COSYVOICE_ENGINE).map_err(|message| {
                ModError::Init {
                    mod_id: DESCRIPTOR.id.to_string(),
                    message,
                }
            })?;
        Ok(Box::new(LocalTtsRuntime::new(services, config)))
    }
}

/// MeloTTS 的静态工厂（**同一个 crate**；不新增 desktop 依赖边）。
pub struct MeloTtsFactory;

impl ModFactory for MeloTtsFactory {
    fn descriptor(&self) -> &'static ModDescriptor {
        &MELO_DESCRIPTOR
    }

    fn settings_spec(&self) -> Option<ModSettingsSpec> {
        Some(spec::settings_spec(&MELO_DESCRIPTOR, MELO_ENGINE))
    }

    fn create(
        &self,
        services: ModServices,
        config: serde_json::Value,
    ) -> Result<Box<dyn ModRuntime>, ModError> {
        let config =
            LaunchConfig::from_json_with_engine(&config, MELO_ENGINE).map_err(|message| {
                ModError::Init {
                    mod_id: MELO_DESCRIPTOR.id.to_string(),
                    message,
                }
            })?;
        Ok(Box::new(LocalTtsRuntime::with_engine(
            services,
            &MELO_DESCRIPTOR,
            MELO_ENGINE,
            config,
            Box::new(real_spawn),
        )))
    }
}

/// 工厂单例（CosyVoice3）。
pub const FACTORY: LocalTtsFactory = LocalTtsFactory;

/// 工厂单例（MeloTTS）。
pub const MELO_FACTORY: MeloTtsFactory = MeloTtsFactory;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
#[cfg(test)]
#[path = "tests_command.rs"]
mod tests_command;
#[cfg(test)]
#[path = "tests_start_failure.rs"]
mod tests_start_failure;
#[cfg(test)]
#[path = "tests_support.rs"]
mod tests_support;
