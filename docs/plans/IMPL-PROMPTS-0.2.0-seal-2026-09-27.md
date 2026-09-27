# 可复制实现提示词：0.2.0 封口四阶段（rc.5 → rc.6 → rc.7 → 0.2.0 末版）

用法：一个 worker 一块，按**阶段与波次顺序**整块粘贴（每块已含公共前置）。
规划真源：docs/plans/PLAN-0.2.0-seal-and-cleanup-2026-09-27.md（每条任务的背景、file:line 证据、分级债都在里面，
worker 开工前**必须**读自己那一节）。调度真源：docs/plans/ORCHESTRATOR-PROMPT-0.2.0-seal-2026-09-27.md。
审计原始复现：shell/flutter/test/zz_audit_tmp_test.dart（15 条，全部通过 = 缺陷可稳定复现）。

---

## 全局前置（每块都已内嵌，无需另贴）

~~~~
[唯一红线 · 资产] 本轮**必须守护**「导演层接线的 Live2D 表演 + TTS 过滤」（规划 §3.1 的 A1–A8）。
  1) 不许删 WS 帧、不许改既有帧字段名（V11 只增不改）：action_cue / preset_id / speak 一律不删；
  2) 不许把 clean_for_tts 旁路：上屏 == 送 TTS == clean_for_tts(段)；
  3) 不许让前端镜像每帧状态流（高频信号仍走 GlobalKey → Bridge → postMessage）；
  4) 不许动 mod_count_is_five 与五个 Mod 的 id 集合断言；wallpaper / pet-desktop 保持封存；
  5) 表演层「说话权归属」是**未裁决项**（V12/Q1）——**不得自行裁决**，遇到就停下写清问题；
  6) 重放前端重设计时，asset 接触点（main.dart / live2d_stage.dart / live2d_bridge.dart /
     tts_section.dart / turn_liveness.dart / ui_state_tracker.dart）必须**逐个保留语义**。

[硬约束] 源码 ≤500 行（豁免 ≤1000 需头注写理由），测试文件 ≤800 行；
  禁止 git worktree / checkout / switch / stash / reset / commit / clean（会丢 WIP；git 操作只由编排者做）；
  禁止不带 --check 的 cargo fmt --all，禁止仓库级 dart format；不做没被点名的重构；不确定就停下并写清。

[门禁·Rust 改动必跑] cargo test --workspace --all-targets；cargo test --doc --workspace；
  cargo fmt --all -- --check；cargo clippy --workspace --all-targets -- -D warnings；
  cargo run -p xtask -- rust-ratio（≥95%）

[门禁·Flutter 改动] cd shell/flutter && flutter analyze && flutter test
  ★ analyze/test 不是产物：只要有 .dart 改动，必须重建前端，否则界面改动在 /app/ 上不生效——
    flutter build web --release --base-href /app/ --no-web-resources-cdn
    （--no-web-resources-cdn 不可省，缺省会走 gstatic CDN，断网白屏）
  ★ 只要有 crates/l2d-wasm-demo/** 改动，必须 cd crates/l2d-wasm-demo && trunk build，
    并复核 dist/ 的 mtime 晚于最后一个被改的 *.rs。

[回报] 改动文件清单 / 新增或修改的测试名 / 每条门禁的**实际输出数字** / 未决问题。
  禁止用「应该没问题」「大概通过」代替门禁原始输出。
~~~~

---

# Stage A · 0.2.0-rc.5：资产守护 + 重设计并入 + 背景 P0

> 阶段目标：把 09-27 的前端重设计**并入 0.2.0 线**，且**一行资产都不许断**。
> **重要**：本阶段的 A2（重放）**不可并发**——29 个冲突文件就是重设计与 main 的全部交界面，多人抢必炸。
> A1 必须先于 A2；A3* 必须晚于 A2。

## A1（先行，串行）· 建线 + 资产守护网

~~~~
[工作区] 新 worktree（由编排者创建）：/home/skystar/Live2D-Ai-fe，分支 feat/frontend-redesign，基于 **main**。
  源快照 worktree：/home/skystar/Live2D-Ai（分支 mod/persona-polish，09-14 旧基线，**只读，不要改**）。
[开工前读] 规划 §3 全文（资产清单 A1–A8 + 守护红线 + 冲突热点）。

【任务 A1：资产守护网（先架网，再重放）】
背景：重设计有 112 个 Flutter 文件，其中 29 个与 main 冲突，包含 main.dart（_applyDirectorCueForSeq）、
live2d/live2d_stage.dart（applyPreset）、settings/sections/tts_section.dart 等**资产接线点**。
若直接覆盖，导演 cue / 动作包下发会被静默打断，而单测很可能照样全绿。
必做（**只加不改**，不要动业务代码）：
1. 产出 docs/architecture/frontend-asset-inventory.md：
   逐条列 A1–A8 的**前端接触点**（file:line）+ 现有测试名 + 「断了会怎样」。
   至少覆盖：main.dart 的 _applyDirectorCueForSeq、live2d_stage.dart 的 applyPreset/set_scales、
   live2d_bridge.dart 的协议帧与 stage-bg、tts_section.dart 的 TTS 配置项、
   turn_liveness.dart 的回合状态、ui_state_tracker.dart 的 error/ack 消费。
2. 为每个接触点**补一条会失败的守护断言**（新测试文件，例：
   shell/flutter/test/asset_guard_director_cue_test.dart、asset_guard_preset_dispatch_test.dart）：
   * main.dart 必须仍存在 _applyDirectorCueForSeq 的**调用点**（不是只定义）；
   * live2d_stage.dart 必须仍能发出 preset 帧（用既有 fake/mock 桥接路径）；
   * 断言要**行为化**，不要写 src.contains(...) 那种源码字符串扫描（规划 §4 P2 已点名这种假测试）。
3. 自查：把断言临时对着**未重放**的 main 跑一遍，必须全绿（证明它守的是现状，不是空中楼阁）。
[文件归属] 新增 docs/architecture/frontend-asset-inventory.md + 新增 shell/flutter/test/asset_guard_*_test.dart。
  **不许改任何既有业务文件**。
[回报] 清单里每个接触点的 file:line / 每条守护断言的测试名 / 对着 main 跑的实际通过数。
~~~~

## A2（串行，独占，最关键）· 重放重设计到 0.2.0 线

~~~~
[工作区] /home/skystar/Live2D-Ai-fe（feat/frontend-redesign，基于 main）。源：/home/skystar/Live2D-Ai（只读）。
[输入] 编排者已导出：/tmp/redesign-tracked.patch（已跟踪改动）+ /tmp/redesign-untracked/（6 个新增
  lib 文件/目录 + 新增测试 + 未跟踪文档）。若缺失，**停下要**，不要自己从零重写。
[开工前读] 规划 §2（拓扑与差距）+ §3.3（冲突热点）+ A1 产出的 frontend-asset-inventory.md。

【任务 A2：重放 + 解冲突 + 资产复核】
必做：
1. 在 /home/skystar/Live2D-Ai-fe 上用 patch/复制重放全部 112 个 Flutter 文件改动 + 新增文件。
2. 逐个解 **29 个重叠文件**的冲突。解冲突的判据顺序：
   ① 资产语义优先（A1 的守护断言必须全绿）；
   ② main 的新能力（导演可观测、每皮套幅度、表演协议 v1 前端接线）**一律保留**；
   ③ 重设计的显示层改动（背景库 / 材质 / tokens / 排版）照搬；
   ④ 两者对同一行的不同意图无法兼容时 → **停下记录**，不要自己发明第三种语义。
3. 重点逐个复核（对照 frontend-asset-inventory.md）：
   * lib/main.dart —— 导演 cue 接线（_applyDirectorCueForSeq）在不在、调用点有没有被删；
   * lib/live2d/live2d_stage.dart —— applyPreset / set_scales 下发路径；
   * lib/live2d/live2d_bridge.dart —— 协议帧、stage-bg；
   * lib/settings/sections/tts_section.dart —— TTS 配置项一个不少；
   * lib/chat/turn_liveness.dart、lib/state/ui_state_tracker.dart —— 回合/错误消费语义。
4. 把重设计中**已删除的字段/常量**（如 syncShellStageBg 的旧引用）与 main 的新字段对齐，不要留下死引用。
[文件归属] 本任务独占 shell/flutter/lib/** 与 shell/flutter/test/**（A1 已收口，A3* 尚未开工）。
[门禁] analyze + test + **flutter build web --release --base-href /app/ --no-web-resources-cdn**；
  若碰了 crates/** 另跑 Rust 门禁（本任务预期不碰）。
[回报] 112 个文件的落点 / 29 个冲突逐个的解法一句话 / 资产守护断言结果 / 未决冲突清单（务必给维护者）。
~~~~

## A3a（A2 后，可与 A3b/A3c 并发）· 背景 P0 修复

~~~~
[工作区] /home/skystar/Live2D-Ai-fe。[开工前读] 规划 §4 的 P0 全部 + 审计复现 zz_audit_tmp_test.dart 的 A/B 组。

【任务 A3a：数据丢失与拖动排序】
必做：
1. **P0-1 背景图永久丢失**：让 BackgroundStore 能区分「**读失败**」与「**确实没有**」
   （三态 read，或新增 probe()/exists()）。读失败时：**绝不摘项、绝不置 migrated、绝不剪枝**。
   空库剪枝前先确认 store 健康（打开成功且探测可读）。
   验收：新增回归「瞬时读失败 → prefs 不变、migrated=false、retainOnly 未被调用」+「第二次启动不删字节」。
2. **P0-2**：background_hydration.dart:67 的 store.put **必须检查返回值**；失败则**保留旧 dataUrl**、不写回 id-only。
3. **P0-3 拖动排序双重减 1**：删掉 shell_prefs.dart:304 的 `if (target > oldIndex) target -= 1;`
   （appearance_section.dart:698 用的是 onReorderItem，Flutter 3.47.3 SDK 明文它已调整 newIndex）。
   同时更新 shell_prefs.dart:293-298 那段基于**已废弃 onReorder 语义**的注释。
   验收：新增回归「[A,B,C] 向下拖一格 → [B,A,C]」（旧实现是 no-op，必须红→绿）。
4. **P0-4**：改 background_store_test.dart:122 的期望——它现在把「存储挂掉→偏好退回没有图」当契约钉死，
   正是 P0-1 的受害者。改成「存储挂掉 → 偏好**原样保留**、不做破坏性写回」。
5. **P0-5**：删掉 appearance_section.dart:455/:475 里「自动加强」「随图变亮自动加强」的文案
   （background_logic.dart:85-97 已显式删除亮度项）。改成与实现一致的说明。
[文件归属] lib/data/**、lib/app/shell_prefs.dart、lib/settings/display_prefs.dart（仅 store 相关）、
  lib/settings/sections/appearance_section.dart（仅遮罩文案）、test/background_*_test.dart、test/display_prefs_test.dart。
  **不要碰** lib/ui/shell_backdrop.dart / field_row.dart / tokens.dart（归 A3b）。
[回报] 每条 P0 的「先红后绿」原始输出 / 新增测试名。
~~~~

## A3b（A2 后，可与 A3a/A3c 并发）· 死代码 + 文档腐烂 + 读数

~~~~
[工作区] /home/skystar/Live2D-Ai-fe。[开工前读] 规划 §4 的 P1-1/P1-6 与 P2 全部。

【任务 A3b：静默失效与文档腐烂】
必做：
1. **P1-1**：删 kShellSurfaceAlpha（lib/ui/shell_backdrop.dart:43）与其文档块（:35-42 声称它「守住可读性」，
   实际没有任何消费者）；删 test/shell_backdrop_test.dart:172-173 那两条**断言常量字面值**的空转测试；
   删 lib/ui/chat_panel.dart:92 对它的指引。删完确认没有其它引用（grep 全仓）。
2. **P1-6**：lib/ui/field_row.dart 的读数（:208 _format(value)）改成夹持后的值（与 :194 的 Slider 一致）；
   并把滑杆区间与 display_prefs.dart 的 clamp 对齐（maxSlideInterval=3600 vs maxSlideIntervalSeconds=300
   这对矛盾必须消除：或统一到同一常量，或给两个常量写清各自的语义并让 UI 显示真实值）。
3. **P2 文档腐烂**：修 lib/ui/shell_backdrop.dart:7/:12（还在讲已删除的 syncShellStageBg、宣称支持
   cover/contain/stretch/tile 而 maxImageFit=1）、lib/app/app_shell.dart:457-461（两段旧文档叠着）、
   lib/ui/chat_panel.dart:92；补 AppMaterial.toString() 漏掉的 uiTransparency（tokens.dart 附近）。
4. **P2 弱测试**：test/app_shell_background_test.dart:44-63 的源码字符串扫描（src.contains(...)）换成
   **行为断言**（pump widget 后查 Scaffold.backgroundColor 的实际取值）。
[文件归属] lib/ui/shell_backdrop.dart、lib/ui/chat_panel.dart、lib/ui/field_row.dart、lib/design/tokens.dart、
  lib/app/app_shell.dart（仅文档注释）、test/shell_backdrop_test.dart、test/app_shell_background_test.dart、
  test/field_row_test.dart、test/design_tokens_test.dart。
[回报] grep 证明 kShellSurfaceAlpha 已零引用 / 每条门的原始输出。
~~~~

## A3c（A2 后，可与 A3a/A3b 并发）· 复现转正 + 断言加固

~~~~
[工作区] /home/skystar/Live2D-Ai-fe。[开工前读] zz_audit_tmp_test.dart 全文 + 规划 §4 P1-8/P1-9、§5.2。

【任务 A3c：把临时复现变成正式回归】
必做：
1. 把 zz_audit_tmp_test.dart 的 15 条**分类搬迁**：
   * A 组（数据丢失）、B 组（拖动）、C 组（clamp/索引）、D 组（UI 断链）按行为拆进对应的正式测试文件；
   * 已被 A3a/A3b 修掉的 → 断言**正确行为**（不再是「给 bug 拍照」）；
   * 仍未决定的（C3 轮播索引语义）→ 先保留为带 TODO 的说明，**不要**假装已定。
2. 搬完**删除** test/zz_audit_tmp_test.dart（它的头注就是「用完即删」）。
3. 断言加固（规划 P1-8/P1-9）：
   * theme_palette_test 的 onAccent ≥4.5 是**零检验力**（onAccent 黑白二选一，数学下界 4.58）——
     改成对任意 accent 值的属性测试，或至少断言「二选一逻辑存在」；
   * 「同色相家族」目前只比 surface/alt/raised 的 sign(b-r)，**不与 accent 比对**——补上；
   * **缺 ink vs raised ≥4.5** ——补上；
   * 白套 ink 色度仅 1/255，与 tokens.dart 注释「推 9% 暖白」不符——要么真上色相，要么改注释，二选一并写清。
[文件归属] shell/flutter/test/**（只动测试）+ 必要时 lib/design/tokens.dart 的**注释**。
  与 A3a/A3b 的测试文件若重叠 → 让 A3a/A3b 先收口。
[门禁] flutter analyze && flutter test；**测试计数必须 ≥955 且不再包含 zz_**。
[回报] 15 条各自的落点 / 删文件确认 / 新计数。
~~~~

## A4（收口，串行）· rc.5 门禁 + 肉眼 + 发布说明

~~~~
[工作区] /home/skystar/Live2D-Ai-fe。
【任务 A4：rc.5 收口】
必做：
1. 全套门禁并贴原始输出：
   cd shell/flutter && flutter analyze && flutter test && flutter build web --release --base-href /app/ --no-web-resources-cdn
   cd /home/skystar/Live2D-Ai-fe && ./scripts/ignite.sh --check   # 四项全 ok
   若碰过 Rust：cargo test --workspace --all-targets / --doc / fmt --check / clippy -D warnings / xtask rust-ratio
2. **Win 浏览器肉眼 checklist**（贴结论，不要只说「正常」）：
   * 背景：选图 → 可见 → 清图 → **刷新后仍然正确**（P0-1 的正面验证）；
   * 背景库：拖动排序（向下拖一格必须真的动）、预览可达、删除后磁盘清空；
   * 遮罩档位文案与观感一致；
   * **资产回归**：发一条消息，确认 ①皮套有动作/表情反应 ②语音正常 ③口型在动 ④导演可观测（dev_mode）四栏有数据。
3. 写 docs/releases/v0.2.0-rc.5.md（版本三处同步：Cargo.toml / pubspec.yaml / README 首屏）。
[回报] 门禁数字 / 肉眼逐条结论 / 发布说明路径。
~~~~

---

# Stage B · 0.2.0-rc.6：背景透传追平参考

> 参考 shalldie/vscode-background @ eef5ddb（v3.1.0）。**参考机制，不抄实现**。
> 允许并发的判据：文件零重叠。B1 与 B2 都碰 display_prefs.dart ⇒ **串行**。

## B1（先行，串行）· imageFit 扩档 + tileSize

~~~~
[工作区] /home/skystar/Live2D-Ai-fe（或编排者指定的 rc.6 分支）。
[开工前读] 规划 §5.1（参考特性模型）+ §5.2（差距表）+ §5.3 第 1 条。

【任务 B1：size 追平参考】
背景：参考用 CSS background-size/position；我们只有 cover/contain（display_prefs.dart:285 maxImageFit=1），
而 shell_backdrop.dart:12 的文档**已经宣称**支持 stretch/tile——文档与实现不符（P2），现在把它做实。
必做：
1. display_prefs.dart：imageFit 扩到 cover / contain / stretch / tile 四档；新增 tileSize（仅 tile 有效，
   给区间与默认值）；**删除或写明 maxImageFit=1 这个假上限**的理由。
2. lib/ui/shell_backdrop.dart：实现四档（BoxFit + CustomPainter 平铺）；坏图/无图退回底色不抛。
   注意：这里画的是**壳自己那层**，舞台背景走渲染面协议，**不要动 wasm**。
3. 旧档兼容：存量 imageFit 值映射不许错位（写上迁移规则 + 回归）。
4. appearance_section.dart：铺法选项补齐四档；位置九宫格保持。
[文件归属] lib/settings/display_prefs.dart、lib/ui/shell_backdrop.dart、lib/ui/background_patterns.dart（若需）、
  lib/settings/sections/appearance_section.dart（仅铺法那一段）、对应测试。
[回报] 四档各自的截图或像素断言 / 存量档位迁移回归结果。
~~~~

## B2（B1 后）· 轮播索引语义 + 管理/预览分离

~~~~
[开工前读] 规划 §5.3 第 4/5 条 + zz_audit 复现的 C3/D1/D2/D3/D4。

【任务 B2：把「控件说一套、画面做一套」收口】
必做：
1. effectiveBackground 恒返回 backgrounds[0]、effectiveBackgroundIndex 恒 0，而轮播索引在
   ShellSlideshow 的运行时状态里。让偏好/UI 与**实际在画的那一张**一致（或明确把索引提出成单一真源）。
   验收：轮播跑到第 2 张时，设置页的「当前」标记与铺法区显示的是第 2 张的属性。
2. currentItemIsImage 永远看第 0 项 ⇒ 来源=舞台那张时铺法/位置被藏但 imageAlign 仍生效。修正为看**当前项**。
3. _managing => _selected.isNotEmpty || _items >= 2 ⇒ ≥2 项时预览永远点不到。
   拆成显式「管理模式」开关（或长按进入管理），让「先看这一张」在任意库大小下可达。
4. 轮播控件在库 <2 项 / 来源=舞台那张时**不该装作能生效**：要么禁用+说明，要么隐藏（P4 禁止静默失效）。
[文件归属] lib/settings/display_prefs.dart、lib/ui/shell_slideshow.dart、lib/app/app_shell.dart、
  lib/settings/sections/appearance_section.dart、对应测试。
[回报] 每条的前后行为对比 / 新增测试名。
~~~~

## B3（等 B1/B2 收口）· 逐图样式覆盖 + 全局开关

~~~~
[开工前读] 规划 §5.2（style/styles 那一行）+ §5.3 第 2 条。

【任务 B3：per-item style】
必做：
1. 每张背景图可单独覆盖 opacity / fit / align（缺省回落全局）；字段进 DisplayPrefs 的
   copyWith / lerp / toValuesMap / 相等性（P4：漏字段会静默失效，有结构枚举测试就补上）。
2. UI 放在背景库的单项编辑里（参考的 styles[] 语义）；**任意 CSS 字符串不做**。
3. 迁移：无该字段的旧档 = 全部回落全局。
[文件归属] lib/design/background_item.dart、lib/settings/display_prefs.dart、lib/ui/shell_backdrop.dart、
  lib/settings/sections/appearance_section.dart、对应测试。
  **注意与 B1/B2 的 display_prefs 冲突** ⇒ 必须等 B1/B2 收口。
[回报] 结构枚举测试结果 / 旧档迁移回归。
~~~~

## B4（任意时刻，docs-only，可并行）· 偏离说明

~~~~
【任务 B4：把「不采纳」写成明文】
参考支持而我们**不做**的：在线 https 图 / 本地文件夹 / ~ 与环境变量展开 / 任意 CSS style /
editor 的 useFront / 跨渲染面的多区域。
必做：写 docs/architecture/background-parity-vscode-background.md：
  逐条「参考怎么做 → 我们为什么不做 → 红线依据」。离线优先（ignite.sh --check 扫 gstatic.com）是硬依据。
  多区域要写清：壳内子区域可做（纯 Flutter），舞台分区=改渲染面=后端口径。
[文件归属] 新增 docs/**（不碰代码）。
[回报] 文件路径 + 每条偏离的一行理由。
~~~~

---

# Stage C · 0.2.0-rc.7：技术债 + 项目管理

## C1（docs-only，可与 C2 并行）· 文档单一化

~~~~
【任务 C1：消灭「两份 AGENTS.md」】
背景：本 worktree 的 AGENTS.md 写「动作在产品路径上不存在、director 已删除、mod_count_is_three」，
而 0.2.0 线（main）是五 Mod + 表演协议 v1。两套 doctrine 同时存在于同一仓库，会误导下一个执行者。
必做：
1. 以 **main 版 AGENTS.md 为底**重写（保留「动作与表演的现行状态」「导演可观测」「每皮套动作幅度」三节），
   删除本 worktree 那份「动作不存在」的旧 doctrine。
2. 新增一节「前端显示层（背景库 / 材质 / 排版）」（写清 DisplayPrefs 的背景字段族与真源文件）。
3. 处理 docs/releases/v0.3.0.md 的版本线矛盾：标注「历史草案，未发布」或移入 docs/legacy/。
4. 更新「变更历史」，把 0.2.0 线 rc.2/rc.3/rc.4 补进（本 worktree 缺）。
[文件归属] AGENTS.md、docs/releases/**、docs/README.md。
[回报] 新旧 doctrine 的 diff 摘要 / 矛盾项消除清单。
~~~~

## C2（编排者/维护者做，不要派给 worker）· 仓库卫生

~~~~
【任务 C2：24 worktree + stash + 未跟踪文档】
**git 破坏性操作只由编排者/维护者执行，禁止 worker 做。**
必做（每条贴命令与结果）：
1. git worktree list + git branch --merged main：已并入的 worktree → git worktree remove + git branch -d；
   未并入的**逐个写明为什么还活着**（不要默认删）。
2. git stash list：stash@{0}（pre-integration dirt）解包归类或删。
3. 6 份未跟踪文档（docs/audit/2026-09-15-five-mod-review.md 等）：并入 0.2.0 线或移 docs/legacy/。
4. .gitignore：临时测试命名 zz_* 与 build/、.dart_tool/ 明确排除。
[回报] 前后 worktree 数量 / 每条处置结果。
~~~~

## C3（C1 后，代码）· appearance_section 拆分 + W4 裁决落地

~~~~
【任务 C3】
1. lib/settings/sections/appearance_section.dart 现在 **1039 行**，超 AGENTS「≤500，豁免 ≤1000 需头注」，
   连豁免带都不满足。按「壳背景 / 背景库 / 舞台与口型 / 互动 / 材质」拆成多个文件，各自 ≤500 行。
   拆分**只搬不改**，行为断言必须原样通过。
2. W4 五项（AppPalette.solve 可计算配色 / 自定义 accent / density 三档 / lookPreset 风格预设 / 设置区搜索框）
   现在处于**静默丢弃**状态（代码 0、测试 0、减记 0）。给**明确裁决**：
   * 做 → 写清范围排期；不做 → 在 PLAN-frontend-redesign-2026-09-27.md 里加**减记**
     （与 radiusScale 的删除同规格：说明为什么删 + 一条防复活测试）。
   二者必居其一，不许继续悬空。
[文件归属] lib/settings/sections/**、lib/design/tokens.dart（若走减记则只加注释）、docs/plans/**。
[门禁] flutter analyze && flutter test（拆分后断言必须零变化）。
[回报] 拆分前后行数 / W4 裁决结论。
~~~~

---

# Stage D · 0.2.0 末版发布

~~~~
【任务 D1：发布（编排者执行，可让 worker 只做校验）】
必做：
1. 版本号三处同步：Cargo.toml / shell/flutter/pubspec.yaml / README 首屏 → 0.2.0。
2. 全套门禁（Rust 全项 + Flutter analyze/test/build + ignite.sh --check），贴**原始数字**。
3. Win 肉眼 checklist 全过一遍（背景 / 动作 / 语音 / 口型 / 导演可观测 / 模型切换）。
4. 写 docs/releases/v0.2.0.md；git tag v0.2.0；推 main → origin/main（本地领先 85+ 提交）。
5. 更新 AGENTS.md 的「当前版本」与变更历史。
[回报] 门禁数字 / tag / 推送结果 / 未做项与理由。
~~~~
