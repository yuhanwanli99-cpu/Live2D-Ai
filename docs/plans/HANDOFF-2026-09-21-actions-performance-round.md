# HANDOFF 2026-09-21 —— 动作 / 表情 / 导演链路（用户可见症状 → 契约 → 波次交付）

> 交接人：本轮 agent（**只做调研 + 裁决 + 验收，没有写产品代码**）。接手人：任意 CLI agent。
> 工作树：**`/home/skystar/Live2D-Ai-l1`**（git worktree，分支 `mod/l1-product`）。
> **不 bump / 不 push / 不打 tag。** 另一个 worktree `/home/skystar/Live2D-Ai` 是另一条分支，**不要碰**。

## 0. 一分钟上手

| 你要的东西 | 在哪 |
| --- | --- |
| **规格真源**（产品形态 / 目标链路 / 时间轴 / 输出契约） | `docs/plans/RESEARCH-actions-director-audit-2026-09-21.md`（604 行）—— **§8 / §9 / §10 是维护者已确认的规格**，§1–§4 是症状与证据 |
| **任务真源**（W1–W11 十一个块，每块含公共前置 + 文件归属） | `docs/plans/IMPL-PROMPTS-actions-performance-round.md`（598 行） |
| **调度真源**（编排者提示词 + 可粘贴指令） | `docs/plans/ORCHESTRATOR-PROMPT-actions-performance-round.md`（391 行）；**§8 首次启动 / §9 Wave1 续作 / §10 Wave2 续作** |
| 调研与复算的原始产物 | `/tmp/baseline-deadzone.txt`（死区复算，359 行）、`/tmp/w0-*.log`、`/tmp/w1-*.log` |
| 运行时配置基线备份 | `/tmp/live2d-ai.toml.baseline-20260921-205756`（md5 `6da460c676a18a2e9f6c61b793a71010`） |

**进度：Wave 0 ✅ 已验收 / Wave 1 ✅ 已验收；Wave 2 – Wave 5 未做。**
**恢复动作**：把 `ORCHESTRATOR-…md` 的 **§10** 整块复制给编排者，即可继续 Wave 2（W7）。

## 1. 这个回合要解决的四件事（用户原话）

| # | 症状 | 根因（已定位并修/待修） | 状态 |
| --- | --- | --- | --- |
| ① | 动作测试会附带之前的老表情 | 调试面板「叠加基础表情」默认开 + `_expressionId` 永久粘住 ⇒ 点手势把旧表情 2.6s 重新起算 | **W3 已修默认**；开关文案等 4 条欠账转 W7 |
| ② | 幅度像「编译死了」、上下左右只动上半身、临时幅度覆盖不生效 | 出厂倍率把 `ParamBodyAngle*` 顶到 ±10 上限（滑条 1.43 以上无效）；`Live2DStage.actionScales` 是从未传过的死参数；临时覆盖与防抖下发器两个写者共用一个通道 | **W2 已重标定**（MAX 2.2 / 表值 12.0-3.9 / 出厂 0.75-0.80-1.0）；**W7 修接线**（未做） |
| ③ | 导演系统理解和实现脱轨（调研 NEKO） | 三件事：与自己的 `director-rfc.md` 红线冲突、两套契约相反的二路 LLM、前端两条未协调的驱动通道 | **W8 已治理文档**、**W9 单一驱动者未做**；目标契约（三字段 / 只切不改字 / 时间轴）**W10/W11 未做** |
| ④ | 其他工程小错误编入下一轮 | `join_endpoint` 4 份实现、注释漂移、令牌绕过 `.env` 等 | **W5/W6 已做**；残留三条 Dart 注释转 W7 |

## 2. 已冻结的裁决（**不得翻案**；下一轮任何人先读这一节）

### 2.1 产品形态（维护者 2026-09-21）
> 「也可以理解成**酒馆 套 live2d 皮套壳子**。本身目前的 **mod 矩阵不就是在实现相关功能**吗。」

⇒ 产品 = **「酒馆（类酒馆角色扮演内核）+ Live2D 皮套壳子」**：人设由内核稳定，**各功能由现有 mod 矩阵承担**，
皮套负责情绪表达；**导演是一个 AI**（不是给用户操控皮套的辅助、也不是普通场景 Mod）。**不做架构搬迁。**

### 2.2 目标链路（维护者描述，已确认）
```text
用户消息
 → ① 表演/前端 LLM（接酒馆人设）出回答（含思考）            【第 1 次 LLM 调用】
 → ② 路由 = 大脑/导演 LLM（独立端点；**不与①共用上下文**）   【第 2 次 LLM 调用】
      产出：段切分 + 三字段（1 半身 / 2 头部 / 3 表情）
 → ③ TTS 产出首个音频并播放（**嘴型由 TTS 驱动**）；同步小句回复 + 连续动画
 → 链路可**整体断开**并回初始：① 用户按停止  ② 再收到用户消息
      断开时清：动作 / 表情 / TTS 待播放 + 退回**该会话**的角色基础状态值
```

### 2.3 逐条不得翻案的清单
| # | 裁决 |
| --- | --- |
| R1 | 情绪/表演决策的**输入 = 用户输入**（**不是**角色回复） |
| ✗ | ~~「导演属场景 Mod / 主链不该有第二 LLM」~~ 是**我的推论、已被维护者否定**，不得再引用，也不要做架构改造建议 |
| Q1 | 过滤层**不允许改写**原文、**允许断句**；`segments` 拼接必须**逐字等于**原文（硬断言） |
| Q2 | 三字段 = `body`（半身 4 向，可按比例合成）/ `head`（头部 4 向）/ `expression`（面板表情）；**同类可重复、按 add 合成**；每字段带强度 |
| Q2′ | 每皮套的「基础表演强度」**复用现有 `[action]` 三个倍率**（body↔字段1、head↔字段2、expression↔字段3），**不新增第四套** |
| Q2″ | **非标准参数（翅膀/耳朵/光环/袖子/头发）本轮不做**；不同皮套的支持**只**靠用户手调强度基础参数 |
| Q8 | **hold 可加**：带 hold 的条目保持到下次指令，不参与 ttl 到点撤销 |
| Q9 | 输出**立即生效**、不排队、**不需要动作队列**；忽略 TTS 首 token 延迟；动作的发生本身作为喂导演的输入段点 |
| Q4′ | 唤醒 = ① 该段 TTS 播完 **且** ② 动作做完（hold-only 段退化为只看 TTS） |
| Q7′ | 断开 = 清「动作 + 表情 + TTS 待播」+ 退回**该会话**的角色基础状态值（baseline 绑会话） |
| Q6′ | 时间轴统一到**音频时钟**（`stage-clock`）；段边界用断句给的锚点 |
| S1 | **是**：表情字段**只写五官**，头/身完全交给字段 1/2（避免两族抢同一参数） |
| S2 | **是**：`at` 取 `now` / `seg:N` / `after_prev` |
| A′ | W2 采用方案 **(i)**（下调表值满足 intensity 走满），**MAX_ACTION_SCALE = 2.2**（不是 1.29）；表值 head 12.0 / body 3.9 / surprised 眼 1.26；出厂 0.75 / 0.80 / 1.0；身/头比窗口 表内与出厂后都 ∈[0.30,0.50] |
| A″ | W3 的叠加语义裁 **(i)**：默认已修（叠加关）；显式打开后「点手势会把当前基础表情重新起算」——**开关文案要写实**（转 W7） |
| 口径 | 死区验收线 = `上限×0.95/(表值×峰值)`（留 5% 余量）；RESEARCH §2.1 的表是**钳死点**（无余量）。核对必须同口径对同口径 |

### 2.4 两条硬约束（工具链层面）
- **碰 `shell/flutter/**` ⇒ 必须** `flutter build web --release --base-href /app/ --no-web-resources-cdn`（前端产物不改不生效）；
- **碰 `crates/l2d-wasm-demo/**` ⇒ 必须** `cd crates/l2d-wasm-demo && trunk build`，并复核 `dist/` mtime 晚于最后一个被改的 `.rs`
  （这条是 W0 实测踩出来的：原红线只写了 `.dart`，wasm 产物陈旧无人发现。已补进两份提示词。）
## 3. 已交付并已验收（Wave 0 / Wave 1）

| 波次 | 块 | 内容 | 我独立复核的结论 |
| --- | --- | --- | --- |
| W0 | **W1** | 文档对齐（AGENTS / L1-GOALS / action-packs / README） | 真源路径 26/26 存在；引用 RESEARCH 逐字一致 |
| W0 | **W2** | 幅值重标定 | **我从源码重算**：四条硬指标违规 NONE；主轴死区起点 2.500/2.564/2.693/2.762 **全部 > 2.2**；身/头比 表内 0.3250 / 出厂后 0.3467；五处默认值一致 |
| W0 | **W3** | 调试面板（默认不叠加 + id 列表派生） | `_overlayFace = false`；`kDebug*` 硬编码 0 命中；浏览器实测（编排者做）表情到点后 UI 不谎报、点手势不重发 |
| W0 | **W6** | 三处令牌走 `secrets::lookup` | 生产路径 `std::env::var` 命中 **0**（剩下 5 处全在测试文件） |
| W0 | **W8** | 导演口径与文档漂移治理 | `不建议`/`辅助用户` 0 命中；Q1 三处一致标「未定」 |
| W1 | **W4** | 撤销语义（`none` 真的归零） | 我读了原文：`:104` **先去重 → :110 再归零**（顺序即契约）；编排者浏览器实测：中性轮发 `{"id":"none"}`、HUD 两槽归零 |
| W1 | **W5** | Rust 卫生（`join_endpoint` 收敛 + 注释漂移 + `:497`） | `fn join_endpoint` 只剩 **runtime 1 处 + 两个 Mod 跨边界副本**；`唯一实现`=0；`preset.rs`=0 |
| W1 | **W8b** | 三份文档漂移收尾 | 「动作在产品路径上不存在」0 命中 |
| W1 | **W2b** | 标定口径 + T3 表（`action-packs-v0.md` §11 + `presets.json` 的 `_doc`） | §11 在 :186；`presets.json` 仍 9 条、标定值在（nod −12/−3.9、surprised 1.26） |

### 门禁基线（供后续回归对照；全部由编排者亲跑，我核过日志）
```
W0: cargo test --workspace --all-targets  → 1394 passed / 0 failed
    cargo test --doc / fmt --check / clippy -D warnings → 全绿
    rust-ratio → 97.1829% PASS        flutter analyze → No issues
    flutter test → +1036 All tests passed        flutter build web → exit 0
    trunk build → exit 0             ignite.sh --check → 四项全 ok
W1: cargo test → 1394/0（我求和确认）    rust-ratio → 97.1830% PASS
    flutter test → +1041 All tests passed
    cargo check --target wasm32-unknown-unknown -p l2d-wasm-demo → exit 0（4 条既有 dead_code 警告）
```

## 4. 未完成的工作（逐条，带归属与前置）

| # | 事项 | 归属 | 前置 / 备注 |
| --- | --- | --- | --- |
| **W7** | 幅度下发通道整修（Wave 2，**已放行**） | 编排者派 worker | 含 Wave 1 转来的 **4 条欠账**：A 开关文案写实 / B 两个消费点优先标签表回落常量 / C `director_panel.dart` 三处文案 / D 三处陈旧注释（`live2d_stage.dart:156`、`shell_prefs.dart:31`、`live2d_bridge.dart:205`，**最后一条已授权**） |
| **W9** | 单一驱动者（Wave 3） | 同上 | 退役「拉 `latest.preset_id`」那条前端通道，统一到 `action_cue`；与 W7 都碰 `main.dart`/`live2d_stage.dart` ⇒ **必须串行** |
| **W10** | 动作协议字段化（Wave 4） | 同上 | body/head/expression 三字段 + 同类 add 叠加 + hold + **事件回执**（`preset-applied/replaced/expired/dropped`）+ `stage-clock`（音频时钟）；**必须先 `trunk build`** |
| **W11** | 导演 AI 侧（Wave 5） | 同上 | 输出 schema + **使用手册与 schema 同源生成** + 只切不改字（`segments` 逐字等于原文）+ 两条件唤醒 + 会话 baseline；唤醒通道**优先复用**现有 director command，要动 `topics.rs`/`supervisor.rs` **必须先问维护者** |
| flaky 定名 | 20 次 `--no-fail-fast` + 并发窗口 + **tee 全量输出** | 编排者（与 W7 并行） | 一次并发跑里 740 passed 后 1 failed 并 fail-fast 中止；3 次干净 1394/0、2 次定向复现绿。**20/20 绿 ⇒ 记未复现 + 点名风险面（`supervisor/tests_stall.rs`、`web_api/ws/connection.rs`）；复现 ⇒ 立刻停并上报** |
| **TTS 未起** | 端到端仍缺一块 | **用户** | `[tts] base_url = http://127.0.0.1:8080/v1` curl → **000**；导致「真实 PCM 帧 + `turn_state=completed`」始终未取证（现有证据只有 LLM 路径 + `text_delta` + `text_fallback` + `error(tts_transport)` 合同）。**不要求造假绿灯** |
| 未取证① | W2 的「不推送 scales」兜底路径（渲染面 `PRODUCT_DEFAULT`） | 同上 | 主干路径已验（HUD `scale h0.75/b0.80/e1.00`），「不推送」路径需另开探针 |
| 未取证② | W2 的 T3 表在浏览器里的肉眼观感（默认摆幅变小：look 头 16.5°→9.0°、身 9.8°→3.12°） | **用户肉眼** | 这是 (i) 方案的**已接受代价**，需用户确认「变小之后是否还够看」 |

### 4.1 记录在案、**有意不改**的
- `mod-product-chain.md:106` 的「恢复 Action / director / 编舞」非目标：该句明确限定在归档分支 `archive/action-layer-p6`，语句成立 ⇒ **不改**。
- `core-chain-baseline.md:143` / `directory.md:64`：保留为**带「当时」限定的历史句** + 现状更正 ⇒ 不改。
- W5 的两处小取舍（已记录未改）：被删的 `join_endpoint` 副本对 `base` 做 `trim()`，runtime 那份靠 URL 解析器吃空白（仅病态空白 base 有差异）；两个 Mod 未收敛到 runtime（会改掉它们各自的错误口径与空 base 特判）。
- `dev_tools_section.dart` 未拆（**1792 行**）：块内标为可选，且拆分与 W7 冲突 ⇒ 记未决。

### 4.2 已接受的范围外扩（下不为例）
- W2 顺带改了 `settings/{view,patch,patch_tests}.rs` 与 `preset/tests/mod.rs`（门禁必需）；
- W6 顺带改了 `mod-external-input/Cargo.toml`（依赖必需）；
- W5 顺带把 `preset/scales.rs` 的 T3 注释 `nod/shake` 并档 73.5 拆成 `shake 73.5` / `nod 79.2`（**修正了 W2 的一处自相矛盾**，无并发写者，已认）。

## 5. 交接时的现状快照（2026-09-21 实测）
```
分支 mod/l1-product         git status --porcelain = 187 项
shell/flutter/build/web/main.dart.js = 2026-09-21 22:20:39   （0 个 .dart 比它新；gstatic 计数 0）
crates/l2d-wasm-demo/dist/index.html  = 2026-09-21 22:19:00   （0 个 .rs 比它新）
服务：pid 64616  ./target/debug/live2d-ai-desktop --web --http-port 18080   GET / → 302，GET /app/ → 200
      （服务仍在运行；ignite.sh --check 四项全 ok）
```

## 6. 恢复时的三个提醒（都是这个回合踩过的）
1. **门禁命令一律 `--no-fail-fast` + tee 全量输出**。这个回合有一次把输出过滤成只剩 `test result:` 行，结果 flaky 的**用例名丢了** —— 比 flaky 本身更值得修。
2. **磁盘写入要保守**：我曾用 `read(offset=尾部)` 的结果**整份回写**，把 604 行的调研报告截断成 84 行（后来从工具溢出备份里取回并逐条重做）。**读全文再写全文，或只用 edit 做定点替换。**
3. **产物重建不是「改了才要建」**：开工前 `main.dart.js` 就已经比 6 个 `.dart` 旧（上一轮漏的）。每个波次收口都要按第 2.4 节两条硬约束复核 mtime。

## 7. 本轮 agent 没做的事（如实）
- **没写任何产品代码**：只创建/修改了 **4 份 md**（本文件 + RESEARCH + IMPL-PROMPTS + ORCHESTRATOR）；
- **没跑全量门禁**：`cargo test --workspace` / `flutter test` 由编排者跑，我只核日志与数字（求和、grep、`stat`）、单点复算 W2 的不变量、`curl` 服务与 TTS；
- **没做浏览器肉眼验收**：W3/W4 的浏览器实测由编排者做，我复核的是它给出的**原始读数**（HUD 行与帧文本）；
- **没碰 `/home/skystar/Live2D-Ai`**，没做任何 git 写操作（无 commit / stash / checkout）。

---

## 8. 文件清单（本回合产出，全在 `/home/skystar/Live2D-Ai-l1/`）
| 文件 | 行数 | 作用 |
| --- | --- | --- |
| `docs/plans/RESEARCH-actions-director-audit-2026-09-21.md` | 604 | **规格真源**：§8 目标链路 / §9 时间轴 / §10 输出契约（含 §10.7 全部答复落地） |
| `docs/plans/IMPL-PROMPTS-actions-performance-round.md` | 598 | **任务真源**：W1–W11 十一个块 + 裁决点（已全部裁定） |
| `docs/plans/ORCHESTRATOR-PROMPT-actions-performance-round.md` | 391 | **调度真源**：§8 首次启动 / §9 Wave1 续作 / §10 Wave2 续作 |
| `docs/plans/HANDOFF-2026-09-21-actions-performance-round.md` | 本文件 | 交接 |

（其余改动均属 Wave 0/1 的 worker 产出，见各块的【文件归属】；本轮未纳入版本库，全部**就地未提交**。）
