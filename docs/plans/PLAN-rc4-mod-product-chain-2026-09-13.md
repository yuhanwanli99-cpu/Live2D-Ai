# Live2D-Ai — rc.4「Mod 产品链路 + 主链人设收敛」长计划

> **状态**：2026-09-13 起草（WSL2），供新对话 worker 移交。改动面允许较大（rc.3 结构债基本收完）。
> **上游**：`v0.1.0-rc.3` @ `0220fd03`；[`PLAN-rc3-…`](PLAN-rc3-structure-quality-2026-09-13.md)；rc.3 发布说明 §9 挂账；用户裁决（上层走 Mod、主链慎加、角色卡出主链、只留 `system_prompt`）。
> **本文件是 rc.4 唯一真源**；与旧 node-e / plugin 文档冲突时，以本文件 + 现行 `main` 为准。
> **口径**：个人项目；可读性优先；宁删勿加（**主链**）；**Mod 可加**；正式/稳定发布的 Mod **Rust/C 为主占比**，非 Rust/C **不得**标正式版。
> **工作区**：一律 WSL2 `/home/skystar/Live2D-Ai`；Win 只开浏览器验 `/app/`。

---

## 0. 目标与版本

**一句话**：把 Mod 从「能编进 binary 的骨架」做成**可持久化、可配置、可照清单加新仓**的产品链路；把酒馆角色卡从主链内嵌抽成**第一条标准 Mod**；顺手收口 rc.3 挂账（舞台背景图）。

| 版本 | 定位 |
|---|---|
| rc.3（已完成） | 结构质量：正文兜底、拆大文件、双壳休眠、半接线、CI≈本地 |
| **rc.4（本计划）** | Mod 产品链路 + 角色卡标准 Mod + 主链人设收敛 + stage-bg |
| 之后 | 导演/半身等**短命 `mod/<name>`**；双 LLM 朗读等长期项（见 §10） |

**rc.4 DoD（可验收）**

1. enable/disable/config **写回** `mods.json`，重启状态不丢；
2. 新 Mod 可按 **注册清单 + 模板 crate** 编入（members / FACTORIES / 测试断言有文档勾选）；
3. `GET /api/v1/mods`（或子路由）带出 `settings_spec`，Flutter 能渲基础表单并 `POST …/config`；
4. 主链 `[persona]` **只剩** `system_prompt`（+ `max_history_pairs` 作会话基建）；酒馆卡字段与导入 UI **不在**主设置分区；
5. 标准 Mod（角色卡）能导入 V2 JSON/PNG，并把合成人设经 **一等** `apply_settings`（或专用窄通道）写入 `system_prompt`；
6. Win：**舞台背景图**可选图后舞台可见（rc.3 §9.1）；
7. 文档：`mod-product-chain` + Mod **Rust/C 正式版规则**；版本号 `0.1.0-rc.4` + `docs/releases/v0.1.0-rc.4.md`；
8. 本地门禁绿（与 rc.3 同一套）；涉及 wasm/UI 时 Win 肉眼。

---

## 1. 非目标

- 动态 `.so` / 热插拔下载市场
- 恢复 Action / director / 编舞 / LLM tools（归档在 `archive/action-layer-p6`）
- 双 LLM「是否朗读」、Dialogue Brain 分轨（**长期**，§10）
- 会话记忆后端、pet-desktop/external-input **产品化堆砌**
- 把 DeepSeek **应用内** Expert 模式体验原样搬进主链（见 §2）
- 为大改而重写 Mod ABI（保持 `MOD_API_VERSION = 1`，不够用再显式 +1）

---

## 2. 调研笔记：DeepSeek API vs 应用（纠正）

| 通道 | 真相（对本项目） |
|---|---|
| **官方 App/Web** | 有会话态、模式（Expert/Quick）等**产品壳**；与 API 不是同一套体验 |
| **官方 API**（Live2D-Ai 使用） | **无**「应用内预设提示词」自动塞进请求；客户端发什么 `messages`（含可选 `system`）就是什么；无状态，多轮需自带历史 |
| **社区说法** | ST 预设、「服务端注入 RP」等**不可直接当成 API 契约**；部分「角色沉浸」标记是**显式**贴在首条 user 末尾的控制指令（社区文档），不是隐藏 system |
| **实现含义** | 角色扮演质量 = **模型选型 + 你们（Mod）自己拼的 system/首轮文本**；主链继续 OpenAI 兼容即可，不必等「官方 RP API」 |

---

## 3. 事实基线（rc.3 后）

| 项 | 现状 |
|---|---|
| FACTORIES | 3：`external-input` / `pet-desktop` / `local-llm`（`main.rs`）；缺省 manifest 只 enable `local-llm` |
| mods.json | 可读；**enable/disable 不落盘** → 重启丢开关 |
| settings_spec | 有登记；**无**产品 API/Flutter 表单 |
| apply_settings | 经 `event_tx` + `__apply_settings` JSON「走私」；可用、非一等 |
| action_tx | **休眠**；禁止本轮唤醒 |
| persona | Flutter 能进酒馆 V1/V2 PNG/JSON；runtime **只**用 name+description+system_prompt；personality/scenario/first **空转** |
| stage-bg | rc.3 **挂账未修**（appearance → shell_prefs → bridge → wasm CSS） |
| 模板 | 无现行 Rust Mod 模板（旧 plugin-template 是 Android 遗留） |

---

## 4. M0 — Mod 产品链路文档钉死【先做】

**产出**：`docs/architecture/mod-product-chain.md`（短）

必须写清：
- 静态编译模型（无动态 .so）；注册步骤勾选表；
- HostChannels：`say` / `apply_settings`（一等化后）/ `config_path`；**action 休眠**；
- manifest 格式与持久化；失败隔离；
- **正式版规则**：Rust/C 为主占比方可标正式/稳定；非 Rust/C → 实验标签 only；与 `xtask rust-ratio` / workspace 成员关系写死；
- 非目标列表（§1）。

**验收**：AGENTS / README 链到该文；与「默认停用」等过时文案一致（缺省 `local-llm=on` 写清楚）。

---

## 5. M1 — manifest 读写对称【P0】

| 工作 | 细节 |
|---|---|
| 持久化 | `enable` / `disable` / `reload_config`（及 config POST）→ **原子写** `mods.json` |
| 启动 | `from_disk` 与内存一致；UI 刷新后重启可复现 |
| 测试 | 启停 → 杀进程语义（或单测写临时 manifest）→ 再读 |

**验收**：改开关 → 重启 `--web` → 状态仍在。

---

## 6. M2 — schema 发现闭环【P1，可与角色卡并行设计】

1. `GET /api/v1/mods` 带 `settings_spec`（或 `GET /api/v1/mods/{id}/settings`）；
2. Flutter `ModsSection`：按 spec 渲 Bool/String/Number/Select；接 `POST …/config`；
3. 无 spec 的 Mod 仍可只显示开关（兼容旧三家）。

**验收**：改 local-llm 或角色卡 Mod 的一项配置 → 落盘 → 生效（或 restart 后生效，行为写进文档）。

---

## 7. M3 — 模板 crate + 注册清单【P0】

1. `crates/live2d-ai-mod-template`（或 `xtask mod-new`）：最小 `ModFactory`/`ModRuntime`、假 settings、单测；
2. CONTRIBUTING/AGENTS **勾选表**：根 `Cargo.toml` members → desktop dep → `AVAILABLE_MOD_FACTORIES` → `mod_count` 断言 → 文档一行；
3. （P1）启动时校验 `descriptor.api_version` vs `MOD_API_VERSION`，不兼容 → Failed，主链不崩。

**验收**：按清单加一个 hello Mod（可随后删或留作 template 示例）全绿。

---

## 8. M4 — `apply_settings` 一等化【P1，角色卡前置强烈建议】

- `ModServices` / `HostChannels` 增加显式 `apply_settings(patch: Value)`（或等价）；
- 去掉（或降级文档禁止）`__apply_settings` 事件走私作为唯一路径；
- （可选更窄）`set_persona_system_prompt(text)`——若担心 Mod 乱 PATCH 任意键；**默认先做通用 apply_settings**，窄通道按需。

**验收**：local-llm 与角色卡 Mod 都走一等 API；单测覆盖写盘+reload。

---

## 9. M5 — 角色卡标准 Mod【P0/P1 主交付】

### 9.1 主链收敛

**保留**：`persona.system_prompt`；`persona.max_history_pairs`（或改挂 `[conversation]`，本轮可不改名）。  
**删除/迁出**：`name` / `description` / `personality` / `scenario` / `first`；`build_effective_system_prompt` 的「拼名字简介」逻辑；Flutter `persona_card` / `persona_import` / 胖 `persona_section`；settings DTO/PATCH/View/测试中的卡字段；egui 休眠壳勿扩字段。

瘦 UI：主设置只留「系统提示词」（+ 历史轮数，可放开发者区）。

### 9.2 新 Mod：`live2d-ai-mod-persona`（名可改）

- **语言**：Rust（正式版候选）；
- 解析 SillyTavern V1/V2 JSON + PNG `chara` tEXt（逻辑从 Flutter 迁入或共享 crate）；
- 配置落在 **Mod config / mods.json**，不回流主 `[persona]` 卡字段；
- 合成：`纪律模板（可选，Mod 内）⊕ 卡字段` → `apply_settings` 写 `system_prompt`；
- 可选：启用时 `say(first_mes)`；
- `register_settings`：导入路径/开关/字段编辑（配合 M2）。

### 9.3 验收

- [ ] 主设置无酒馆卡分区；toml 无卡字段（或 migration 说明一次清掉）
- [ ] 导入一张 V2 PNG/JSON → 对话人设生效（system 可见于行为）
- [ ] 禁用 Mod → 回到仅主链 `system_prompt`
- [ ] rust-ratio / 门禁绿；Mod 计入正式版占比规则

---

## 10. M6 — Win 舞台背景图【P0 挂账】

**范围钉死**：修复 **Live2D 舞台 `stage-bg`**（appearance → `shell_prefs` → bridge → wasm），**不是**另做壳层/聊天区壁纸（若要壳背景 = 另项，勿混进本里程碑）。

**验收**：Win 浏览器选图 → 舞台 CSS/画面可见变化；超限/失败有老实文案；清图可用。

---

## 11. 建议还做的（P2，不挡 DoD 核心时可并行）

| 项 | 理由 |
|---|---|
| 拆 `live2d-ai-mod-local-llm`（~806 行） | rc.3 挂账；示范 Mod 内文件纪律 |
| wasm CI job | 背景/舞台改动回归 |
| Mod 计数/文档一致性门禁 | 防 FACTORIES vs 文档漂移 |
| N5 史诗注释迁出（registry/cli_entry） | 可读性 |

---

## 12. 明确不进 rc.4（长期 / 另计划）

| 项 | 归处 |
|---|---|
| 双 LLM 判是否朗读全文（可与「导演」合并喂 TTS） | 长期；且需主链 **TtsJob 前极窄钩子**（另开 RFC） |
| 导演 Mod / 半身联动（产品） | 步骤 2；**短命 `mod/<name>`**；若要动动作通道必须**单独裁决**，禁止私自唤醒 |
| 会话记忆后端 | future-roadmap |
| 壳层/聊天区背景（≠ stage-bg） | 另项短计划 |
| DeepSeek App Expert 体验复刻 | 不需要；API 无预设，Mod 自管提示词即可 |

---

## 13. 执行顺序（worker）

```text
M0 文档 + 正式版 Rust/C 规则
 → M1 mods.json 持久化
 → M3 模板 + 注册清单（+ api_version 门禁可同 PR）
 → M4 apply_settings 一等化
 → M5 角色卡迁出 + 标准 Mod（可与 M2 schema UI 咬合）
 → M2 schema UI（若 M5 需要表单则提前或并行）
 → M6 stage-bg
 → P2 挂账按余力
 → 版本 0.1.0-rc.4 + release note + tag（先本地，Win 点火后再 push）
```

---

## 14. 移交检查清单

```text
[ ] 读本文件 §0–§3、§9；rc.3 release §9.1
[ ] 工作区 WSL /home/skystar/Live2D-Ai；git pull；基线 v0.1.0-rc.3
[ ] 确认：主链只留 system_prompt；不恢复 Action
[ ] 确认：DeepSeek 走 API = 无应用内预设；提示词由客户端/Mod 负责
[ ] 当前里程碑从 M0/M1 起
[ ] 每 PR：删除/迁出清单；门禁；UI/wasm 要 Win 肉眼
[ ] 正式 Mod：Rust/C 占比规则自检
[ ] 先本地 tag，Win 点火通过再 push
```

---

## 15. 新 worker 启动口令

```text
工作区：/home/skystar/Live2D-Ai（WSL2）
真源：docs/plans/PLAN-rc4-mod-product-chain-2026-09-13.md
基线：tag v0.1.0-rc.3
必做：Mod 链路（文档/持久化/模板/schema）+ 角色卡标准 Mod + 主链只留 system_prompt + Win stage-bg
口径：主链慎加收敛；Mod 可加；正式 Mod=Rust/C 为主；非 Rust/C 不得正式版；禁止唤醒 Action
顺序：M0→M1→M3→M4→M5(+M2)→M6→发 rc.4
DeepSeek：只用 API 语义（无 App 预设）；RP 质量靠 Mod 拼 system/首轮，不靠官方隐藏预设
```

---

*本计划是 rc.4 移交真源。*
