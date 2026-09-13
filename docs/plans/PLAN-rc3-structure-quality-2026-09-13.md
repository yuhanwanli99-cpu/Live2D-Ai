# Live2D-Ai — rc.3「结构质量」改良长计划

> **状态**：2026-09-13 起草（WSL2 工作区），供新对话 worker 移交。
> **上游输入**：
> ① 已发布 [`v0.1.0-rc.2`](../releases/v0.1.0-rc.2.md) + [`HANDOFF-2026-09-13-rc2-second-baseline.md`](HANDOFF-2026-09-13-rc2-second-baseline.md)；
> ② rc.2 只读审查裁决（核心链「基本够硬」；**勿正式开步骤 2**；建议做瘦 rc.3 再谈 0.1.0）；
> ③ [`PLAN-rc2-second-baseline-2026-09-12.md`](PLAN-rc2-second-baseline-2026-09-12.md) 中明确推到 rc.3 的条目。
> **本文件是 rc.3 / 步骤 1 收尾的唯一真源**；与 rc.2 计划或旧节点 plan 冲突时，以**本文件 + 现行 `main`（含 tag `v0.1.0-rc.2`）**为准。
> **口径**：个人项目；可读性与工程优雅优先；弱化安全；**宁删勿加**；**不做步骤 2（Mod 功能堆砌）**。
> **工作区纪律**：核心开发**一律在 WSL2** `/home/skystar/Live2D-Ai`；Windows **只开浏览器**验 `/app/`；服务进程只在 WSL 起（`./scripts/ignite.sh`）。

---

## 0. 目标与版本路线

**一句话**：在 rc.2 已打通的主链上，把**结构债与同拍脆点**收干净，让「可读、可改、CI≈本地」站得住，再标正式 `0.1.0`。

| 版本 | 定位 | 内容 |
|---|---|---|
| **0.1.0-rc.2**（已完成） | 第二基线 | 动作删到底 + 模型闭环 + `.env` + 点火纪律 + 旧 JS 隔离 |
| **0.1.0-rc.3**（本计划） | **结构质量** | 正文兜底论证/实现 + God Object/大文件止血 + 双壳裁决 + 半接线清理 +（可选）背景图修复 + **M4 门禁对齐** |
| **0.1.0**（首个正式） | 稳定 | rc.3 DoD 全绿 + 发布说明；**仍不做步骤 2** |

> **rc.3 范围冻结（瘦版）**：只做下面加粗里程碑。不为拆而拆 crate；不恢复动作/director；不开空 `mainline/2-*`。

**rc.3 DoD（可验收）**：

1. TTS 故障 / 未成句时，用户仍能看到**已生成正文**（并标明未收尾）——同拍契约有**书面裁决 + 实现 + 测试**；
2. `shell/flutter/lib/main.dart` **≤700 行**（组合根只装配）；生产大文件无「无豁免且 >1000」；测试套无「无说明且 >800」或豁免台账只减不增；
3. egui / `--chat` / `--pet-mode`：**feature-gate 或文档钉死非主线 + 休眠台账**（二选一写进 AGENTS）；
4. 半接线清干净：`chat_panel`「不落盘」注释、`forcedByLaunchFlag` 接线或删除；capabilities/文案无新假广告；
5. **本地门禁 = CI 必跑表**对齐（见 §7）；`rust-ratio` ≥ 95%；
6. `docs/releases/v0.1.0-rc.3.md` 写清删了/拆了什么；版本三处同步为 `0.1.0-rc.3`。

---

## 1. 非目标

- 新 Mod、完善 director / pet-desktop / external-input **产品**能力（步骤 2）
- 恢复 `RootEvent::Action` / director / 渲染面编舞 / LLM tools
- 大范围「优雅架构」重写、crate 爆炸
- 安全 hardening、企业级观测、Android / 原生 Windows 主线
- 会话记忆后端、思考落盘、复杂 settings schema 扩张
- 新视觉 / 新动画 / 动作调试器

> 若任务属于上列：**停**，记到 §10 backlog，不要混进本计划 PR。

---

## 2. 事实基线（rc.2 交付后，2026-09-13）

| 事实 | 数值 / 位置 |
|---|---|
| 远端基线 | `origin/main` = tag `v0.1.0-rc.2` = **`6ab4a074`** |
| 版本 | `Cargo.toml` / capabilities / `pubspec.yaml` = `0.1.0-rc.2` |
| 门禁（handoff） | cargo test **795**；clippy 0；rust-ratio **96.9754%**；flutter test **813**；`verify_core_chain --timeout 300` 18 跳 |
| `main.dart` | **~1288** 行（God Object；rc.2 只接线未拆） |
| `surface.rs` | **~1167–1169** 行（编舞已删，仍 >1000；wasm 门控、原生测零覆盖） |
| `ws.rs` | **1073** 行 |
| `tests_models_routes.rs` | **~1249** 行（超测试 800） |
| 双壳 | egui / `--chat` / `--pet-mode` **仍默认编译**，未 gate |
| 同拍脆点 | 上屏闸门 = `SentenceVoiced`；TTS/句界失败 → **整轮无字**（HANDOFF §5/§7.4） |
| 半接线 | `chat_panel.dart` 仍写「历史不落盘」；`forcedByLaunchFlag: false` 写死 |
| CI | `pr-checks.yml` / `nightly.yml` 仍只 `cargo test --workspace` |
| 已知小债 | **Windows：背景图无法应用**（appearance → `stage-bg` → sync；见 §6） |
| 动作 / capabilities | 已删假广告；director 不在 members；core `action/` 明文休眠——**禁止本轮唤醒** |

**核心链判定（审查）**：**基本够硬**（能聊/出声/口型/换皮/密钥），但同拍副作用与结构债挡住「正式 0.1.0 + 上层」。

---

## 3. N0 — 正文兜底（同拍契约让位）【rc.3 核心产品】

### 3.1 问题

`EngineEvent::TextDelta` 不上屏；UI 文字等 `SentenceVoiced`。TTS 故障、半句切不出句、超时 → 用户以为「模型没回」。rc.2 已修截断主因（思考挤占 + 4096）；**兜底仍缺**。

### 3.2 决策（开工前先写进 PR / docs，再改代码）

**冻结口径（建议，可在 PR 首段微调但必须书面）**：

1. **默认仍同拍**：健康路径不变——有语音的句子仍以 `SentenceVoiced` 上屏。
2. **异常路径兜底**：当 turn 进入 `Failed` / TTS 不可用 / 句组装超时且已有未上屏正文缓冲区时：
   - 经 WS 推送一种**可区分**的帧（新建如 `text_fallback` / 或扩展现有 `error`+payload——**宁少字段**；禁止同时发明 `turn_id`）；
   - UI 立刻展示已缓冲正文，并标明「未收尾 / 无语音」；
   - **不得**让 TextDelta 流式抢跑健康路径（避免文字再次跑到声音前面）。
3. 验收必须含：**人为掐 TTS** 时仍能看见字。

### 3.3 工作项

1. 读 `live2d-ai-runtime` 对话引擎 + `supervisor/handlers` + Flutter `chat_controller` / WS 帧解析；画「健康 vs 失败」时序图进 `docs/architecture/` 短文或 PR。
2. 实现兜底 + 单测（runtime/desktop）+ Dart `ws_frame` 回归（按**真实抓包**钉形状，见 HANDOFF §7.6）。
3. 更新 `core-chain-baseline.md`：写清同拍默认 + 异常让位，删除任何与实现矛盾的句子。
4. `verify_core_chain.py`：能加「TTS 挂掉仍有字」探针则加；否则手工步骤写入 release note。

### 3.4 验收

- [ ] 健康路径：文字仍不领先声音（人工 + 既有测试）
- [ ] TTS 不可用 / 合成失败：气泡有正文 + 未收尾提示
- [ ] `cargo test` / `flutter test` 绿；相关帧有回归测试

---

## 4. N1 — 结构性减法（God Object + 大文件）【rc.3 核心工程】

### 4.1 原则

- **先删死肢 / 迁注释，再拆文件**；按行为边界拆，禁止为过门禁平均切文件。
- 生产 ≤500（豁免 ≤1000 且**数量不增**）；测试 ≤800；`rust-ratio` ≥95%（大砍测试前先量）。
- wasm 改动 = **重建 + 肉眼**（口型、待机呼吸/眨眼）；**勿删 `IdleState`**。

### 4.2 工作项

1. **`main.dart` → ≤700**  
   外移：`admin`（Mod/诊断）、`settings` wiring、`模型库` 接线、偏好/背景图回调等；组合根只构造与 dispose。  
   模型库 UI 从 `dev_tools_section.dart` **名实分离**（主路径分区文件），但**不要**借机做新功能。
2. **`surface.rs` 拆「渲染 / 输入 / 待机」**（单独立项+肉眼）；保持 Idle 与口型。
3. **`ws.rs`**：音频帧 vs 连接生命周期推进已有 `ws/` 子模，父文件变薄。
4. **`tests_models_routes.rs`**：按场景切开或删冗余；合并前跑 `rust-ratio`。
5. supervisor / `cli_entry` / `mod_registry`：优先删史诗注释与死分支，再抽象。

### 4.3 验收

- [ ] `main.dart` ≤700；`flutter analyze` + `flutter test` 绿
- [ ] 无「无豁免且生产 >1000」；测试超线有登记或切开
- [ ] 豁免头注数量 ≤ 执行前；`rust-ratio` ≥95%
- [ ] wasm 重建后 Win 浏览器：皮套、口型、待机仍正常

---

## 5. N2 — 双壳裁决（egui / `--chat` / `--pet-mode`）

### 5.1 决策（二选一，PR 里写死）

| 选项 | 做法 | 何时选 |
|---|---|---|
| **A（推荐）** | Cargo feature（如 `native-shell`）默认 **off**；`--web` 主路径零 egui | 编译/体积收益明显或测试矩阵可控 |
| **B** | 暂不 gate；AGENTS + README + `core-chain-baseline` **钉死「非主线 / 实验」+ 休眠台账**（谁休眠、为何、谁唤醒） | feature 矩阵会炸 `cli/tests` 且短期无人力 |

**禁止**：继续往 egui 设置面板加与 Flutter 重复的产品字段。

### 5.2 验收

- [ ] 默认文档入口只推销 `--web` / `ignite.sh`
- [ ] 若 A：默认 build 不含 egui 主依赖（或等价可验证断言）
- [ ] 若 B：休眠台账进 AGENTS，且无第三种含糊表述

---

## 6. N3 — 半接线清理 + 已知小债

### 6.1 必做

1. 修 `chat_panel.dart`「历史不落盘」过时注释；UI 明示 **会话记录 ≠ 模型记忆**（`POST /chat` 仍只 `{text}`）。
2. `forcedByLaunchFlag`：**接线读 status** 或 **删除分支文案**——不许写死 `false` 装能关。
3. 扫一遍设置/诊断：无已删能力字样；`rg` 双向验证。

### 6.2 可选（用户 Win 实测，token 紧时可单独短 PR）

**现象**：Windows 测试「无法应用背景图」。

粗定位：

- `settings/sections/appearance_section.dart`
- `main.dart` `_pickStageImage` / `_updatePrefs` / `_clearStageImage`
- `display_prefs.dart` `kStageImageMaxChars`
- bridge `stage-bg` → wasm CSS `background-image`

方向：localStorage 配额 / dataURL 超限、iframe 未就绪丢 sync、Win 选图差异。  
**修好则写入 rc.3 release note**；修不好则记 HANDOFF「已知债」，**不挡 DoD #1–#5**。

### 6.3 验收

- [ ] `rg -n "不落盘" shell/flutter/lib` 无过时义
- [ ] `forcedByLaunchFlag` 不再半接线
- [ ] （可选）Win：选背景图 → 舞台可见变化

---

## 7. N4 — 门禁对齐（CI = 本地一套真相）

对照 rc.2 计划 M4；**本轮纳入 rc.3 DoD**（审查：无此则不宜标 0.1.0）。

1. `.github/workflows/pr-checks.yml`：补 `--all-targets`、`cargo test --doc --workspace`、`xtask rust-ratio`；Flutter job（`paths: shell/flutter/**`）跑 `analyze` + `test`。
2. `nightly.yml`：加上 `python3 scripts/verify_core_chain.py`（可 `--timeout 300`）；PR 保持快速集。
3. AGENTS / CONTRIBUTING：**一张表**「本地必跑 vs CI 必跑」。
4. 删除空指针分支 `origin/mainline/1-core-baseline`（若仍 == `main`）。
5. （可选）故意破坏一条核心链断言，确认 nightly/CI 会红——记进 PR。

### 7.1 验收

- [ ] 文档命令与 workflow **逐条对得上**
- [ ] 本地手跑与 CI 清单无「两套真相」

---

## 8. N5 — 可读性收尾（可与 N4 并行，不挡核心）

1. 根目录 Python 时代作废文档 → `docs/legacy/` 指针。
2. `docs/design/` 收束为**一份现行规格 + 历史存档**（legacy 旧 JS 已在 rc.2，勿再当现网）。
3. 史诗节点注释迁 `docs/plans/`；源码头注只留不变量三行 + 链接。
4. 日志口径沿用，**不引入** `turn_id` 等新跨层字段。

---

## 9. 建议执行顺序（worker）

```text
N0 正文兜底（先论证再改） 
 → N1 main.dart 拆分（收益最大）
 → N1 surface/ws/tests 止血（surface 单独 PR + 肉眼）
 → N2 双壳裁决
 → N3 半接线（+ 可选背景图）
 → N4 CI 对齐
 → N5 可读性收尾
 → 版本号 0.1.0-rc.3 + docs/releases + tag（先本地，Win 点火后再 push）
```

每个里程碑**一个（或少数）提交**；删除/大拆前有归档点或可回退提交边界（学 rc.2：M1 与 M1.5 分开）。

---

## 10. 步骤 2 占位（仅记录，不执行）

- 启动条件：**本 rc.3 DoD 完成**后，且 capabilities/文档/主路径无半拆除矛盾。
- 形式：短命 `mod/<name>`；**不要**现在建 `mainline/2-*`。
- 任何 Mod **不得**恢复 Action 注入，除非另开计划并改 capabilities 为实话。

---

## 11. 移交检查清单（新 worker 启动时）

```text
[ ] 读完本文件 §0–§3 + HANDOFF-2026-09-13-rc2-second-baseline.md §5–§7
[ ] 工作区 = WSL2 /home/skystar/Live2D-Ai（不是 win 侧 Copilot 镜）
[ ] git fetch && git checkout main && git pull --ff-only
[ ] 确认基线 = v0.1.0-rc.2（6ab4a074 或其快进后代）
[ ] 跑一遍 handoff §2 门禁，确认「出发时是绿的」
[ ] ./scripts/ignite.sh --check；Windows 打开 /app/ 确认现状
[ ] 确认当前里程碑（建议从 N0 或 N1 起，与用户对齐一项）
[ ] 列出本 PR 要删/要拆的路径（先清单后改）
[ ] 大砍测试前先 rust-ratio
[ ] wasm/UI 改动：flutter build web + 肉眼；前端改完必须重建（HANDOFF §7.7）
[ ] PR 写清「删除/拆了什么」「主路径如何更短」
[ ] §1 非目标禁止顺手做
[ ] 发布：本地提交+tag → Win 点火 → 再 push（与 rc.2 相同纪律）
```

---

## 12. 参考路径速查

- 计划真源（本文件）：`docs/plans/PLAN-rc3-structure-quality-2026-09-13.md`
- rc.2 交接：`docs/plans/HANDOFF-2026-09-13-rc2-second-baseline.md`
- 核心链文档：`docs/architecture/core-chain-baseline.md`
- 同拍 / 引擎：`crates/live2d-ai-runtime/`、`crates/live2d-ai-desktop/src/supervisor/`
- 前端组合根：`shell/flutter/lib/main.dart`
- 渲染面：`crates/l2d-wasm-demo/src/web/surface.rs`
- 点火：`scripts/ignite.sh`；门禁：`scripts/verify_core_chain.py`
- CI：`.github/workflows/`

---

## 13. 给新对话 worker 的启动口令（可原样粘贴）

```text
工作区：/home/skystar/Live2D-Ai（WSL2）
真源：docs/plans/PLAN-rc3-structure-quality-2026-09-13.md
上游基线：tag v0.1.0-rc.2 / origin/main
口径：个人项目，可读性优先，宁删勿加；不做步骤 2；不恢复动作层
顺序：N0 → N1 → N2 → N3 → N4 → N5 → 打 rc.3 发布说明与版本号
每步：先列删除/拆分清单再改；门禁绿；Win 浏览器肉眼（涉及 UI/wasm 时）
禁止：空 mainline 分支、新 Mod 产品能力、先 push 后点火
```

---

*本计划是 rc.3 的唯一移交真源。rc.2 计划保留为历史；新工作以本文件为准。*
