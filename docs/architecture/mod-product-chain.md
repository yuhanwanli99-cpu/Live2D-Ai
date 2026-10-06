# Mod 产品链路（rc.4 起）

> **状态**：2026-09-13 rc.4 起草，rc.4 唯一 Mod 契约。与旧「插件 SDK」文
> （[`plugin-sdk.md`](plugin-sdk.md)，Python 时代、含动态加载与动作通道）**冲突时以本文为准**。
> 真源计划：`../legacy/plans/PLAN-rc4-mod-product-chain-2026-09-13.md`（**已移出工作树**，取回见 [文档索引](../REMOVED-docs-index-2026-10-06.md)）。

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
| `ModFactory::settings_spec()` | **静态** schema（rc.4 M2）：未启用也能拿到，前端可「先填配置、再启用」；缺省 `None` = 沿用运行时 `register_settings` |
| `ModSettingsSpec` | 纯数据 schema（Bool/String/Number/Select），前端统一渲染；**禁止** Mod 注入 HTML/JS |
| `ModServices.say_tx` | 外部文本 → `supervisor.say`（主链路） |
| `ModServices.apply_settings` | **一等**配置写回（rc.4 M4）：namespaced JSON patch → 写盘 + `supervisor.reload()`；未注入 host 时**默认拒绝** |
| `ModServices.settings` | **脱敏设置读取**（rc.4 M5）：`read()` 返回当前设置快照（无密钥、无 `api_key_env` 变量名），供 Mod 记忆基线并还原 |
| `ModServices.config_path` | 真实 `live2d-ai.toml` 路径（host 注入，Mod 不自行探测） |
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
- `mods.json` 缺失 → 用内建缺省（`cli_entry::default_mods_manifest`，缺省只 enable `external-input`）。
- **写回（rc.4 M1）**：`enable` / `disable` / `config` 都**原子写**该文件（tmp + rename），
  重启状态不丢；磁盘不可写时保留内存状态并记 `error` 日志。
- `config` 只属于 **Mod 自己**（namespaced），**不回流**主 `live2d-ai.toml` 的 `[persona]` 等段。
- **对外读法（rc.4 M2）**：`GET /api/v1/mods` 每个条目带 `config` 与 `settings_spec`
  （schema，无则为 `null`）；`settings_spec.fields[].kind` ∈ `bool/string/number/select`。
  `secret=true` 的字段值**永不出现在 GET**；另有 `GET /api/v1/mods/{id}/config` 单读。

## 5. 现行 Mod（产品级加强波次起：5 个注册 + 2 个封存 + 1 个废除）

| id | 缺省 | 语言 | 说明 |
| --- | --- | --- | --- |
| `external-input` | **on** | Rust | 外部事件接入：`POST /api/v1/external/chat` → `say`；静态 `settings_spec`（`token` / `text_template` / `prefix`），契约见 [external-input.md](../external-input.md)，B 站弹幕示例见 [docs/examples/bilibili-sidecar](../examples/bilibili-sidecar/README.md) |
| `persona` | off | Rust | 酒馆角色卡（rc.4 M5）：导入 V2 JSON/PNG → 合成 `system_prompt`；`0.2.0-rc.2` 起坏配置**显式 `Failed`**（不再假报「运行中」） |
| `voice-input` | off | Rust | 语音输入（Wave 2 A 轨 / 0.2.0-rc.3）：`POST /api/v1/voice/transcript` → `clean_transcript` → `say` **已接线**；ASR 本体在 sidecar（Rust 侧不开 socket、不做 IPC），契约见 [voice-input.md](../voice-input.md)，示例见 [docs/examples/voice-sidecar](../examples/voice-sidecar/README.md)，接线清单见 [REGISTER-voice-sidecar-v1](../plans/parallel-mods/REGISTER-voice-sidecar-v1.md) |
| `memory` | off | Rust | 会话记忆 v0（Wave 2 C 轨 / 0.2.0-rc.3）：本地 JSONL + 词元重叠检索 top-k → 写会话注入槽（有会话）或 `apply_settings` 写全局 `persona.system_prompt`（无会话降级）；**同轮生效**（2026-09-15 起，见 memory-mod-v0.md §2）。全局路径与 persona 仍是 last-writer-wins，见 [memory-mod-v0.md](memory-mod-v0.md) |
| `director` | off | Rust | 导演（Wave 3 G 轨 2026-09-14 建**最小骨架**，2026-09-21 已接线）：订阅 `TurnPrompt` + `TurnEnded` → 确定性 `{emotion,intent,suggested_tts}` 决策（纯函数、词表驱动），只写日志与 `state_json`；**`action_tx` 休眠、不写 `live2d-ai.toml` 仍成立**；**现状**另产 `latest.preset_id`（只读状态面，供面板展示）+ 按句 `action_cue`（`ModServices.cues` → WS `action_cue`，驱动舞台）。口径：**导演是一个 AI、属产品本体、不做架构搬迁**，输入 = **用户输入**（R1），谁的 `speak` 能力保留**未定、不裁决**；见 [director-mod-v0.md](director-mod-v0.md) |
| ~~`wallpaper`~~ | — | Rust | **已删除（2026-10-01 W2-A/D1）**：移出 `AVAILABLE_MOD_FACTORIES`（2026-09-14 封存）后，crate 已于 2026-10-01 **物理删除**（tag `checkpoint/pre-d1-dormant`），**禁止挂回**。用户手动的舞台/壳背景能力（`DisplayPrefs`）**不受影响**。见 [ARCHIVED-mods.md](ARCHIVED-mods.md) |
| ~~`pet-desktop`~~ | — | Rust | **已删除（2026-10-01 W2-A/D1）**：移出 `AVAILABLE_MOD_FACTORIES`（2026-09-14 封存）；本波**不做真窗/应用级桌宠**，crate 已于 2026-10-01 物理删除，**禁止挂回**。见 [ARCHIVED-mods.md](ARCHIVED-mods.md) |
| ~~`local-llm`~~ | — | Rust | **已删除**（0.2.0-rc.1 废除启动 → 2026-10-01 W2-A/D1 物理删除）：移出 `AVAILABLE_MOD_FACTORIES`，**禁止挂回** |

数量由 `main.rs::mod_count_is_five` 守住（Wave 3 的 7 → 产品级加强波次的 **5**）。
`live2d-ai-mod-template` 是**模板 crate**，不注册进 `AVAILABLE_MOD_FACTORIES`。
封存口径（原因 / 没有连坐删掉什么 / 恢复条件）见 [ARCHIVED-mods.md](ARCHIVED-mods.md)。

许可与分发门槛（社区 Mod / 闭源 / 商业许可）见
[Mod 社区许可与注册边界](mod-community-license.md)。

### 5.1 从 rc.3 升级：`[persona]` 迁移（**必须手改一次**）

`PersonaSettings` 带 `deny_unknown_fields`。rc.4 删掉卡字段后，旧的
`live2d-ai.toml` 里若还有 `name` / `description` / `personality` /
`scenario` / `first`，启动会**解析失败**（错误会指出具体键名）。处理：
删掉这几行（人设改由 `persona` Mod 的 `card_path` / `card_json` 提供），
或直接用新的 `live2d-ai.toml.example`。`[persona]` 只保留
`system_prompt` 与 `max_history_pairs`。

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
- 恢复 Action / director / 编舞 / LLM tools（旧实现在 `0.1.0-rc.2` / M1.5 已删；归档分支 `archive/action-layer-p6` **已不存在**，2026-10-06 复核。取回：director `git show 98469df^:crates/live2d-ai-mod-director/src/lib.rs`、渲染面编舞 `git show ef9f428^:crates/l2d-wasm-demo/src/web/surface.rs`）；
- 会话记忆后端、双 LLM「是否朗读」分轨；
- 为主链增加「应用内预设提示词」——DeepSeek **API** 无此能力，RP 质量由 Mod 自己拼
  system / 首轮文本负责。
