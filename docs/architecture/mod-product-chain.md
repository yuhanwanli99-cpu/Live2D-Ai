# Mod 产品链路（rc.4 起）

> **状态**：2026-09-13 rc.4 起草，rc.4 唯一 Mod 契约。与旧「插件 SDK」文
> （[`plugin-sdk.md`](plugin-sdk.md)，Python 时代、含动态加载与动作通道）**冲突时以本文为准**。
> 真源计划：[`../plans/PLAN-rc4-mod-product-chain-2026-09-13.md`](../plans/PLAN-rc4-mod-product-chain-2026-09-13.md)。

## 1. 定位

Mod 是**编译进当前二进制的静态模块**（Rust workspace crate）。enable /
disable / config 是**运行时开关**，不是加载器：

- 没有动态 `.so`、没有热插拔下载市场、没有运行期安装新 crate；
- 新增 Mod = 改仓库、过门禁、重新构建——见 §3 勾选表；
- 主链只做「LLM → TTS → 口型 → Live2D + UI」，增强能力一律走 Mod 边界
  （[`core-chain-baseline.md`](core-chain-baseline.md)）。

## 2. 契约面（`live2d-ai-mod-system`）

| 项 | 说明 |
| --- | --- |
| `ModFactory` / `ModRuntime` | 静态注册的工厂 + 实例；`start` 注册能力，`shutdown` 收尾 |
| `ModRegistrar` | `register_settings(spec)` / `subscribe(topic)` / `unsubscribe(id)`；**不暴露**宽泛 reload / register_action_source |
| `ModSettingsSpec` | 纯数据 schema（Bool/String/Number/Select），前端统一渲染；**禁止** Mod 注入 HTML/JS |
| `ModServices.say_tx` | 外部文本 → `supervisor.say`（主链路） |
| `ModServices.apply_settings` | **一等**配置写回（rc.4）：namespaced JSON patch → 写盘 + `supervisor.reload()` |
| `ModServices.logger` | 带 `mod_id` 的 tracing 日志 |
| `ModServices.action_tx` | **休眠**（rc.2 起）：host 注入固定 sender，请求只留一行 debug 并返回 `false`；**禁止**私自唤醒 |
| `MOD_API_VERSION` | 当前 `1`；Mod 的 `descriptor.api_version` 不对齐 → 启动即 `Failed`，主链不崩 |

## 3. 加一个新 Mod：勾选表

按顺序做完每一行，缺一行就是「半接线」：

- [ ] `crates/live2d-ai-mod-<id>/` 新建 crate（可复制 `crates/live2d-ai-mod-template`）
- [ ] 根 `Cargo.toml` 的 `members` 加入该 crate
- [ ] `crates/live2d-ai-desktop/Cargo.toml` 加依赖
- [ ] `crates/live2d-ai-desktop/src/main.rs` 的 `AVAILABLE_MOD_FACTORIES` 注册 `&<crate>::FACTORY`
- [ ] 同步 `main.rs` 里的 Mod 数量 / id 断言（`mod_count_is_*` / `mod_factory_ids_match_expected`）
- [ ] `descriptor.api_version = MOD_API_VERSION`；settings spec 字段 key 唯一
- [ ] crate 内至少一条单测（`ModFactory` 可构造 + `start` 成功）
- [ ] 若改默认启用：同步 `cli_entry::default_mods_manifest` 与本文/AGENTS 一行
- [ ] 文档：本文件 §5 表加一行；`AGENTS.md` 的 Mod 段同步

## 4. 配置与持久化（`mods.json`）

- 路径：与 `live2d-ai.toml` 同目录（web 模式 cwd 优先，否则 `~/.config/live2d-ai/`）。
- 格式：`{"mods":{"<id>":{"enabled":bool,"config":{...}}}}`。
- `mods.json` 缺失 → 用内建缺省（`cli_entry::default_mods_manifest`，缺省只 enable `local-llm`）。
- **写回（rc.4 M1）**：`enable` / `disable` / `config` 都**原子写**该文件（tmp + rename），
  重启状态不丢；磁盘不可写时保留内存状态并记 `error` 日志。
- `config` 只属于 **Mod 自己**（namespaced），**不回流**主 `live2d-ai.toml` 的 `[persona]` 等段。

## 5. 现行 Mod

| id | 缺省 | 语言 | 说明 |
| --- | --- | --- | --- |
| `local-llm` | **on**（`externally_managed=true`） | Rust | 本地推理进程探活 + 就绪后经 `apply_settings` 写回 `llm.base_url` |
| `external-input` | off | Rust | 外部文本接入（`say`） |
| `pet-desktop` | off | Rust | 桌宠骨架 |
| `persona` | off | Rust | 酒馆角色卡（rc.4 M5）：导入 V2 JSON/PNG → 合成 `system_prompt` |

`live2d-ai-mod-template` 是**模板 crate**，不注册进 `AVAILABLE_MOD_FACTORIES`。

## 6. 正式版规则（Rust/C 为主）

- 正式 / 稳定发布的 Mod **必须以 Rust（或 C）为实现主体**；非 Rust/C（Python/Dart/JS…）
  只能标**实验**，**不得**出现在正式发布说明的 Mod 清单里。
- 判定口径与 `cargo run -p xtask -- rust-ratio` 同源：Mod crate 计入 workspace 分子/分母；
  新增非 Rust/C 实现不得进 workspace（否则拉低全仓占比门禁，属应当被拦下的改动）。
- 新增 Mod 的 PR 必须自检：`flutter analyze` 不涉及；Rust 侧跑全套门禁（见 `AGENTS.md` 一张表）。

## 7. 失败隔离

- `factory.create` / `runtime.start` 返回 `Err` → 该 Mod `Failed` 并 disable，**主链继续**。
- host → Mod 事件经**有界 channel + 独立 worker**：慢 Mod 不卡 supervisor，队列满则丢弃并计数。
- `on_event` 返回 `Err` → 清空该 Mod runtime 槽位，待 host restart 恢复。
- 任何 Mod **不得**绕过 core 仲裁、不得直接持有密钥（密钥只经 `live2d-ai-runtime::secrets`）。

## 8. 非目标（rc.4 明文）

- 动态 `.so` / 热插拔下载市场；
- 恢复 Action / director / 编舞 / LLM tools（归档在分支 `archive/action-layer-p6`）；
- 会话记忆后端、双 LLM「是否朗读」分轨；
- 为主链增加「应用内预设提示词」——DeepSeek **API** 无此能力，RP 质量由 Mod 自己拼
  system / 首轮文本负责。
