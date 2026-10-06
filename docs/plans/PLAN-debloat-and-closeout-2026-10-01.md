# PLAN · 0.2.0 收口 + 去臃肿（2026-10-01）

> 立档 **2026-10-01** · 状态：**活** · 上一版计划：`docs/plans/PLAN-0.2.0-seal-and-cleanup-2026-09-27.md`（已于 2026-10-06 移出工作树，见 [`REMOVED-docs-index`](../REMOVED-docs-index-2026-10-06.md)）
> **口径（维护者 2026-10-01）**：项目太大太臃肿，**技术 / 工程债要收，代码精简与优化排上日程**。
> 本文承接 09-27 计划的收口部分，并**新增"去臃肿（Debloat）"专线**；冲突时本文为准。

## 0. 前提与纪律

- **唯一开发线**：`/home/skystar/Live2D-Ai-fe`（`feat/frontend-redesign` @ `dde6b855`）。
  `/home/skystar/Live2D-Ai`（`-Ai`）是死树，**不在其上开发**。
- **不推远端**；每阶段打本地 tag；任何破坏性 git 操作前先打 tag。
- **先修 bug，再删代码**：`rc.8-a` 必须排在任何"删/拆/重命名"之前。
- **重构与行为分开提交**（每个 commit 可归因）；阶段内可并发，但文件零重叠。

## 1. 现状快照（2026-10-01 09:17）

| 项 | 值 |
|---|---|
| 开发线 HEAD | `dde6b855` = `v0.2.0-rc.7`（2026-09-28） |
| `main`（本地） | `e4f139a8` = `v0.2.0-rc.4`；是 `-fe` 的祖先 ⇒ 末版可 `--ff-only` |
| tag | `v0.2.0-rc.1…rc.7` + `checkpoint/rc5-pre-rc6` |
| worktree / 分支 / stash | 3 / 12 / 0；磁盘 36G 可用 |
| 未提交（`-fe`） | `docs/README.md`、`AUDIT-REPO/`、2 份 09-28 文档 |
| 门禁基线（rc.7） | cargo 1457/0 · doc 3 · fmt clean · clippy 0 · rust-ratio 97.3263% · flutter analyze 0 · flutter test 1378 · gstatic 0/0 · ignite --check 四项 ok |

## 2. 体量实测基线（**冻结**：后续所有精简都与此对比）

### 2.1 总量

| 类别 | 行数 | 说明 |
|---|---:|---|
| Rust 生产（不含内联 test） | **67,806** | 15 个 crate |
| Rust 测试（内联 16,231 + 集成/示例 13,129） | 29,360 | 测试/生产 = 43% |
| Dart `lib` | **33,476** | Flutter Web |
| Dart `test` | 28,996 | 测试/生产 = 87% |
| docs `*.md` | **69,733**（298 份） | 比 Rust 生产还多 |
| 仓内残留 Python | 2,674 | 只剩 scripts/tests |

### 2.2 Rust 生产按职责（分母 67,806）

| 组 | 行数 | 占比 |
|---|---:|---:|
| 平台-桌面壳 / WebAPI | 30,135 | 44.4% |
| Mod 体系（10 crate） | 18,347 | 27.1% |
| 主链-渲染面（`l2d` + `l2d-wasm-demo`） | 8,017 | 11.8% |
| runtime-配置 / 密钥 / 其它 | 5,163 | 7.6% |
| **主链-对话 / LLM / TTS / 音频** | **3,479** | **5.1%** |
| 主链-核心状态机 | 2,240 | 3.3% |
| xtask | 425 | 0.6% |

⇒ **功能本体（对话+音频+渲染+状态机）≈ 13.7k 行（20.2%）**；其余 ~80% 是平台、Mod 框架、配置、工具。

### 2.3 与 Python 旧版对照（`py-legacy` bundle 实测）

| | Python 旧版 | 现在 |
|---|---:|---:|
| 主语言 | 277 `.py` = **52,669** 行 | Rust 生产 **67,806**（1.29×） |
| 含测试 | — | Rust 97,166（1.85×） |
| 前端 | 88 `.ts` = 19,533 行 | Dart 62,472（lib + test） |
| 文档 | 25,825 行 | 69,733（2.7×） |

### 2.4 规则违反与集中度

- `crates/*/src` 生产 `.rs` **>500 行 = 58 个**；**>1000 行 = 4 个**：
  `mod_registry.rs` 1363 · `web_api/chat_routes.rs` 1087 · `web_api/external_routes.rs` 1063 · `live2d-ai-mod-persona/src/lib.rs` 1005。
- Dart **>800 行 = 7 个**：`dev_tools_section.dart` 1875 · `appearance_background.dart` 1552 · `main.dart` 1345 ·
  `display_prefs.dart` 1158 · `app_shell.dart` 999 · `settings_models.dart` 932 · `tokens.dart` 929。
- **休眠 / 非主路径代码 = 11,153 行 / 32 文件**：egui 壳 `app/`、`tray`、`repl`、`benchmark`、`model_smoke`、
  `local-llm`（DEPRECATED）、`wallpaper`/`pet-desktop`（ARCHIVED）、core 的 `action/` + `performance/`。

### 2.5 依赖与产物

- Cargo 依赖：`live2d-ai-desktop` **34** · `live2d-ai-runtime` 12 · `l2d` 8。
- 产物：`shell/flutter/build/web` **47M** · `crates/l2d-wasm-demo/dist` **5.4M** · `target/` **19G**。
- 测试数量：Rust **1460** 条 · Dart **1281** 条。

### 2.6 文档与审计

- docs 分布：plans 115/29,961 · architecture 32/9,532 · research 23/8,937 · verification 18/4,420 ·
  design 2/2,652 · releases 14/2,372 · audit 6/1,863 · legacy 10/1,350。
- 审计：**91 条有效发现（P1 9 / P2 40 / P3 42，P0 0）**，已耗 **≈2,374M token**，
  覆盖 ≈70/483 文件（15%）；账本 `AUDIT-REPO/` = 942 文件 / 6.05MB / 67,435 行 ≈ 全部 Rust 生产代码行数。

## 3. 债务真源索引（**不在本文重复列条目**）

| 域 | 真源 | 条数 |
|---|---|---|
| 全库审计 | `AUDIT-REPO/FINDINGS.md` + `STATE.md` | 91（P1 9） |
| 前端审计（封口） | `docs/audit/2026-09-28-frontend-nightly/` + `docs/plans/TRIAGE-0.2.0-audit-45-2026-09-28.md` | 45（已关 16 / rc.8 顺延 29） |
| Mod 侧 Rust 债务 | `docs/audit/2026-09-15-five-mod-review.md` | S1–S6 / M1–M12 / L1–L16 |
| Mod 链性能 | `docs/research/2026-09-15-mod-chains-perf-and-single-session-memory.md` | P-1…P-15 |
| 规则违反 | 本文 `2.4 | 58 / 4 / 7 |
| 09-27 计划债务分级 | `PLAN-0.2.0-seal-and-cleanup-2026-09-27.md` `4 | P0/P1/P2/P3 |

## 4. 现在（Now）：0.2.0 收口

### S0 · 文档整理（本波，0 代码）

- [x] 本文 + `docs/DOC-MAP.md` + 归档候选 manifest + `docs/README.md` 索引（**本轮落盘，未提交**）
- [ ] docs-only 提交：`docs(chore): DOC-MAP + 0.2.0 收口/去臃肿计划 + 归档清单`
- [ ] 归档执行：94 份 → `docs/legacy/plans/` + 全仓改链（**单独 commit**，步骤见 `DOC-MAP.md` `4.1）
- [ ] `AGENTS.md` 单一化（以 `-fe` 版为底），消除 `-Ai` 旧 doctrine 的残留引用
- [ ] `CHANGELOG.md` 去留裁决；`docs/releases/v0.3.0.md` 标"历史草案，未发布"

> **2026-10-06 复核**：上面四项均已执行（归档 `b9eff54e` / 完成点 `4285af8b`、AGENTS 单一化、
> `CHANGELOG.md` 停更头注、`v0.3.0.md` 标作废）。**新增一条**：2026-10-06（E8 文档减量）把
> `docs/legacy/` 整目录**移出工作树**——归档只解决「活 / 历史混放」，不减行数；取回见
> [`docs/REMOVED-docs-index-2026-10-06.md`](../REMOVED-docs-index-2026-10-06.md)。

**判据**：`grep` 归档名在 `docs/plans/` 场景下为 0；`docs/README.md` 无断链；AGENTS 只剩一份。

### S1 · `rc.8-a` 正确性与诚实性（**先修后删**）

| 波次 | 内容 |
|---|---|
| R8a-1 | `F-0008-1` 心跳看门狗 + `F-0007-1` 本地失败不回落相位（**必须同波次**） |
| R8a-2 | `F-0012-1` + `F-0003-2`：自检成败禁止扫 `contains('ok'/'ms')`；成功结果必须有渲染槽 |
| R8a-3 | `F-0005-4` 气泡 `excludeSemantics` 吃掉重试/复制；`F-0003-6` Mod 配置 re-seed 吞草稿 |
| R8a-4 | `F-0006-2` 白主题文字对比度 3.96<AA；`F-0006-3`；`F-0005-5` 字体门禁覆盖运行时文本 |
| R8a-5 | `F-0010-2` / `F-0001-4` progress/fps 无出口；`F-0004-1` 记忆面板跨桶 |

**判据**：每条 P1 红-绿双向 + 非实施者复核；cargo 全门禁 + flutter analyze/test 全绿。

### S2 · `rc.8-b` 结构 + D0 / D1（见 `5）

### S3 · `0.2.0` 末版

```bash
cd /home/skystar/Live2D-Ai-fe && git checkout main && git merge --ff-only feat/frontend-redesign
```
版本三处（`Cargo.toml` / `pubspec.yaml` / `README`×2）→ 完整门禁 →
肉眼 8 项落成 `docs/verification/v0.2.0-rc.8-checklist.md`（**该文件尚不存在**）→ tag → release note。**不推远端。**

## 5. 去臃肿专线（Debloat D0–D6）

### D0 · 度量与门禁（**先做，否则精简没有刻度**）

- `xtask` 新增 `code-stats`：逐 crate prod/test/dart/docs 行数 + 超限文件清单 + 依赖计数 +
  产物体积，输出可直接进 release note 的 markdown。
- CI 加三条硬门禁：文件行数（`src .rs >500` / `Dart >800`）、`crates/*/src` 超 1000 行 = 0、依赖计数上限。
- **判据**：本地与 CI 同一条命令；本文 `2 的数字可一键复算。

### D1 · 死代码 / 休眠资产裁决（**最大单项**）

- 对象：`2.4` 的 32 文件 / 11,153 行（egui 壳、tray、repl、benchmark、model_smoke、local-llm、wallpaper、pet-desktop、core `action/`+`performance/`）。
- 三条出路：**① 删**（先归档分支/文档）；**② 移出 workspace**（crate 保留、不编译）；**③ feature-gate**（rc.3 曾裁"不 gate"，因需求改为精简，**需重新裁决并记录**）。
- 收益：~11k 行 + `desktop` 依赖 34→~22 + 编译时间与 `target/` 体积。
- **判据**：休眠行数归零，或"显式冻结"（有台账 + 不参与构建 + 有恢复条件）。

### D2 · 结构拆分

| 现值 | 目标 |
|---|---|
| `mod_registry.rs` 1363 | 拆 3 文件（注册表 / 生命周期 / manifest 持久化） |
| `chat_routes.rs` 1087 · `external_routes.rs` 1063 | 各拆 2–3 |
| `persona/src/lib.rs` 1005 | 拆 2 |
| `dev_tools_section.dart` 1875 · `appearance_background.dart` 1552 | 各拆 3–4 |
| `main.dart` 1345 · `display_prefs.dart` 1158 | 各拆 2–3 |
| `app_shell` 999 · `settings_models` 932 · `tokens` 929 | 各 ≤600 |

**判据**：`src` 生产 `.rs >1000` = 0；`>500` ≤15；Dart `>800` ≤2。

### D3 · 重复消除

- 已确认副本：`formatWsError` ×3 · `decodeDataUrl` ×4 · `redact` ×4 · `effective_token` ×2；
  审计另记 `stripCommentsAndStrings` 在测试下 ×8（其一已漂移）。
- 抽成单一定义 + 一条"不得再复制"的回归。
- **判据**：同名 helper 单一定义；新增断言。

### D4 · 依赖与构建瘦身

- `live2d-ai-desktop` 34 → **≤22**（与 D1 联动：egui/egui-winit/egui-wgpu/winit/ksni/raw-window-handle 等视裁决）。
- 产物体积预算：`flutter web` 47M → ≤35M；`wasm dist` 5.4M → ≤4M（tree-shaking / 字体子集 / 去重）。
- **判据**：依赖数与体积进 `code-stats`。

### D5 · 文档瘦身

- 归档 94 份；`docs` 69,733 → **≤45,000**；`docs/plans` 115 → **≤25**。
- **判据**：`DOC-MAP.md` §3 复算。
- **✅ 2026-10-06 达标（E8 文档减量）**：docs **不含 `docs/audit/**`** = **39,273 ≤ 45,000**（余量 5,727）；
  `docs/plans` 顶层 = **21 ≤ 25**。做法 = **移出工作树 139 文件 / 31,509 行**（不是搬运、不是压缩），
  逐份登记 + `git show` 取回见 [`docs/REMOVED-docs-index-2026-10-06.md`](../REMOVED-docs-index-2026-10-06.md)。
  **并已变成机器判据**：`cargo run -p xtask -- code-stats --check --only docs`（门禁组 `docs`，判**不含 audit**），
  单独接进 `pr-checks.yml`。**增长纪律**：逼近预算时按 `DOC-MAP` §4「移出四步」处理，**不是**调大常量。

### D6 · 测试治理（**不砍覆盖，砍重复与假绿灯**）

- Rust 1460 / Dart 1281 条**不减**；空转断言、源码字符串扫描式测试、恒真守卫清零（假绿灯已有判据）。
- 重复夹具/helper 合并；测试文件 `>800` 行拆分。
- **判据**：覆盖不减 + "判别力自查"（把断言改错必须变红）。

### 量化目标（总表）

| 指标 | 现在 | 目标 |
|---|---:|---:|
| Rust 生产 | 67,806 | **≤54,000（−20%）** |
| Dart `lib` | 33,476 | **≤28,500（−15%）** |
| `src .rs >1000` | 4 | **0** |
| `src .rs >500` | 58 | **≤15** |
| Dart `>800` | 7 | **≤2** |
| 休眠/封存 | 11,153 | **0 或显式冻结** |
| `desktop` 依赖 | 34 | **≤22** |
| `docs/plans` | 115 | **≤25**（**2026-10-06 已达：21**） |
| docs 总行数 | 69,733 | **≤45,000**（**2026-10-06 已达：39,273**，不含 `docs/audit/**`） |
| 测试条数 | 1460 / 1281 | **不减** |

## 6. 未来（Beyond 0.2.0）

### 6.1 工程（`0.3.0`）

- **Mod 框架重审**（18,347 行 / 27%）：5 个在册 Mod 是否值这个体量；`mods.json` 写回、`HostChannels`、锁与生命周期（S1–S6 / M1–M12）。
- **preset 数据化**：`l2d-wasm-demo/src/preset` 5,242 行 → 由 `assets/actions/presets.json` 驱动，Rust 只留运行时。
- **`web_api` 收敛**（18,963 行）：路由 / dispatch / WS 分区瘦身。
- **性能预算入 CI**：启动、首帧、内存、构建时间（benchmark 工具保留但独立于产品入口）。

### 6.2 功能

- **R1** 正文帧带 `sentence_seq`（朗读高亮，**唯一"不做就永远做不出来"**的一条）。
- **会话记忆后端**（`history` 字段 + 端点 + 持久化；先裁决是否真多会话）。
- **动作 / 语音选型**（`PLAN-actions-voice-memory-2026-09-15.md`；实施前先改 AGENTS 的 director 台账措辞）。
- **壳工程**（Tauri / Android 悬浮窗）：独立 crate / 独立仓，不污染核心。

### 6.3 审计

- **冻结**长跑审计；只做有界补审（3 个最大 Dart + 123 个从未提及文件），
  设 token 上限 + "每批至少新增 1 个覆盖文件"的机械校验，避免 13 批/文件的空转。

## 7. 红线（**不得为"精简"牺牲**）

1. 主链契约：WS 帧**只增不改**；`clean_for_tts` 不得旁路；上屏 == 送 TTS。
2. 错误可观测：错误码两侧同源；**前端不得从显示文案猜错误类型**。
3. 离线优先：`--no-web-resources-cdn` + 自托管字体 + 界面零外部源。
4. 密钥：`.env` 真源、永不回值、loopback / Origin / CT。
5. 表演资产 A1–A8（preset 下发 / `action_cue` / ack / baseline / 导演可观测）逐个保留语义。
6. Mod 契约与 `mod_count` 断言；**待机生命体征 `IdleState` 必须保留**。
7. 测试条数不降；删任何测试必须给出"其行为已被其他断言覆盖"的证据。

## 8. 顺序与依赖

`S0（文档）→ S1（rc.8-a 修 bug）→ D0+D1（rc.8-b 刻度 + 休眠裁决）→ D2/D3/D4（rc.9 结构/重复/依赖）→ D5（文档归档，可与 S1 并行）→ D6（测试治理）→ 0.2.0 末版`

理由：先修真 bug（删码会掩盖问题）→ 再建刻度（D0）→ 再删最大块（D1）→ 最后动结构与测试。

## 9. 验收与回滚

- 每阶段：`cargo test --workspace --all-targets` + `--doc` + `fmt --check` + `clippy -D warnings` + `rust-ratio` +
  `flutter analyze` + `flutter test` + `./scripts/ignite.sh --check` 四项 + 涉及界面时的肉眼项。
- 每阶段 tag：`checkpoint/<阶段>`。
- 回滚：`git revert <commit>` 或 `git checkout <tag> -- <path>`。

## 10. 本文数字的可复算来源

- 体量：本次实测脚本思路（按 crate / 按目录 prod-test 拆分 / 超限清单）→ **D0 落成 `xtask code-stats`**。
- 审计：`AUDIT-REPO/STATE.md` `3/`5；前端：`docs/plans/TRIAGE-0.2.0-audit-45-2026-09-28.md`。
- 版本与门禁：`docs/releases/v0.2.0-rc.7.md`。
