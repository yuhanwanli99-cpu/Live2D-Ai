# 可复制实现提示词（W1–W9）：动作/表情/导演链路修复

用法：一个 worker 一块，按波次顺序整块粘贴（每块已含公共前置）。
调研真源：`docs/plans/RESEARCH-actions-director-audit-2026-09-21.md`（每个任务的背景与 file:line 证据都在里面，
worker 开工前应读相关章节）。调度真源：`docs/plans/ORCHESTRATOR-PROMPT-actions-performance-round.md`。

波次（文件零重叠才可并发）：
- **Wave 0（可立即派，6 路并行）**：W1 文档对齐 / W2 幅值重标定 / W3 调试面板 / W5 Rust 卫生 / W6 令牌走 secrets / W8 导演口径与文档漂移治理（docs-only）
  （W5 在 settings.rs 里只改 :594 那段文档；若与 W2 撞同一文件，等 W2 收口。
   W8 与 W1 文件不重叠，但**与 W4 共用 docs/architecture/performance-layer-v0.md** ⇒ W8 必须在 Wave 0 收口，W4 才能开工）
- **Wave 1（W2 / W8 收口后）**：W4 撤销语义（独占 main.dart / live2d_stage.dart）
- **Wave 2（W4 收口后）**：W7 幅度下发通道整修
- **Wave 3（W7 收口后）**：W9 单一驱动者（退役 latest.preset_id 那条通道，统一到 action_cue）
- **Wave 4（W9 收口后）**：W10 动作协议字段化（body/head/expression）+ 叠加 + hold + 事件回执 + 音频时钟基准
- **Wave 5（W10 收口后）**：W11 导演 AI 侧（输出 schema / 使用手册同源 / 段切分 / 两条件唤醒 / 会话 baseline）
  后三个波次是**同一条链路上的递进改造**（协议 → 渲染面 → 导演侧），必须串行，不要并行抢文件。
- 同一波次不要在同一棵工作树上并发改同一个文件；每个块末尾都写了【文件归属】。

验收底线（全部完成后逐条核）：
1. 两个旋钮**各自独立走满都不触上限**：scale 全程 0.2…MAX、intensity 走满（或按选定方案收到新上限）；
   出厂组合 ≤ 上限的 60%；两者**同时**拉满的预期钳位组合必须被算出并写进文档（不许装作不存在）。
2. 手势包「表内身/头」与「出厂后身/头」都落在 [0.30, 0.50]（**主轴配对**：look_*/shake→X 对、nod→Y 对、tilt_*→Z 对）。
3. 调试面板**默认不叠加**表情；表情到点后 UI 不再声称在演，点手势不再把它续期。
4. 「本轮没有预设」会让舞台两槽**显式归零**；文档不再声称「空 action_cue 会清残留」。
5. 临时幅度覆盖：改主题 / 改任意 DisplayPrefs 后**仍然生效**；「恢复产品设置」一定生效；iframe 重建后幅度不丢。
6. `grep -rn 'fn join_endpoint' crates/ --include=*.rs` 收敛到 runtime 1 处 + 两个 Mod 的跨边界副本。
7. 三个本地令牌不再出现 `std::env::var`。
8. 门禁全绿 + `flutter build web` 产物 mtime 晚于最后一个 .dart 改动 + `./scripts/ignite.sh --check` 四项 ok。
9. **只切不改字**：导演输出的 `segments` 拼接后**逐字等于**主模型原文（含标点/换行/emoji 样本）。
10. **三字段 + 叠加 + hold + 事件回执**：body/head/expression 可同类叠加；hold 保持到下次指令；渲染面发出 `preset-applied/expired/dropped` 事件。
11. **时间轴统一**：动作进度用音频时钟（`stage-clock`），不再用 `performance.now()`；渲染面缺该消息时回落墙钟。


================================================================
===== 复制给 Worker W1 =====
================================================================
[工作区] /home/skystar/Live2D-Ai-l1（git worktree，分支 mod/l1-product，约 178 个未提交改动）。不要动 /home/skystar/Live2D-Ai（另一个 worktree，另一个分支）。
[开工前读] 本任务点名文件各自的模块头注；以及 docs/plans/RESEARCH-actions-director-audit-2026-09-21.md 里与本任务相关的章节（那份调研带 file:line 证据与量化表，先读再动手）。
[硬约束] 源码 ≤500 行（豁免 ≤1000 需头注写理由），测试文件 ≤800 行；禁止 git worktree / checkout / switch / stash / reset / commit / clean（会丢 WIP）；禁止不带 --check 的 cargo fmt --all，禁止仓库级 dart format；不做没被点名的重构；不确定就停下，写清哪里不确定。
[门禁·Rust 改动必跑] cargo test --workspace --all-targets；cargo test --doc --workspace；cargo fmt --all -- --check；cargo clippy --workspace --all-targets -- -D warnings；cargo run -p xtask -- rust-ratio（≥95%）
[门禁·Flutter 改动] cd shell/flutter && flutter analyze && flutter test
   ★ analyze/test 不是产物：只要有 .dart 改动，必须重建前端，否则界面改动在 /app/ 上不生效——
     flutter build web --release --base-href /app/ --no-web-resources-cdn
     （--no-web-resources-cdn 不可省，缺省会走 gstatic CDN，断网白屏）
   ★ **同理**：只要有 `crates/l2d-wasm-demo/**` 的改动，必须重建渲染面：`cd crates/l2d-wasm-demo && trunk build`，
     并复核 `dist/` 的 mtime **晚于**最后一个被改的 `l2d-wasm-demo/**/*.rs`。
     （2026-09-21 实测踩过：wasm 产物陈旧而无人发现，而原红线只写了 `.dart`。）
[回报] 改动文件清单 / 新增或修改的测试名 / 每条门禁的实际输出数字 / 未决问题。禁止用「应该没问题」「大概通过」代替门禁原始输出。

【任务 W1：文档对齐——把「动作/表演现行状态」写进文档（只改文档，不写代码）】
问题：AGENTS.md（-l1）仍写「LLM 工具层与动作系统已整体拆除」（:112）、「动作在产品路径上不存在」（:256-268），
docs/plans/PRODUCT-L1-GOALS-2026-09-15.md:39 的非目标写「不复活 Action」、director「本轮不做」，
而分支现实是：[action] 段存在、9 条动作包（assets/actions/presets.json）、渲染面 preset 协议存在、
director 在 mods.json 里缺省启用、[performance] 段存在但缺省关。文档与代码不一致会直接误导下一个执行者。
必做：
1. 在 AGENTS.md（-l1）的「动作与表演的归属（休眠台账）」一节后新增小节「动作与表演的**现行状态**（2026-09 实测）」：
   逐条列出 ① 动作包（表情/手势）经 preset 帧直接驱动渲染面参数、② [action] 幅度倍率存在、
   ③ director Mod 缺省启用且会产出 latest.preset_id 与 action_cue、④ [performance] 段存在但缺省关、
   ⑤ core 的 action/performance 子系统仍无驱动方（动作包走的是渲染面参数层，不经 core reducer）。
   每条都要写「真源文件」，不要只写结论。
2. 把 :112 与 :256-268 里**已不成立**的断言改成带现状更正的说法（保留历史沿革，但不能再留下无条件断言）。
3. PRODUCT-L1-GOALS-2026-09-15.md：在 §5「非目标」的「不复活 Action」与 §2 表格 director 行后各加一句现状更正
   （该文件是 2026-09-15 的历史任务书，**不要重写历史结论**，只加「已变更 / 现状见 …」的指引）。
4. docs/architecture/action-packs-v0.md：加一节「与实现的偏差（2026-09 实测）」，把 RESEARCH 报告 §3.2 的 RFC 冲突表
   与 §3.4 的清单**原样引用**。⚠ §3.4 的 D1 那行**已被维护者裁决撤回**（RUN 报告 §3.6），引用时必须**连撤回标记一起引**，
   不得只引 D1 的结论——否则等于把已被否掉的口径又写回文档。
   **director-rfc.md 不在本块范围**（归 W8，避免同文件并发）。
5. docs/README.md 索引补上新文档：本轮的 RESEARCH 报告 + 本轮两份提示词文件。
6. **把产品口径写进 AGENTS.md**（维护者 2026-09-21 裁决，原文照抄，不要改写）：
   「本项目 = **Live2D 皮套 + AI 接入的底座**，由 **Mod 分化各场景**。因此导演/表演属**场景能力**、不属底座能力；
     情绪/表演决策的**输入是用户输入**（用户说了什么 → 皮套怎么反应），**不是角色回复**。
     后者属于『角色有自己心理』的产品形态，不是本项目底座要表达的东西——**不要**照别的项目（如 N.E.K.O 分析角色回复）改回去。」
   放在「动作与表演的现行状态」小节的最后一段，并注明这是维护者裁决、改动需先改本段。
禁止：改任何 .rs / .dart；在文档里下架构裁决（裁决是维护者的事）；重写历史发布说明。
验收：grep -rn '动作系统已整体拆除' AGENTS.md 命中的行后面必须紧跟现状更正或指向新小节；
     新增小节里的每个「真源」路径都能在树上找到（自己 ls 一遍并把输出贴进回报）。
【文件归属】AGENTS.md、docs/plans/PRODUCT-L1-GOALS-2026-09-15.md、docs/architecture/action-packs-v0.md、
           docs/README.md（**只这些**；director-rfc.md 归 W8）

================================================================
===== 复制给 Worker W2 =====
================================================================
[工作区] /home/skystar/Live2D-Ai-l1（git worktree，分支 mod/l1-product，约 178 个未提交改动）。不要动 /home/skystar/Live2D-Ai（另一个 worktree，另一个分支）。
[开工前读] 本任务点名文件各自的模块头注；以及 docs/plans/RESEARCH-actions-director-audit-2026-09-21.md 里与本任务相关的章节（那份调研带 file:line 证据与量化表，先读再动手）。
[硬约束] 源码 ≤500 行（豁免 ≤1000 需头注写理由），测试文件 ≤800 行；禁止 git worktree / checkout / switch / stash / reset / commit / clean（会丢 WIP）；禁止不带 --check 的 cargo fmt --all，禁止仓库级 dart format；不做没被点名的重构；不确定就停下，写清哪里不确定。
[门禁·Rust 改动必跑] cargo test --workspace --all-targets；cargo test --doc --workspace；cargo fmt --all -- --check；cargo clippy --workspace --all-targets -- -D warnings；cargo run -p xtask -- rust-ratio（≥95%）
[门禁·Flutter 改动] cd shell/flutter && flutter analyze && flutter test
   ★ analyze/test 不是产物：只要有 .dart 改动，必须重建前端，否则界面改动在 /app/ 上不生效——
     flutter build web --release --base-href /app/ --no-web-resources-cdn
     （--no-web-resources-cdn 不可省，缺省会走 gstatic CDN，断网白屏）
   ★ **同理**：只要有 `crates/l2d-wasm-demo/**` 的改动，必须重建渲染面：`cd crates/l2d-wasm-demo && trunk build`，
     并复核 `dist/` 的 mtime **晚于**最后一个被改的 `l2d-wasm-demo/**/*.rs`。
     （2026-09-21 实测踩过：wasm 产物陈旧而无人发现，而原红线只写了 `.dart`。）
[回报] 改动文件清单 / 新增或修改的测试名 / 每条门禁的实际输出数字 / 未决问题。禁止用「应该没问题」「大概通过」代替门禁原始输出。

【任务 W2：幅值体系重标定——让倍率滑条全行程有效（症状②的主修）】
已实测的缺陷（见 RESEARCH 报告 §2.1/§2.2，数据是复刻 Rust 公式算出来的）：
  最终值 = clamp_to_channel(id, 表值 x 包络 x 通道倍率)，上限：ParamAngle* = 30、ParamBodyAngle* = 10（preset/scales.rs:13-25）。
  出厂倍率 head 0.75 / body 1.4 / expression 1.0，滑条量程 0.2…2.5。
  结果：body 滑条对 nod/shake/look_* 在 1.43 以上**完全无效**（出厂 1.4 已吃掉 96-98% 行程）；
  head 滑条对 look_* 在 1.36 以上、nod 1.50、tilt 1.88 以上无效。
  另一个副作用：出厂倍率把表内「身约头的 1/3」实际变成 0.47-0.65（躯干摆得比头还显眼）。
**先把「不可能全满足」这件事说清**（这是本任务最重要的一条认知，别去追一个不存在的解）：
  最终值 = 表值 x 峰值系数 x intensity x 通道倍率。**有两个乘法旋钮**：
  intensity（调试面板 0.5-3.0；表演层 cue 合法区间 1..=3）与通道倍率（滑条 0.2..MAX）。
  上限是死的（头 30 / 身 10 / 五官 4）。因此「两个旋钮同时拉满也不触上限」要求
  表值 <= 上限 / (3 x MAX) ≈ 30/(3x2.5) = 4 ——那会让默认摆幅小到看不见。
  **所以要求是「每个旋钮独立走满都不触上限」，不是「整个二维网格都不触上限」。**

必做（四条硬指标，一条都不能少）：
1. **出厂组合不贴上限**：每个 (包, 通道) 满足 |表值 x 峰值系数 x 出厂倍率| <= 上限 x 0.60。
   （峰值系数：Single = 1.0；Oscillate 取 |包络 x 波形| 的数值上确界——shake(cycles=2) 实测 0.9285，
    **必须真的算出来，不许写 1.0**。上限取**运行期**的 clamp_to_channel 口径：头 30 / 身 10 / 五官 4；
    注意表情包的 12/4 是**解析期** pack_limit，不是运行期上限，别用错。）
2. **scale 旋钮独立走满不触上限**：|表值 x 峰值系数 x MAX_ACTION_SCALE| <= 上限 x 0.95。
3. **intensity 旋钮独立走满**：三种选一，并在报告里写明选了哪个与理由（这是维护者的审美裁决，你只需给数据）：
   (i)  下调表值到满足 |表值 x 峰值系数 x 3 x 出厂倍率| <= 上限 x 0.95（观感会变小，附新旧对照表）；
   (ii) 把 MAX_INTENSITY 从 3.0 收到满足第 3 条的值（要同步改 preset/mod.rs 的 MAX_INTENSITY、
        runtime performance plan.rs 的 cue 合法区间 1..=3、以及调试面板两条滑条的 max），并在文档里写清新上限；
   (iii) 保持现状（intensity 高档会钳位），但**必须**把「会钳位的组合」算出来列成表，
        写进 preset/scales.rs 注释与 docs/architecture/action-packs-v0.md，不许假装不存在。
4. **身/头比回到设计口径**：按**主轴配对**算——look_left/right 与 shake 用 ParamAngleX / ParamBodyAngleX，
   nod 用 ParamAngleY / ParamBodyAngleY，tilt_left/right 用 ParamAngleZ / ParamBodyAngleZ；
   每个手势包的「表内 身/头」与「出厂倍率后 身/头」都要落在 [0.30, 0.50]。
   ⚠ 已算出 tilt_left/right 的**表内比现在是 4/16 = 0.25，本来就不达标**（不是倍率造成的），必须一起改。
5. **上限语义不变**：ParamAngle* 上限仍是 Cubism 标准 ±30、ParamBodyAngle* 仍是 ±10；
   不要靠抬高上限来回避问题（那是把红线往后挪，不是修好）。

推荐路径（已按上面 1/2/4 逐条验算过，可直接采用；若另有方案，必须同样满足 1-5 并附数值表）：
  a. 把 MAX_ACTION_SCALE 从 2.5 收到 **1.29**（runtime settings.rs 的 MAX_ACTION_SCALE + Dart maxScale + 文案 + 校验测试）。
     推导：head 最大表值 22（look_left/right），22 x 1.29 = 28.38 <= 30 x 0.95 = 28.5 ✔；
     body 最大表值 7.5（shake，峰值 0.9285），7.5 x 0.9285 x 1.29 = 8.98 <= 9.5 ✔。
     理由：今天 1.43 以上本来就无效，收到 1.29 是**诚实的收缩**，不是砍功能。
  b. 出厂倍率改为 head 0.75 / body 0.80 / expression 1.0（body 从 1.4 收到 0.8，把躯干从 9.8 降到约 5.6，
     正是用户说的「一按就动上半身」的修法；出厂后身/头比回到约 0.34 ✔）。
     **四处必须同步**，缺一处就会出现「界面一套、渲染面一套」：
       (1) crates/live2d-ai-runtime/src/settings.rs 的 DEFAULT_HEAD_SCALE / DEFAULT_BODY_SCALE / DEFAULT_EXPRESSION_SCALE；
       (2) crates/l2d-wasm-demo/src/preset/scales.rs 的 PresetScales::PRODUCT_DEFAULT（渲染面兜底）；
       (3) live2d-ai.toml.example 的 [action] 与其上方注释（出厂说明、取值范围）；
       (4) live2d-ai.toml（**本工作树的运行时配置**，gitignored）就地改值并保留注释；
       (5) shell/flutter/lib/api/settings_models.dart 的 defaultHeadScale/defaultBodyScale/defaultExpressionScale 与 minScale/maxScale；
       (6) shell/flutter/lib/settings/sections/appearance_section.dart 里「出厂 head 75% / body 140% / expression 100%」这句文案。
  c. 新增回归（crates/l2d-wasm-demo/src/preset/tests/）：
     ① 遍历「包 x 通道」断言第 1 条（出厂组合）与第 2 条（scale 走满）都不越线；
     ② 按你选的方案断言第 3 条（intensity 走满，或断言新的 MAX_INTENSITY）；
     ③ 逐通道打印「死区起点」（用 println!，方便下次直接看数字）；
     ④ 断言第 4 条的身/头比区间（主轴配对）。
     以上都要覆盖**内建 fallback 表**（preset/mod.rs 的 PRESETS）与**外置 JSON**（assets/actions/presets.json）两份。
  d. 同步文档：preset/mod.rs 头注的「身约头的 1/3~1/2」、preset/scales.rs 的限值注释、
     assets/actions/presets.json 的 _doc 里关于幅值的描述。表值若改动，_doc 要跟着改。
禁止：改 CoreChain 主链（LLM/TTS/口型）一行；改 preset 的协议字段名；放宽第 5 条的上限；
     只改一处默认值就交（多处不同步 = 本次最想防的 bug）。
验收（回报里必须贴出四张表，缺一张即未完成）：
  T1「每包每通道：表值 / 峰值系数 / 出厂值 / 上限 / scale 死区起点（= 上限 x0.95 / (表值 x 峰值)）」；
  T2「身/头比对照：表内 / 出厂后（按主轴配对），含 tilt_* 从 0.25 修到多少」；
  T3「两个旋钮同时拉满会钳位的组合表」（口径要与文档里写的那张一致）；
  T4 第 1/2/3/4 条断言的实际命令输出（cargo test 的原始输出，不要转述）。
【文件归属】assets/actions/presets.json、crates/l2d-wasm-demo/src/preset/{mod.rs,scales.rs,table.rs}、
           crates/l2d-wasm-demo/src/preset/tests/{packs.rs,assets.rs}、crates/live2d-ai-runtime/src/settings.rs、
           crates/live2d-ai-desktop/src/web_api/settings_routes/tests.rs、live2d-ai.toml.example、live2d-ai.toml、
           shell/flutter/lib/api/settings_models.dart、shell/flutter/lib/settings/sections/appearance_section.dart、
           shell/flutter/test/{action_scales_preview_test.dart,settings_api_test.dart}
（本块独占以上文件；settings.rs 的**注释漂移**也归本块，见 W5 的排除说明）

================================================================
===== 复制给 Worker W3 =====
================================================================
[工作区] /home/skystar/Live2D-Ai-l1（git worktree，分支 mod/l1-product，约 178 个未提交改动）。不要动 /home/skystar/Live2D-Ai（另一个 worktree，另一个分支）。
[开工前读] 本任务点名文件各自的模块头注；以及 docs/plans/RESEARCH-actions-director-audit-2026-09-21.md 里与本任务相关的章节（那份调研带 file:line 证据与量化表，先读再动手）。
[硬约束] 源码 ≤500 行（豁免 ≤1000 需头注写理由），测试文件 ≤800 行；禁止 git worktree / checkout / switch / stash / reset / commit / clean（会丢 WIP）；禁止不带 --check 的 cargo fmt --all，禁止仓库级 dart format；不做没被点名的重构；不确定就停下，写清哪里不确定。
[门禁·Rust 改动必跑] cargo test --workspace --all-targets；cargo test --doc --workspace；cargo fmt --all -- --check；cargo clippy --workspace --all-targets -- -D warnings；cargo run -p xtask -- rust-ratio（≥95%）
[门禁·Flutter 改动] cd shell/flutter && flutter analyze && flutter test
   ★ analyze/test 不是产物：只要有 .dart 改动，必须重建前端，否则界面改动在 /app/ 上不生效——
     flutter build web --release --base-href /app/ --no-web-resources-cdn
     （--no-web-resources-cdn 不可省，缺省会走 gstatic CDN，断网白屏）
   ★ **同理**：只要有 `crates/l2d-wasm-demo/**` 的改动，必须重建渲染面：`cd crates/l2d-wasm-demo && trunk build`，
     并复核 `dist/` 的 mtime **晚于**最后一个被改的 `l2d-wasm-demo/**/*.rs`。
     （2026-09-21 实测踩过：wasm 产物陈旧而无人发现，而原红线只写了 `.dart`。）
[回报] 改动文件清单 / 新增或修改的测试名 / 每条门禁的实际输出数字 / 未决问题。禁止用「应该没问题」「大概通过」代替门禁原始输出。

【任务 W3：表情/动作调试面板——不再把老表情续期（症状①）】
已定位的行为（RESEARCH 报告 §1.1）：dev_tools_section.dart 里 _overlayFace 默认 true（:1346）、
_expressionId 一旦点过就永久粘住（:1337、:1483-1494），_applyGesture()（:1400-1406）先发那条表情再发手势，
于是每点一次手势都把上一次表情的 2.6s ttl 重新计时（手势只有 0.9s）——用户看到「动作总带着那张老脸」。
必做：
1. 「叠加基础表情」默认改 false（_overlayFace = false）。语义不变、开关仍在，只是不再默认接管。
2. 表情状态不再谎报：_expressionId 加一条与渲染面同口径的到点计时（表情 ttl 2600ms / 手势 900-1000ms，
   权威值在渲染面 preset 的 duration_ms）；到点后 UI 文案必须从「当前基础表情：微笑」回到「无（已到点）」，
   并且**行为上**也回到不叠加（即到点后 _expressionId 视为 none，点手势不再重发它）。
   要求：计时用 **可注入的时钟**（不要直接 DateTime.now() 写死在 widget 里），这样 flutter test 能用假时钟钉住。
3. 点手势按钮**不得**隐式延长任何表情的寿命：叠加是「发一条新表情」而不是「把旧的重发一遍」——
   若表情仍在有效期内，允许保持（不重发）；已到点则不发。把这条写进 _applyGesture 的注释。
4. 预设 id 列表**不要**在 Dart 里再抄一份（RESEARCH 报告 §4 E7）：
   settings/preset_labels.dart:22 的 kExpressionPresetIds、dev_tools_section.dart:1273-1287 的
   kDebugExpressionPresets / kDebugGesturePresets 都与「单一真源 = preset_labels.json 的 channel 字段」冲突。
   改为：从 PresetLabelTable 派生（新增一个按 channel 过滤的 getter），取不到表时回落为空并**在 UI 上如实说明**
   （不要静默显示空按钮组）。
5. 测试：shell/flutter/test/developer_section_test.dart 现有那条「叠加基础表情开着时，先发 Face 再发 Gesture」
   （:122-154）保留语义但改成「显式打开开关后」；新增：① 默认不叠加；② 表情到点后 UI 不再声称在演且点手势不重发；
   ③ id 列表来自标签表的 channel（给一张假表断言按钮集合）。
6. （可选，只在改动后文件仍 >1000 行时做）把 dev_tools_section.dart 拆成 debug_panels/ 下的 2-3 个文件；
   若不做，在回报里写「未拆，当前 N 行」并记入未决问题。**不要为了拆而拆**。
禁止：改渲染面 preset 状态机（preset/mod.rs 的分槽撤销是对的，不要动）；改「归零（none）」按钮语义（归 W4）；
     顺手改其它 section。
验收：flutter analyze 无问题 + flutter test 全过；回报里贴出新增测试名与「默认不叠加」那条的断言原文。
【文件归属】shell/flutter/lib/settings/sections/dev_tools_section.dart、shell/flutter/lib/settings/preset_labels.dart、
           shell/flutter/test/developer_section_test.dart、shell/flutter/test/preset_labels_test.dart

================================================================
===== 复制给 Worker W4 =====
================================================================
[工作区] /home/skystar/Live2D-Ai-l1（git worktree，分支 mod/l1-product，约 178 个未提交改动）。不要动 /home/skystar/Live2D-Ai（另一个 worktree，另一个分支）。
[开工前读] 本任务点名文件各自的模块头注；以及 docs/plans/RESEARCH-actions-director-audit-2026-09-21.md 里与本任务相关的章节（那份调研带 file:line 证据与量化表，先读再动手）。
[硬约束] 源码 ≤500 行（豁免 ≤1000 需头注写理由），测试文件 ≤800 行；禁止 git worktree / checkout / switch / stash / reset / commit / clean（会丢 WIP）；禁止不带 --check 的 cargo fmt --all，禁止仓库级 dart format；不做没被点名的重构；不确定就停下，写清哪里不确定。
[门禁·Rust 改动必跑] cargo test --workspace --all-targets；cargo test --doc --workspace；cargo fmt --all -- --check；cargo clippy --workspace --all-targets -- -D warnings；cargo run -p xtask -- rust-ratio（≥95%）
[门禁·Flutter 改动] cd shell/flutter && flutter analyze && flutter test
   ★ analyze/test 不是产物：只要有 .dart 改动，必须重建前端，否则界面改动在 /app/ 上不生效——
     flutter build web --release --base-href /app/ --no-web-resources-cdn
     （--no-web-resources-cdn 不可省，缺省会走 gstatic CDN，断网白屏）
   ★ **同理**：只要有 `crates/l2d-wasm-demo/**` 的改动，必须重建渲染面：`cd crates/l2d-wasm-demo && trunk build`，
     并复核 `dist/` 的 mtime **晚于**最后一个被改的 `l2d-wasm-demo/**/*.rs`。
     （2026-09-21 实测踩过：wasm 产物陈旧而无人发现，而原红线只写了 `.dart`。）
[回报] 改动文件清单 / 新增或修改的测试名 / 每条门禁的实际输出数字 / 未决问题。禁止用「应该没问题」「大概通过」代替门禁原始输出。

【任务 W4：撤销语义——让「本轮没有预设」真的把舞台归零（症状①的后半）】
现状（RESEARCH 报告 §1.4）：
  - live2d_stage.dart:326-352 的 applyPreset('none') **已经**能正确发 Revoke（会清两个槽）——协议与渲染面都就绪；
  - 但 main.dart:483 `if (preset is! String || preset.isEmpty || preset == 'none') return;` 遇到 none/空**直接 return**；
    main.dart:446 对空 cue 也 return；而 director 的 resolve() 对 neutral 返回的是 **null**，不是 'none'。
  - 于是「本轮判定为中性/无动作」既不清上一轮的残留，也没人撤销：只能等 ttl（表情 2.6s）自然到点。
  - 另外 performance-layer-v0.md §2.2 写「noop … 发一份空 action_cue（清上一轮残留）」，**这句与代码不符**。
必做：
1. main.dart 的 _applyDirectorPreset()：在 `if (seq <= _lastDirectorSeq) return;` **之后**（即确认这是**本轮的新决策**），
   把 preset 为 null / 空串 / 'none' 都当作「本轮没有预设」→ 调 applyPreset('none', source: 'director') 显式归零两个槽。
   注意顺序：**先**完成 seq 去重判断，**再**归零；否则每一帧 text_delta 都会重复归零。
2. main.dart 的 _applyDirectorCueForSeq()：保持「空 cue 列表 = 本轮不动」，不要让它承担归零职责
   （cue 是**按句**锚定的，用整份计划去归零会把别的句的表演也清掉）。
3. 修正文档而不是发明新语义：把 docs/architecture/performance-layer-v0.md §2.2 里「发一份空 action_cue（清上一轮残留）」
   改成实情——**空 cues = 本轮不动；撤销的唯一哨兵是 preset_id='none'**；并补一句「noop 轮不清上一轮残留，
   残留由该 preset 自己的 ttl 收敛」。若你决定实现「空计划也归零」，必须给出锚点定义（哪一句、哪个事件）并单独一段说明，
   默认**不做**。
4. 测试：
   - Dart：一条纯逻辑/组件测试证明「同 seq 重复帧只归零一次」「新 seq 且无 preset 一定发 none」；
   - 若能在不回退依赖的前提下做，再加一条「none 之后渲染面两槽都清」的协议级断言。
禁止：改渲染面 preset 状态机；改 action_cue 的帧结构；把「归零」做成周期性的（那会让舞台抖动）。
验收：applyPreset('none') 的调用点与 seq 去重的先后顺序在 diff 里一眼可见；文档与代码口径一致。
【文件归属】shell/flutter/lib/main.dart、shell/flutter/lib/live2d/live2d_stage.dart、
           docs/architecture/performance-layer-v0.md、shell/flutter/test/action_cue_test.dart

================================================================
===== 复制给 Worker W5 =====
================================================================
[工作区] /home/skystar/Live2D-Ai-l1（git worktree，分支 mod/l1-product，约 178 个未提交改动）。不要动 /home/skystar/Live2D-Ai（另一个 worktree，另一个分支）。
[开工前读] 本任务点名文件各自的模块头注；以及 docs/plans/RESEARCH-actions-director-audit-2026-09-21.md 里与本任务相关的章节（那份调研带 file:line 证据与量化表，先读再动手）。
[硬约束] 源码 ≤500 行（豁免 ≤1000 需头注写理由），测试文件 ≤800 行；禁止 git worktree / checkout / switch / stash / reset / commit / clean（会丢 WIP）；禁止不带 --check 的 cargo fmt --all，禁止仓库级 dart format；不做没被点名的重构；不确定就停下，写清哪里不确定。
[门禁·Rust 改动必跑] cargo test --workspace --all-targets；cargo test --doc --workspace；cargo fmt --all -- --check；cargo clippy --workspace --all-targets -- -D warnings；cargo run -p xtask -- rust-ratio（≥95%）
[门禁·Flutter 改动] cd shell/flutter && flutter analyze && flutter test
   ★ analyze/test 不是产物：只要有 .dart 改动，必须重建前端，否则界面改动在 /app/ 上不生效——
     flutter build web --release --base-href /app/ --no-web-resources-cdn
     （--no-web-resources-cdn 不可省，缺省会走 gstatic CDN，断网白屏）
   ★ **同理**：只要有 `crates/l2d-wasm-demo/**` 的改动，必须重建渲染面：`cd crates/l2d-wasm-demo && trunk build`，
     并复核 `dist/` 的 mtime **晚于**最后一个被改的 `l2d-wasm-demo/**/*.rs`。
     （2026-09-21 实测踩过：wasm 产物陈旧而无人发现，而原红线只写了 `.dart`。）
[回报] 改动文件清单 / 新增或修改的测试名 / 每条门禁的实际输出数字 / 未决问题。禁止用「应该没问题」「大概通过」代替门禁原始输出。

【任务 W5：Rust 侧卫生——join_endpoint 收敛 + 注释漂移（症状④）】
必做：
1. **join_endpoint 从 4 份收敛到 1 份**。现状：
   runtime/src/lib.rs:162（pub，正确的那份，但头注自称「base_url 拼接的唯一实现」——不实）；
   runtime/src/performance/client.rs:136（**同一个 crate 内**的私有重复，还被 performance/mod.rs:48 对外 re-export）；
   mod-director/src/staging_http.rs:52；mod-memory/src/summary_http.rs:41（这两个 Mod 各有自己的最小副本）。
   做法：把 performance/client.rs 的副本删掉、改用 crate::join_endpoint（注意返回类型差异：runtime 那份返回
   url::Url + 自定义错误，副本返回 Result<reqwest::Url, String>——**先把错误类型对齐**，再删副本，不要靠 unwrap 蒙过去）；
   两个 Mod 允许保留自己的最小副本（Mod 边界纪律），但必须在各自注释里写明「与 runtime::join_endpoint 同语义，
   因为 Mod 不依赖 runtime 的网络层」，并把「唯一实现」这类话从 runtime/lib.rs 头注里删掉。
   若你判断两个 Mod 也应该依赖 runtime（crate 依赖方向允许的话），可以做，但要在报告里说明代价。
2. **注释漂移**（都是「注释与代码相反」这一类，逐条改）：
   - l2d-wasm-demo/src/web/surface/input.rs:237-242 说「动作 override 层已删除，idle 层之上不再有 final_override 写入方」，
     但**同文件 :200-203 每帧都在写 final_override**（preset 预设）。改成实情。
   - runtime/src/settings.rs:594 的 to_toml_string 文档仍写「下次启动仍由 resolve_with(|n| env::var(n).ok()) 从环境读出」，
     与「密钥真源 = .env、只能走 secrets::lookup」矛盾。改成实情（**只改文档，不要改代码语义**）。
   - 已不存在的 preset.rs：l2d-wasm-demo/src/main.rs:56、preset/table.rs:1（现在是 preset/ 目录）。改成 preset/。
3. 顺手确认（不改也算结论，写进回报）：`grep -rn 'fn join_endpoint' crates/ --include=*.rs` 收敛后应只剩
   runtime/lib.rs 一处 + 两个 Mod 的「跨边界副本」；把实际输出贴出来。
禁止：动 settings.rs 的 DEFAULT_*/MIN/MAX 常量（那是 W2 的独占文件范围——本块在 settings.rs 里**只改 :594 那段文档**，
     若与 W2 并发，等 W2 收口后再改同一个文件）；动 preset/mod.rs；动 Mod 的密钥读取（那是 W6）。
验收：cargo test/clippy/fmt/rust-ratio 全绿；回报里给出收敛前后的 grep 输出对比。
【文件归属】crates/live2d-ai-runtime/src/lib.rs、crates/live2d-ai-runtime/src/performance/{client.rs,mod.rs}、
           crates/l2d-wasm-demo/src/web/surface/input.rs、crates/l2d-wasm-demo/src/main.rs、
           crates/l2d-wasm-demo/src/preset/table.rs、crates/live2d-ai-runtime/src/settings.rs（**仅 :594 文档段**）

================================================================
===== 复制给 Worker W6 =====
================================================================
[工作区] /home/skystar/Live2D-Ai-l1（git worktree，分支 mod/l1-product，约 178 个未提交改动）。不要动 /home/skystar/Live2D-Ai（另一个 worktree，另一个分支）。
[开工前读] 本任务点名文件各自的模块头注；以及 docs/plans/RESEARCH-actions-director-audit-2026-09-21.md 里与本任务相关的章节（那份调研带 file:line 证据与量化表，先读再动手）。
[硬约束] 源码 ≤500 行（豁免 ≤1000 需头注写理由），测试文件 ≤800 行；禁止 git worktree / checkout / switch / stash / reset / commit / clean（会丢 WIP）；禁止不带 --check 的 cargo fmt --all，禁止仓库级 dart format；不做没被点名的重构；不确定就停下，写清哪里不确定。
[门禁·Rust 改动必跑] cargo test --workspace --all-targets；cargo test --doc --workspace；cargo fmt --all -- --check；cargo clippy --workspace --all-targets -- -D warnings；cargo run -p xtask -- rust-ratio（≥95%）
[门禁·Flutter 改动] cd shell/flutter && flutter analyze && flutter test
   ★ analyze/test 不是产物：只要有 .dart 改动，必须重建前端，否则界面改动在 /app/ 上不生效——
     flutter build web --release --base-href /app/ --no-web-resources-cdn
     （--no-web-resources-cdn 不可省，缺省会走 gstatic CDN，断网白屏）
   ★ **同理**：只要有 `crates/l2d-wasm-demo/**` 的改动，必须重建渲染面：`cd crates/l2d-wasm-demo && trunk build`，
     并复核 `dist/` 的 mtime **晚于**最后一个被改的 `l2d-wasm-demo/**/*.rs`。
     （2026-09-21 实测踩过：wasm 产物陈旧而无人发现，而原红线只写了 `.dart`。）
[回报] 改动文件清单 / 新增或修改的测试名 / 每条门禁的实际输出数字 / 未决问题。禁止用「应该没问题」「大概通过」代替门禁原始输出。

【任务 W6：本地令牌也走 secrets::lookup（症状④）】
现状：三处直接 std::env::var 读令牌，绕过 .env 快照，会出现「界面上刚写了 key、链路还说没配置」：
  - crates/live2d-ai-mod-external-input/src/lib.rs:155（EXTERNAL_INPUT_TOKEN）
  - crates/live2d-ai-desktop/src/web_api/external_routes.rs:248（EXTERNAL_INPUT_TOKEN）
  - crates/live2d-ai-desktop/src/web_api/voice_routes.rs:226（VOICE_INPUT_TOKEN）
必做：
1. 三处改为 live2d_ai_runtime::secrets::lookup；语义保持「查不到值 = 不鉴权」（不要改成 401）。
2. 维持既有的令牌优先级链（若原实现是 env -> Mod config -> 不鉴权，改完必须仍是这个顺序，并且在注释里写清）。
3. web_api/cli_entry.rs:272 的 Mod 读取面仍用**弱口径** settings_to_view（has_api_key = 仅「声明了变量名」），
   与 GET/PUT /api/v1/env、以及 GET /api/v1/settings 的强口径不一致。二选一并在报告里说明理由：
   (a) 注入 settings_to_view_with_keys + secrets::lookup，统一口径；或
   (b) 保持弱口径，但在该处写出**显式例外注释**（为什么 Mod 面允许弱口径），并在 settings/view.rs 的两种语义说明里互相引用。
   **推荐 (a)**；选 (b) 必须给出「Mod 面为什么不能读 .env」的实际理由，写不出就选 (a)。
4. 测试：三处各补一条「值只写在 .env 里、进程环境没有」也能读到令牌的回归；
   用**注入的 lookup / 临时 .env 文件**构造，不要用 std::env::set_var（并发测试下不可靠，且本项目已禁止）。
禁止：把令牌明文写进日志/响应/报告；放宽 external-input 的 403 mod_disabled 门禁；动 external_input 的模板逻辑。
验收：`grep -rn 'std::env::var' crates/ --include=*.rs | grep -v test` 的输出里，
      只剩 secrets.rs（.env 路径与回退）、纯环境探测（XDG/DISPLAY/HOME/WAYLAND）、以及 CLI 的 LIVE2D_AI_* 开关；
      三个令牌名不再出现。把这条命令的实际输出贴进回报。
【文件归属】crates/live2d-ai-mod-external-input/src/lib.rs、crates/live2d-ai-desktop/src/web_api/external_routes.rs、
           crates/live2d-ai-desktop/src/web_api/voice_routes.rs、crates/live2d-ai-desktop/src/web_api/cli_entry.rs 及其测试

================================================================
===== 复制给 Worker W7 =====
================================================================
[工作区] /home/skystar/Live2D-Ai-l1（git worktree，分支 mod/l1-product，约 178 个未提交改动）。不要动 /home/skystar/Live2D-Ai（另一个 worktree，另一个分支）。
[开工前读] 本任务点名文件各自的模块头注；以及 docs/plans/RESEARCH-actions-director-audit-2026-09-21.md 里与本任务相关的章节（那份调研带 file:line 证据与量化表，先读再动手）。
[硬约束] 源码 ≤500 行（豁免 ≤1000 需头注写理由），测试文件 ≤800 行；禁止 git worktree / checkout / switch / stash / reset / commit / clean（会丢 WIP）；禁止不带 --check 的 cargo fmt --all，禁止仓库级 dart format；不做没被点名的重构；不确定就停下，写清哪里不确定。
[门禁·Rust 改动必跑] cargo test --workspace --all-targets；cargo test --doc --workspace；cargo fmt --all -- --check；cargo clippy --workspace --all-targets -- -D warnings；cargo run -p xtask -- rust-ratio（≥95%）
[门禁·Flutter 改动] cd shell/flutter && flutter analyze && flutter test
   ★ analyze/test 不是产物：只要有 .dart 改动，必须重建前端，否则界面改动在 /app/ 上不生效——
     flutter build web --release --base-href /app/ --no-web-resources-cdn
     （--no-web-resources-cdn 不可省，缺省会走 gstatic CDN，断网白屏）
   ★ **同理**：只要有 `crates/l2d-wasm-demo/**` 的改动，必须重建渲染面：`cd crates/l2d-wasm-demo && trunk build`，
     并复核 `dist/` 的 mtime **晚于**最后一个被改的 `l2d-wasm-demo/**/*.rs`。
     （2026-09-21 实测踩过：wasm 产物陈旧而无人发现，而原红线只写了 `.dart`。）
[回报] 改动文件清单 / 新增或修改的测试名 / 每条门禁的实际输出数字 / 未决问题。禁止用「应该没问题」「大概通过」代替门禁原始输出。

【任务 W7：动作幅度的下发通道整修（症状②的后半）——必须在 W2 收口后开工】
现状（RESEARCH 报告 §2.3/§2.4）：
  - Live2DStage 的 actionScales 参数（live2d/live2d_stage.dart:96）**从未被传过**：main.dart:709-717 的构造点没有它，
    所以 _attach()（:287-294）与 didUpdateWidget（:205-207）这两条「iframe 重建后补发」的通路永远拿到 null，
    测试目录对 actionScales 零引用。唯一有效通路是 ActionScalesSyncer。
  - 通道有两个互不知情的写者、各自去重：产品/草稿值走 syncer（shell_prefs.dart:38-45 -> action_scales_sync.dart:55-67，
    自己的 _sent 快照）；调试面板的「临时幅度覆盖」直发（shell_settings.dart:254-261），**不更新 _sent**。
  - 确定性缺陷 1：任何 DisplayPrefs 变更都会无条件冲掉临时值——shell_prefs.dart:99-114 _updatePrefs -> _applyPrefs(next) -> :91-92
    _syncActionScalesNow(force: true)，force 绕过去重。即「设临时 head=2.0 -> 应用 -> 换个主题 / 调音量」=> 静默回到产品值。
    这条**不在**面板文案承认的范围内（dev_tools_section.dart:1596-1601 只说「未保存的草稿」会冲掉它）。
  - 确定性缺陷 2：_settings.addListener(_scheduleActionScalesSync)（main.dart:367）在任何一次 edit() 后 150ms 走 syncNow(effective)；
    若 effective 的键恰好等于 _sent 就不发送（临时值意外粘住），不等就发送（临时值被冲掉）——同一操作序列结果依赖历史。
必做（把「谁在什么时候写这个通道」收敛成一条明确规则）：
1. 明确三层优先级并写进注释与文档：**临时覆盖（调试面板，显式、可一键取消） > 草稿（谁在拖谁优先） > 磁盘值**。
   实现建议：把临时覆盖做成 syncer 的一个显式状态（例如 ActionScalesSyncer 增加 pinned/override 字段与 clear 方法），
   让所有下发**只经过 syncer 一个出口**；main.dart 里 _stageKey.currentState?.sync(actionScales: ...) 这条直发路径删掉。
2. Live2DStage.actionScales 二选一，并在报告里说明选了哪个：
   (a) **接上**：main.dart 的构造点传入当前有效倍率，让 _attach()/didUpdateWidget 在 iframe 重建后能自愈；
   (b) **删掉**：连同 :96、:205-207、:292 一起删，并在 Live2DStage 头注里写明「幅度的唯一通路是 applyActionScales（host 决定）」。
   推荐 (a)——(b) 会让「iframe 重建后幅度丢失」成为一个只能靠 onReady 兜住的隐式依赖。
3. `_applyPrefs` 里那条 force 补发必须**尊重临时覆盖**：临时覆盖生效时，_applyPrefs 不得把产品值写下去。
4. 面板文案（dev_tools_section.dart:1596-1601）改成与新规则一致的实情：写清「临时值在什么情况下会被谁清掉」。
5. 测试（**新建文件**，不要改 W2 的测试文件）：shell/flutter/test/action_scales_wiring_test.dart
   - 设临时值 -> 触发一次 DisplayPrefs 变更 -> 临时值仍在（无 force 冲掉）；
   - 点「恢复产品设置」-> 产品值一定写下去（不受 _sent 去重挡住）；
   - iframe 重建（重挂）-> 幅度不会退回渲染面出厂默认（对应第 2 条选的方案）；
   - 值没变时不重复下发；拖动只发一帧（现 action_scales_sync.dart 已有回归则复用）。
禁止：改渲染面 scales.rs 的公式与限值（那是 W2）；改 ActionSettingsView 的默认值（那是 W2）；
     把临时覆盖落盘（它按定义只对本会话有效）。
验收：flutter analyze/test 全过 + 重建产物；回报里贴出四个新测试名与「改主题后临时值仍在」的断言原文。
【文件归属】shell/flutter/lib/main.dart、shell/flutter/lib/live2d/live2d_stage.dart、shell/flutter/lib/app/shell_prefs.dart、
           shell/flutter/lib/live2d/action_scales_sync.dart、shell/flutter/lib/app/shell_settings.dart、
           shell/flutter/lib/settings/sections/dev_tools_section.dart（**仅 :1596-1601 文案段**）、
           shell/flutter/test/action_scales_wiring_test.dart（新建）

================================================================
===== 复制给 Worker W8 =====
================================================================
[工作区] /home/skystar/Live2D-Ai-l1（git worktree，分支 mod/l1-product，约 178 个未提交改动）。不要动 /home/skystar/Live2D-Ai（另一个 worktree，另一个分支）。
[开工前读] 本任务点名文件各自的模块头注；以及 docs/plans/RESEARCH-actions-director-audit-2026-09-21.md 的 §3.7 与 §3.8（产品形态；先读再动手）。
[硬约束] 源码 ≤500 行（豁免 ≤1000 需头注写理由），测试文件 ≤800 行；禁止 git worktree / checkout / switch / stash / reset / commit / clean（会丢 WIP）；禁止不带 --check 的 cargo fmt --all，禁止仓库级 dart format；不做没被点名的重构；不确定就停下，写清哪里不确定。
[门禁·Rust 改动必跑] cargo test --workspace --all-targets；cargo test --doc --workspace；cargo fmt --all -- --check；cargo clippy --workspace --all-targets -- -D warnings；cargo run -p xtask -- rust-ratio（≥95%）
[门禁·Flutter 改动] cd shell/flutter && flutter analyze && flutter test
   ★ analyze/test 不是产物：只要有 .dart 改动，必须重建前端，否则界面改动在 /app/ 上不生效——
     flutter build web --release --base-href /app/ --no-web-resources-cdn
     （--no-web-resources-cdn 不可省，缺省会走 gstatic CDN，断网白屏）
   ★ **同理**：只要有 `crates/l2d-wasm-demo/**` 的改动，必须重建渲染面：`cd crates/l2d-wasm-demo && trunk build`，
     并复核 `dist/` 的 mtime **晚于**最后一个被改的 `l2d-wasm-demo/**/*.rs`。
     （2026-09-21 实测踩过：wasm 产物陈旧而无人发现，而原红线只写了 `.dart`。）
[回报] 改动文件清单 / 新增或修改的测试名 / 每条门禁的实际输出数字 / 未决问题。禁止用「应该没问题」「大概通过」代替门禁原始输出。

【任务 W8：导演口径与文档漂移治理（**只改文档与资产注释，不改行为**）】
背景（维护者 2026-09-21，**已确认**；规格见 RESEARCH 报告 §3.7 / §3.8 / §8 / §9 / §10；本块只做文档治理，目标契约由 W10/W11 实现）：
  产品形态 = 「**酒馆（类酒馆角色扮演内核）+ Live2D 皮套壳子**」：人设由酒馆式内核稳定住，
  **各功能由现有 mod 矩阵承担**（记忆 / 外部输入 / 语音 / 人设 / 导演都在矩阵里），Live2D 皮套负责情绪表达；
  **导演是一个 AI**（不是给用户操控皮套的辅助）。
  ⇒ **架构不改造**。要治理的是**文档漂移**：多处仍把导演写成「遗留并行实现 / 不建议开启 / 零投递旁路 / 不驱动动作」，
     既与代码事实不符（它会产出 latest.preset_id 与 action_cue），也与产品定位不符。
必做（只改文档与资产注释）：
1. docs/architecture/director-rfc.md：在文件头加一节「**现行状态与已过时红线（2026-09-21）**」，逐条标注**已被产品演进推翻**的红线——
   「主链不新增第二 LLM」「端点权威只有 [tts]/[llm]」「骨架不投递任何动作/TTS 参数」——并写明
   「这些是 2026-09-14 骨架期结论；产品已演进为『酒馆 + 皮套壳子』，**以代码 + 本节为准**」。语气是**标注过时**，不是删历史。
   另：把 §2.1 TextDelta「回复侧**补充**证据」标注为**不采用**（情绪判定的输入 = 用户输入：情绪的起因是用户说的话）。
2. docs/architecture/director-mod-v0.md：
   - 清掉「遗留并行实现」「不再是主路由」「零投递」「不驱动动作」这类与事实不符的定性；
   - 顶部写清**实际职责**：按情绪/意图选动作包 → 产出 latest.preset_id（状态面，供面板展示）与按句 action_cue（驱动舞台）；
   - §0 的「host 下行通道仍然零调用」改成事实描述：action_tx / apply_settings 零调用**成立**，
     但**下行确实存在**——走 ModServices.cues → WS action_cue。
3. docs/architecture/performance-layer-v0.md：把「director staging 是遗留并行实现」改写成事实：
   「主链 [performance] 与 Mod staging_* 是**两个可选提供者，都默认关，职责重叠**；
     谁的 speak 能力该保留**未定**（RESEARCH §3.7 Q1）」；§7 的「谁主谁退」表改成「**当前实现事实**」。
4. assets/actions/preset_labels.json：_doc 与 sources.neko 各加一句
   「canonical 字段只用于展示归并，**不表示输入语义与 N.E.K.O 相同**：本项目判定**用户输入**，N.E.K.O 判定**角色回复**」。
5. assets/actions/presets.json **不要动**（归 W2）；AGENTS.md **不要动**（归 W1）——把你希望 W1 写进去的产品口径原文列进回报，由编排者转给 W1 校对。
禁止：改任何 .rs / .dart；**裁决**「导演是否产出 speak」（Q1 未定，只写事实 + 标注未定）；删 [performance] 段；改 preset 表值。
验收：三份文档 + 一个 JSON 注释口径一致；**不再有任何一处**把导演写成「辅助用户操控皮套 / 遗留 / 不建议开启 / 零投递」；
      回报里贴 grep -rn 的实际输出（检索「遗留 / 不建议 / 辅助用户 / 零投递」于 docs/architecture/），逐条说明「保留（历史记录）」还是「已改写」。
【文件归属】docs/architecture/{director-rfc.md,director-mod-v0.md,performance-layer-v0.md}、assets/actions/preset_labels.json
（⚠ 与 W4 共用 performance-layer-v0.md ⇒ 本块必须在 Wave 0 收口，W4 才能开工）

================================================================
===== 复制给 Worker W9 =====
================================================================
[工作区] /home/skystar/Live2D-Ai-l1（git worktree，分支 mod/l1-product，约 178 个未提交改动）。不要动 /home/skystar/Live2D-Ai（另一个 worktree，另一个分支）。
[开工前读] 本任务点名文件各自的模块头注；以及 docs/plans/RESEARCH-actions-director-audit-2026-09-21.md 的 §3.7 与 §3.8（产品形态；先读再动手）。
[硬约束] 源码 ≤500 行（豁免 ≤1000 需头注写理由），测试文件 ≤800 行；禁止 git worktree / checkout / switch / stash / reset / commit / clean（会丢 WIP）；禁止不带 --check 的 cargo fmt --all，禁止仓库级 dart format；不做没被点名的重构；不确定就停下，写清哪里不确定。
[门禁·Rust 改动必跑] cargo test --workspace --all-targets；cargo test --doc --workspace；cargo fmt --all -- --check；cargo clippy --workspace --all-targets -- -D warnings；cargo run -p xtask -- rust-ratio（≥95%）
[门禁·Flutter 改动] cd shell/flutter && flutter analyze && flutter test
   ★ analyze/test 不是产物：只要有 .dart 改动，必须重建前端，否则界面改动在 /app/ 上不生效——
     flutter build web --release --base-href /app/ --no-web-resources-cdn
     （--no-web-resources-cdn 不可省，缺省会走 gstatic CDN，断网白屏）
   ★ **同理**：只要有 `crates/l2d-wasm-demo/**` 的改动，必须重建渲染面：`cd crates/l2d-wasm-demo && trunk build`，
     并复核 `dist/` 的 mtime **晚于**最后一个被改的 `l2d-wasm-demo/**/*.rs`。
     （2026-09-21 实测踩过：wasm 产物陈旧而无人发现，而原红线只写了 `.dart`。）
[回报] 改动文件清单 / 新增或修改的测试名 / 每条门禁的实际输出数字 / 未决问题。禁止用「应该没问题」「大概通过」代替门禁原始输出。

【任务 W9：单一驱动者——退役「拉 latest.preset_id」那条通道，统一到 action_cue】
（**注**：这条通道退役后，W10/W11 才能把动作协议换成字段化形状；W9 是它们的前置，不是可选优化。）
背景（RESEARCH 报告 §3.5 / D5）：前端现在有**两条**未协调的驱动通道，都调 applyPreset：
  A) 按句 action_cue：main.dart:421-425（存计划）+ 443-456（该句音频开始时应用）——**这是有契约的正路**；
  B) 一次性拉 GET /api/v1/mods/director/state 的 latest.preset_id：main.dart:417-419（首个 SentenceVoiced / TextFallback 时触发）+ 466-492。
  两条**取到的集合不同**（B 走 presets.resolve() 只给一条；A 走 resolve_slots() 给手势 + 表情），同一轮会各驱动一遍；
  而文档（performance-layer-v0.md §7、HANDOFF-2026-09-22 §6、app_event.rs:233）都声称「同轮只有一个 cue 产者」——与代码不符。
必做：
1. **退役通道 B**：删掉 _applyDirectorPreset()、_directorTurnHandled、_lastDirectorSeq、_directorFetching 以及那次 state('director') 拉取；
   舞台的动作预设**只**由 action_cue 驱动。删完必须确认：**director 启用时仍然有动作**
   （它的 cue 走 SentenceReady → ModServices.cues → WS action_cue；这条链路不许被你一起删掉）。
2. latest / recent_decisions 仍作为**面板只读展示**（director_panel.dart 照旧读 state），只是不再驱动舞台——把这条写进 director-mod-v0.md 对应段落与 main.dart 的注释。
3. 断言（可测，缺一条即未完成）：
   - Dart 侧：新增一条纯逻辑/组件测试证明「收到 action_cue 帧 → 按 sentence_seq 应用一次」，
     并证明**没有任何代码路径**会 state('director') + applyPreset（grep 级断言或注入 spy 均可，写清算哪种）；
   - 保留并跑通既有 shell/flutter/test/action_cue_test.dart；
   - 文字断言：grep -rn 'mods/director/state' shell/flutter/lib/ 只剩面板读取处（把实际输出贴进回报）。
4. 修掉与实情不符的注释与文档句（crates/live2d-ai-desktop/src/app_event.rs:233、performance-layer-v0.md §7、
   docs/plans/HANDOFF-2026-09-22-performance-layer.md §6）。
禁止：改 action_cue 帧结构；删面板；让两条通道都活跃；裁决「导演是否产出 speak」（Q1 未定，不归本块）。
验收：回报里给出两种配置下**谁**驱动表情/手势的实测——「director 启用 + [performance] 关」与「[performance] 开」各一次，
      证据 = 起服务后的渲染面 HUD preset: 行 + WS 帧序列（不要只用单测替代）。
【文件归属】shell/flutter/lib/main.dart、crates/live2d-ai-desktop/src/app_event.rs（注释）、
           docs/architecture/{performance-layer-v0.md,director-mod-v0.md}、docs/plans/HANDOFF-2026-09-22-performance-layer.md、
           shell/flutter/test/action_cue_test.dart（或新建 driver_policy_test.dart）
================================================================
===== 复制给 Worker W10 =====
================================================================
[工作区] /home/skystar/Live2D-Ai-l1（git worktree，分支 mod/l1-product，约 178 个未提交改动）。不要动 /home/skystar/Live2D-Ai（另一个 worktree，另一个分支）。
[开工前读] docs/plans/RESEARCH-actions-director-audit-2026-09-21.md 的 §8（目标链路）/ §9（时间轴对齐）/ §10（输出契约与字段分解）——这三节是维护者已确认的规格，先读再动手。
[硬约束] 源码 ≤500 行（豁免 ≤1000 需头注写理由），测试文件 ≤800 行；禁止 git worktree / checkout / switch / stash / reset / commit / clean（会丢 WIP）；禁止不带 --check 的 cargo fmt --all，禁止仓库级 dart format；不做没被点名的重构；不确定就停下，写清哪里不确定。
[门禁·Rust 改动必跑] cargo test --workspace --all-targets；cargo test --doc --workspace；cargo fmt --all -- --check；cargo clippy --workspace --all-targets -- -D warnings；cargo run -p xtask -- rust-ratio（≥95%）
[门禁·Flutter 改动] cd shell/flutter && flutter analyze && flutter test
   ★ analyze/test 不是产物：只要有 .dart 改动，必须重建前端，否则界面改动在 /app/ 上不生效——
     flutter build web --release --base-prefix /app/ --no-web-resources-cdn 的**正确形式**是：
     flutter build web --release --base-href /app/ --no-web-resources-cdn
     （--no-web-resources-cdn 不可省，缺省会走 gstatic CDN，断网白屏）
   ★ **同理**：只要有 `crates/l2d-wasm-demo/**` 的改动，必须重建渲染面：`cd crates/l2d-wasm-demo && trunk build`，
     并复核 `dist/` 的 mtime **晚于**最后一个被改的 `l2d-wasm-demo/**/*.rs`。
     （2026-09-21 实测踩过：wasm 产物陈旧而无人发现，而原红线只写了 `.dart`。）
[回报] 改动文件清单 / 新增或修改的测试名 / 每条门禁的实际输出数字 / 未决问题。禁止用「应该没问题」「大概通过」代替门禁原始输出。

【任务 W10：动作协议字段化（body / head / expression）+ 叠加 + hold + 事件回执 + 音频时钟基准】
背景（维护者 2026-09-21 已确认，RESEARCH §8/§9/§10）：
  · 导演的输出 = 3 个语义字段：body（半身）/ head（头部）/ expression（表情），**同类可重复出现、按 add 合成**；
    每皮套的“基础表演强度”**复用现有 [action] 三个倍率**（body ↔ 字段1、head ↔ 字段2、expression ↔ 字段3），**不要新增第四套倍率**；
  · hold **可加**：带 hold 的条目保持到下次指令，不参与 ttl 到点撤销；
  · 立即生效（不为等音频而延迟）；
  · 嘴型由 TTS 音频驱动，而动作现在用 `performance.now()` 墙钟 ⇒ **两套时钟必须统一到音频时钟**；
  · 导演要“上一个动作字段 / 动作做完没有”，**必须由渲染面发事件**（前端 `presetStatus` 是按 2600/900ms 猜的，不许再当依据）。
必做：
1. **协议 v2（渲染面 `preset` 帧，向后兼容 v1）**：新增字段化形状
   `{ "cues": [ {field, x?, y?, z?, id?, intensity, at, hold} ], "clear": bool }`；
   旧的 `{id:"smile"}` 形状**必须继续可用**（debug 面板与 W3 依赖它，现有 preset 回归一条都不许红）。
   · `field` ∈ {body, head, expression}；body → `ParamBodyAngleX/Y`（Z 可选）；head → `ParamAngleX/Y/Z`；
     expression → 表情表里的五官通道（`ParamEyeLOpen/ROpen`、`ParamEyeLSmile/RSmile`、`ParamBrowLY/RY`、
     `ParamMouthForm`、`Param4`、`Param6`）；**`ParamMouthOpenY` 永远排除**（口型归 TTS）。
   · 轴值域 `[-1,1]`，多轴同时给 = 按比例合成。
   · **同类多条相加**后按通道上限钳位（新增 additive 语义；现有 `override_parameter` 是 set，不要直接复用它做叠加）。
   · `hold=true` 的条目登记为“保持态”，`clear` 或下一次带同字段的指令才替换；ttl 撤销只作用于非 hold 条目。
2. **每皮套强度**：把字段最终值 = 轴值 × intensity 档位系数 × `sync.actionScales` 对应项（body/head/expression），再钳上限。
   倍率口径**复用 W2/W7 的结论**（不要另立一套）。
3. **音频时钟**：新增 `stage-clock` 消息 `{seg, pos_ms, playing}`（前端每 30ms 或每帧发）；
   渲染面把 `apply_frame` 的 `now_ms` 从 `performance.now()` 换成它（**未收到时回落到墙钟**，保证旧前端仍可用）。
4. **事件回执（渲染面 → 父页；事件级，不是每帧状态流）**：
   `preset-applied{fields, intensity, clamped, dropped}` / `preset-replaced` / `preset-expired{field}` / `preset-cleared`；
   其中 `dropped` = 皮套缺该参数（`override_parameter` 返回 false）——**这是“点了没反应”的唯一可信证据**，非空必须上报。
5. **前端桥接**：`live2d_bridge.dart` / `live2d_stage.dart` 增加发送字段化形状与接收 ack 的方法；
   `api/ws_frame.dart` **不动**（WS 契约本轮不变）。
6. **文档**：`docs/architecture/action-packs-v0.md` 增一节「字段化协议 v2」（与代码逐字一致）；
   新建 `docs/architecture/performance-fields-v0.md`：三字段语义 / 每皮套强度 / hold / 叠加规则 / 事件表 / 音频时钟。
7. 测试（缺一条即未完成）：
   · Rust：字段化解析、**同类相加**、hold 不参与 ttl 撤销、缺参数 → `dropped` 非空、`clear` 清全部（含 hold）；
   · Rust：旧 `{id}` 形状仍工作（现有 `preset` 回归全绿）；
   · Dart：桥接发送与 ack 解析各一条。
禁止：改 WS `action_cue` 帧结构（那是导演→前端的计划帧，归 W11）；动口型通道；
     开非标准参数白名单（维护者已明确**本轮不做**翅膀/耳朵/光环等）。
验收：起服务后在 Developer 面板发一个含 body+head+expression 三条同类叠加的帧 → HUD `preset:` 行能读出三字段与强度；
      ack 事件在前端日志可见；构造一个皮套缺参数的字段 → `dropped` 非空。三条都要原始输出。
【文件归属】crates/l2d-wasm-demo/src/{main.rs,preset/*,web/surface/input.rs,web/surface/render.rs}、
           shell/flutter/lib/live2d/{live2d_bridge.dart,live2d_stage.dart}、
           docs/architecture/{action-packs-v0.md,performance-fields-v0.md（新建）}、相关测试文件
================================================================
===== 复制给 Worker W11 =====
================================================================
[工作区] /home/skystar/Live2D-Ai-l1（git worktree，分支 mod/l1-product，约 178 个未提交改动）。不要动 /home/skystar/Live2D-Ai（另一个 worktree，另一个分支）。
[开工前读] docs/plans/RESEARCH-actions-director-audit-2026-09-21.md 的 §8（目标链路）/ §9（时间轴对齐）/ §10（输出契约与字段分解）——这三节是维护者已确认的规格，先读再动手。
[硬约束] 源码 ≤500 行（豁免 ≤1000 需头注写理由），测试文件 ≤800 行；禁止 git worktree / checkout / switch / stash / reset / commit / clean（会丢 WIP）；禁止不带 --check 的 cargo fmt --all，禁止仓库级 dart format；不做没被点名的重构；不确定就停下，写清哪里不确定。
[门禁·Rust 改动必跑] cargo test --workspace --all-targets；cargo test --doc --workspace；cargo fmt --all -- --check；cargo clippy --workspace --all-targets -- -D warnings；cargo run -p xtask -- rust-ratio（≥95%）
[门禁·Flutter 改动] cd shell/flutter && flutter analyze && flutter test
   ★ analyze/test 不是产物：只要有 .dart 改动，必须重建前端，否则界面改动在 /app/ 上不生效——
     flutter build web --release --base-prefix /app/ --no-web-resources-cdn 的**正确形式**是：
     flutter build web --release --base-href /app/ --no-web-resources-cdn
     （--no-web-resources-cdn 不可省，缺省会走 gstatic CDN，断网白屏）
   ★ **同理**：只要有 `crates/l2d-wasm-demo/**` 的改动，必须重建渲染面：`cd crates/l2d-wasm-demo && trunk build`，
     并复核 `dist/` 的 mtime **晚于**最后一个被改的 `l2d-wasm-demo/**/*.rs`。
     （2026-09-21 实测踩过：wasm 产物陈旧而无人发现，而原红线只写了 `.dart`。）
[回报] 改动文件清单 / 新增或修改的测试名 / 每条门禁的实际输出数字 / 未决问题。禁止用「应该没问题」「大概通过」代替门禁原始输出。

【任务 W11：导演 AI 侧——输出 schema、手册同源生成、段切分、两条件唤醒、会话 baseline】
背景（维护者 2026-09-21 已确认，RESEARCH §8/§9/§10；**必须在 W10 收口后开工**）：
  · 导演 = **第二个 LLM**（独立端点；**不与主模型共用上下文**，这是红线）；
  · 输出 = **段切分（只切不改字）** + 三字段 cues（body/head/expression，同类可重复相加，可 hold）；
  · 唤醒条件**两个同时满足**：① 该段 TTS 播放完成 ② 动作做完（非 hold 动作的 ttl 到点 ack）；
    边界：若该段只下了 hold 动作 ⇒ 没有“动作做完”这个点，唤醒退化为只看 TTS 段播完；
  · 导演输入 = **日志文本**（主模型回复 / 用户原消息 / 已上屏到哪 / 上一个动作字段 / 动作事件时间点）；
  · 断开（停止键 / 新用户消息）= 清「动作 + 表情 + TTS 待播」+ 退回**该会话**的角色基础状态值。
必做：
1. **schema 落到代码**：`{segments: [string], cues: [{field, x?, y?, z?, id?, intensity, at, hold}]}`；
   校验规则：整份失败即回退（不做局部抢救）；`field` 不在词表 → 整份失败；`at` ∈ {now, seg:N, after_prev}；
   `intensity` 钳到 1..3；**`segments` 拼接后必须逐字等于主模型原文**（这条是“只切不改字”的护栏，必须是硬断言）。
2. **使用手册（导演的基础提示词）从 schema 与字段表生成，不许手抄**：
   字段词表、轴含义、intensity 三档（1 轻微 / 2 中 / 3 强）、hold 语义、`at` 锚点、可用表情 id（来自 `assets/actions/`）
   都定义在**一处**，提示词与校验器**同时引用它**（本仓 `preset_labels.json` 就是这条纪律的先例）。
3. **两条件唤醒**：前端在「该段音频 `ended`」+「动作做完 ack」**都到**时组一条**段完成事件**；
   host 把它连同日志文本喂给导演。通道优先**复用现有**（`POST /api/v1/mods/director/command` 已有 command 通道；
   或现有 Mod 事件主题）；**若必须新增 WS 帧或改动 `crates/live2d-ai-mod-system/src/topics.rs` → 先停下问维护者**（基座独占文件）。
4. **会话 baseline（角色基础状态值绑定会话）**：新增 per-session 的 baseline（body/head/expression 三字段 + 强度）；
   断开时下发“clear + 退回 baseline”。本仓已有 per-session 作用域（`session_scope.rs`、`session_scopes.prompt_for(...)`），
   baseline 作为它的兄弟字段，**不要新造一套作用域机制**。
5. **日志喂导演**：把「已上屏进度 / 上一个动作字段及其强度 / 动作事件时间点 / 用户原消息 / 主模型回复」
   整理成**结构化文本**（不是塞原始 JSON 洪水）；脱敏：不含密钥、不塞完整历史。
6. 测试（缺一条即未完成）：
   · `segments` 拼接 === 原文（含换行 / 标点 / emoji 的样本）；
   · `field` 越界或 `at` 非法 → **整份失败并回退**；
   · 两条件唤醒：只到 TTS 播完 → **不唤醒**；只到动作 ack → **不唤醒**；两者都到 → 唤醒一次；
   · hold-only 段：只看 TTS 播完即唤醒；
   · 断开（停止 / 新消息）→ 清三字段 + 回该会话 baseline；换会话 → baseline 跟着换；
   · 日志文本不含密钥。
禁止：把导演与主模型塞进同一个上下文；让导演**改写**文本（只能切分）；改 `topics.rs`；动 WS `action_cue` 帧结构。
验收：端到端 mock 导演跑一遍“好好想想”场景，原始输出需包含：
      段1「嗯……」+ expression=thinking(hold) → （段1 播完 + 动作 ack）→ 唤醒 → 段2「我想到了。」+ expression=surprised →
      停止 → 三字段归零 + 回该会话 baseline。
【文件归属】crates/live2d-ai-runtime/src/performance/*、crates/live2d-ai-mod-director/src/*、
           crates/live2d-ai-desktop/src/{session_scope.rs,supervisor.rs,supervisor/*,web_api/mods_routes.rs 必要时}、
           docs/architecture/performance-fields-v0.md（与 W10 共用 ⇒ 必须等 W10 收口）、assets/actions/*（只读，除非需要新增表情 id 并在报告里说明）
================================================================
===== 裁决点（已全部裁定，无需再问维护者） =====
================================================================

★ 产品形态（权威）：**「酒馆（类酒馆角色扮演内核）+ Live2D 皮套壳子」**——人设由内核稳定，各功能由 **mod 矩阵**承担，
  Live2D 皮套做情绪表达；**导演是一个 AI**（不是用户操控辅助、不是普通场景 Mod）。**不做架构搬迁。**

★ 已裁定、不得翻案的清单：
  · 情绪/表演决策的**输入 = 用户输入**（不是角色回复）；
  · ~~「导演属场景 Mod / 主链不该有第二 LLM」~~ **是我的推论、已被维护者否定**，不得再引用；
  · 过滤层**不允许改写**原文、**允许断句**（只切不改字：`segments` 拼接必须**逐字等于**原文）；
  · 三字段（body / head / expression）+ 每皮套强度**复用现有 `[action]` 三倍率**（body↔字段1 / head↔字段2 / expression↔字段3），
    **不新增第四套倍率**；非标准参数（翅膀/耳朵/光环…）本轮**不做**；
  · **hold 可加**；输出**立即生效**、不排队、**不需要动作队列**；动作的发生本身作为喂导演的输入段点；
  · 唤醒 = ① TTS 段播完 ② 动作做完，**两者同时满足**（hold-only 段退化为只看 TTS 段播完）；
  · 断开（停止键 / 新用户消息）= 清「动作 + 表情 + TTS 待播」+ 退回**该会话**的角色基础状态值（baseline 绑会话）；
  · S1 = **是**（表情字段只写五官，头/身交给字段 1/2）；S2 = **是**（`at` 取 `now` / `seg:N` / `after_prev`）；
  · 时间轴：嘴型由 TTS 驱动 ⇒ 动作必须统一到**音频时钟**（W10 的 `stage-clock`）。

★ 唯一需要维护者过目的东西：**裁决点 A**（W2 的 `MAX_ACTION_SCALE` 取舍）——W2 按块内推荐路径实施，
  并在交付报告里**附备选方案的数值表**，让维护者一眼比较即可，**不需要等答复再开工**。
