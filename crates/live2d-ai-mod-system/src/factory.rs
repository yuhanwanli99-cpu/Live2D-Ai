//! Mod 工厂与运行时（E0 ADR §1：工厂/实例分离，`start` 返回 `Result`）。

use crate::descriptor::ModDescriptor;
use crate::error::ModError;
use crate::registry::ModRegistrar;
use crate::services::ModServices;

/// Mod 运行时请求返回给 host 的动作（host 决定是否执行）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModAction {
    /// 无。
    None,
    /// 请求重启本 Mod 实例（host 决定）。
    RequestRestart,
}

/// Mod 工厂（静态注册进 host 的 `AVAILABLE_MOD_FACTORIES`）。
pub trait ModFactory: Send + Sync {
    /// 静态描述符。
    fn descriptor(&self) -> &'static ModDescriptor;

    /// **静态**设置 schema（rc.4 M2）。
    ///
    /// 与 `ModRuntime::start` 里 `register_settings` 的差别只有一个：
    /// **未启用也能拿到**。前端要在「开关还关着」时就渲染配置表单
    /// （尤其是缺省停用的 Mod：先让用户填好再启用），而 `start` 有副作用
    /// （spawn / 写配置），不能为了拿 schema 去跑它。
    ///
    /// 缺省 `None` = 沿用运行时注册（旧 Mod 不必改）；两者都提供时以
    /// 运行时注册为准（后者可携带动态字段）。
    fn settings_spec(&self) -> Option<crate::settings::ModSettingsSpec> {
        None
    }

    /// 构造一个 Mod 实例。
    ///
    /// `config` = 该 Mod 的 namespaced JSON 配置（`~/.config/live2d-ai/mods/<id>.json`
    /// 或主配置 `[mods.<id>]` 段解析结果）。
    fn create(
        &self,
        services: ModServices,
        config: serde_json::Value,
    ) -> Result<Box<dyn ModRuntime>, ModError>;
}

/// Mod 运行时实例（host 持有，`start` 注册能力，`shutdown` 收尾）。
pub trait ModRuntime: Send {
    /// 注册能力（settings schema / 事件订阅）。
    ///
    /// 返回 `Err` 表示注册失败 → host 置 `ModStatus::Failed` 并 disable。
    fn start(&mut self, registrar: &mut dyn ModRegistrar) -> Result<(), ModError>;

    /// 处理一次事件（由 host 的独立 worker 调用）。
    ///
    /// 返回 `Err` → host 记录 last_error 并停止投递（`ModStatus::Failed`）。
    fn on_event(
        &mut self,
        topic: crate::topics::ModEventTopic,
        payload: &str,
    ) -> Result<(), ModError> {
        let _ = (topic, payload);
        Ok(())
    }

    /// **带会话事件**（L1 基座，2026-09-15）。
    ///
    /// 与 Self::on_event 完全同义，多一个「本轮属于哪个会话」的参数。
    /// 缺省实现**原样转发**给 Self::on_event，因此：
    /// - 不关心会话的 Mod（director / voice-input / external-input）一行都不用改；
    /// - 只关心会话的 Mod（persona / memory）覆写本方法即可，不再需要
    ///   「把 payload 改成 JSON」这种破坏性协议升级。
    ///
    /// 语义约定：
    /// - session = Some(id)：该 id **已通过** sanitize_session_id（宿主归一化后再投递）；
    /// - session = None：调用方没带会话（裸 HTTP / 终端壳）；Mod 必须按
    ///   「全局桶」处理，**不得**假装它属于某个会话。
    fn on_scoped_event(
        &mut self,
        topic: crate::topics::ModEventTopic,
        payload: &str,
        session: Option<&str>,
    ) -> Result<(), ModError> {
        let _ = session;
        self.on_event(topic, payload)
    }

    /// 收尾（release 资源）。
    fn shutdown(&mut self) -> Result<(), ModError> {
        Ok(())
    }

    /// **只读运行态快照**（Wave 2 新增，2026-09-14）。
    ///
    /// 用途：让 host 把 Mod 的**内部状态**暴露到可测面——`GET /api/v1/mods/{id}/state`
    /// 与前端消费（本轮使用者：`wallpaper` 的当前决策、`pet-desktop` 的口型/窗口态）。
    /// 在它之前，Mod 的状态只能靠日志观察，前端拿不到，于是出现「Mod 里跑了一套
    /// 状态机、界面上什么也看不见」。
    ///
    /// # 契约
    ///
    /// - 返回 `Some(json)` = 该 Mod 愿意公开的状态（**必须脱敏**：不得含 token /
    ///   密钥明文——返回体会被前端渲染、被日志记录）；
    /// - 返回 `None` = 本 Mod 没有可公开状态（缺省实现）；
    /// - **只读语义**：host 只把它当「取一次快照」。允许为计算快照而推进内部游标
    ///   （例如 `wallpaper` 用本次调用时间推进策略时钟），但**不得**在里头发起
    ///   网络 / 写盘 / 阻塞等待——它在 web_api 线程上被调用，卡住就是卡住 HTTP；
    /// - 取 `&mut self` 的代价：host 必须拿到 runtime 锁才能调用，因此
    ///   **Mod worker 正在处理事件时本调用会失败（host 侧回 503）**——
    ///   这是刻意的：宁可一次读不到，也不要让 HTTP 线程等 Mod worker。
    fn state_json(&mut self) -> Option<serde_json::Value> {
        None
    }

    /// **一次性命令**（产品级加强波次新增）。
    ///
    /// 与 `Self::state_json` 的分工：
    /// - `state_json` 是**只读快照**（面板每次展开都取）；
    /// - `command` 是**有副作用的动作**（清空记忆库、导出、自检……），
    ///   由 host 在用户按下按钮时调用一次。
    ///
    /// # 契约
    ///
    /// - 入参 `command` 是稳定字符串（`clear` / `export` 等），`args` 是
    ///   JSON 对象（可空）；**命令词汇由各 Mod 自定**，host 只转达；
    /// - 返回 `Ok(json)` = 已执行，`json` 是给前端的**脱敏**结果；
    /// - 返回 `Err(ModError::UnsupportedCommand { .. })` = 不认识这条命令
    ///   （host 回 409 `unsupported_command`，前端据此说「这个 Mod 没有这个动作」）；
    /// - 返回其它 `Err` = 执行失败（host 回 409 `command_failed`）；
    /// - 与 `state_json` 同样是 `&mut self`、同样会被 worker 锁挡住：
    ///   Mod worker 正忙时 host 回 503，**调用方必须能重试**；
    /// - **不得**在里头发起网络请求或无限期阻塞（它在 web_api 线程上跑）；
    ///   本地文件 IO 是允许的（这正是命令存在的意义）。
    fn command(
        &mut self,
        command: &str,
        _args: &serde_json::Value,
    ) -> Result<serde_json::Value, ModError> {
        Err(ModError::UnsupportedCommand {
            command: command.to_string(),
        })
    }
}
