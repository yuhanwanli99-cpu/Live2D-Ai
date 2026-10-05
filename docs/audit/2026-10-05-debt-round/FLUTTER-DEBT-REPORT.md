# FLUTTER-DEBT-REPORT · task-4（W1-D · Flutter 债）

> 工作树：`/home/skystar/Live2D-Ai-fe`（分支 `chore/debt-round-2026-10-05`）
> 写作用域：`shell/flutter/lib/**`、`shell/flutter/test/**`、`docs/audit/2026-10-05-debt-round/`
> 报告人：teammate `flutter-debt` · 2026-10-05
> **本报告不采信自述**：每条结论都附命令与原始输出摘要；红-绿双向都真的跑过。

## 0. 门禁数字（最终树，即报告里所有改动都落地之后）

| 项 | 命令 | 结果 |
|---|---|---|
| 静态检查 | `flutter analyze` | **No issues found!**（exit 0） |
| 全量测试 | `flutter test` | **1564 passed / 0 failed**（基线 **1524** ⇒ **+40，只增不减**） |

**+40 的逐项对账**（`git diff --numstat` + 新文件）：

| 来源 | 增量 |
|---|---|
| `test/error_action_busy_resend_test.dart`（新） | +15 |
| `test/display_prefs_style_change_detection_test.dart`（新） | +11 |
| `test/mod_state_surface_test.dart`（由 `pet_desktop_state_test.dart` 改名重写，9 → 11） | +2 |
| `test/source_scan_test.dart`（反复制门禁扩表 + 词法器契约） | +8 |
| `test/design_tokens_lint_test.dart`（豁免面新门禁） | +3 |
| `test/admin_api_test.dart`（在册 id 交叉核对） | +1 |
| 合计 | **+40** |

**没有删掉任何测试**：`git diff` 里被删的 test/testWidgets 声明共 12 行，逐行核对全部是
**同文件内的改名**（admin_api 1、design_tokens_lint 1、source_scan 1）或**文件改名**
（`pet_desktop_state_test.dart` 9 条 → `mod_state_surface_test.dart` 11 条）：

```
$ git diff -U0 -- shell/flutter | awk '/^diff --git/{f=$3} /^-[^-]/{if ($0 ~ /test\(|testWidgets\(/) print f}'
a/shell/flutter/test/admin_api_test.dart            <- 改名（同一用例）
a/shell/flutter/test/design_tokens_lint_test.dart   <- 改名（同一用例）
a/shell/flutter/test/pet_desktop_state_test.dart    <- 9 条，全部在 mod_state_surface_test.dart 里（且多了 2 条）
a/shell/flutter/test/source_scan_test.dart          <- 改名（同一用例，改成表驱动）
```

**临时文件清零**（Lead 点名）：变红自证用的 `test/zz_red_proof_tmp_test.dart` 已删，
`ls test/ | grep -iE "tmp|red_proof|zz_"` → `(none)`；上面的 1564 就是最终树的实际条数。

---

## 1. 任务 1 · F-0007-2：busy 的「打断并重发」只 stop 不 send（P2 真 bug）

### 1.1 回源码核实（账本行号会腐烂，以源码为准）

- `lib/ui/error_actions.dart:39-46`（改前）：两个 busy 分支都是
  `ErrorAction(label: '打断并重发', onPressed: onStop)` —— `onSend` 参数**已经传进来**
  却在这条分支上完全没用。
- `lib/main.dart`（改前 :1328）：`onStop: () => unawaited(_stopWithCancellation())`。
- `lib/app/shell_chat.dart:57-62`：`_stop()` 只 `_chat.stop()` + `_ui.markStopped()`。

⇒ 审计「只调 onStop，从不重发」**成立**。

**★ 回源码时发现的第二层（账本没写、比第一层更隐蔽）**：就算把 busy 分支接上 `onSend`，
**也发不出去**——`shell_chat._send()` 读的是 `_input.text`，而 busy 现场输入框
**早已被清空**（`_send` 先 `_input.clear()` 再 POST；`chat_controller.send` 先上屏再 POST）。
于是 `_send()` 在 `text.trim().isEmpty` 处**静默 return**，按钮依然是「按了什么都不发」。
这一层决定了修法：**重发必须从会话记录里取回正文**。

### 1.2 改了什么（一条改动一个理由）

| 文件 | 改动 |
|---|---|
| `lib/ui/error_actions.dart` | 新增纯编排 `interruptAndResend({stop, resend})`（**先 await stop，再 await resend**）；`errorActionsFor` 的两个 busy 分支改用组合回调 `onInterruptAndResend`（旧参数 `onStop` 删除——它只剩这一条用途，留着就会再有「接一半」的机会） |
| `lib/chat/chat_message.dart` | 新增纯函数 `lastUserText(messages)`：从后往前找**最后一条用户消息**（跳过 assistant 失败气泡 / system 行 / 空文本）。它是「重发哪一条」的唯一判据，**VM 可测** |
| `lib/chat/chat_controller.dart` | `send(raw, {bool echoUser = true})`；新增 `resendLastUserMessage()`（`_streaming` 或没有可重发消息 → `false`；否则 `send(text, echoUser: false)`——**不补第二条用户气泡**）；新增 getter `lastUserMessageText` |
| `lib/app/shell_chat.dart` | `_interruptAndResend()`（= `interruptAndResend(stop: _stopWithCancellation, resend: _resendLastUserMessage)`）+ `_resendLastUserMessage()`（与 `_send` 同形的相位处理：`markTurnAccepted` 在 await 之前、失败按 `sendFailedLocally` 回落） |
| `lib/main.dart` | `errorActionsFor(...)` 的 busy 出路换成 `onInterruptAndResend`（块闭包，便于结构守卫复用已有的 balancedFrom/closureBodyAfter 判据） |
| `test/error_action_opens_settings_test.dart` | 调用点适配新签名；**顺带**把它的四个私有扫描 helper（`_balancedFrom`/`_closureBodyAfter`/`_matchBracket`/`_isSpace`）搬到 `test/support/source_scan.dart`（见 D6-c，避免第三次复制） |

**顺序是契约**：先 `stop`（服务端腾出 turn）再 `resend`；反过来的新请求会撞**同一个 busy**。
它被抽成 `interruptAndResend` 就是为了能被 Completer 钉住（`main.dart` 是 `package:web` 组合根，VM 里 import 不了）。

### 1.3 新增测试（`test/error_action_busy_resend_test.dart`，15 条）

- **真泵**（真 `ErrorBanner` + 真 `errorActionsFor` + 与生产同形的宿主）：
  「点『打断并重发』→ 先打断，再真的发出一轮」——按下后断言
  `order == ['stop','resend']` **且 `rounds == ['你好呀，帮我看看这个']`**（旧实现 rounds 为空）；
  另一条「只发一轮（点一下不该发两遍）」。
- 动作面：`code == busy` / 码缺失但文案带 busy / 非 busy 分支不受影响。
- 纯编排：resend 绝不能被吞掉；stop 还没回来就不许发。
- 纯判据：busy 现场取它前面那条用户消息 / 系统行不是用户消息 / 没有用户消息→null / 最后一条就是用户消息。
- 结构守卫（钉回生产接线）：main.dart 不再有裸 `onStop:` 且 `onInterruptAndResend` 是回调；
  shell_chat 的 `stop:` 在 `resend:` **之前**；`_resendLastUserMessage` 有 streaming/没得重发/markTurnAccepted 三处；
  `ChatController.resendLastUserMessage` 走 `send(..., echoUser: false)`。

### 1.4 红-绿双向（原始输出）

**绿（最终）**

```
$ flutter test test/error_action_busy_resend_test.dart
00:00 +14: 结构守卫：把行为级那一半钉回生产接线 chat_controller.dart：重发走 send(..., echoUser: false)，不补第二条用户气泡
00:00 +15: All tests passed!
```

**红自证 A：把 `interruptAndResend` 退回旧行为（只 stop，不 resend）**

```
-  await stop();
-  await resend();
+  await stop();
+  // RED-PROOF（临时）：旧行为——只打断，不重发。

$ flutter test test/error_action_busy_resend_test.dart
00:00 +11 -4: Some tests failed.
Failing tests:
  ...: 真泵：点「打断并重发」→ 先打断，再真的发出一轮 只发一轮（点一下不该发两遍）
  ...: 真泵：点「打断并重发」→ 先打断，再真的发出一轮 旧实现（只 stop）在这里 rounds 为空
  ...: 编排：interruptAndResend 两步都做，且 stop 先完成 resend 绝不能被吞掉（F-0007-2 的现场）
  ...: 编排：interruptAndResend 两步都做，且 stop 先完成 stop 还没回来就不许发（否则撞同一个 busy）
```

**红自证 B：把 `lastUserText` 退回「取最后一条」（不筛角色）**

```
+  // RED-PROOF（临时）：天真写法——取最后一条（不筛角色）。
+  if (messages.isNotEmpty) return messages.last.text;

$ flutter test test/error_action_busy_resend_test.dart
Failing tests:
  ...: 重发哪一条：lastUserText busy 现场：最后一条是 assistant 失败气泡 → 取它前面那条用户消息
  ...: 重发哪一条：lastUserText 没有用户消息 / 全是空文本 → null（没得重发就什么都不做）
  ...: 重发哪一条：lastUserText 系统行不是用户消息
```

两次都**已还原**（同文件再跑即 15 passed）。

---

## 2. 任务 2 · F-0034-01（P2 在线）+ F-0074-01（总闸零覆盖）

### 2.1 回源码核实

- `lib/settings/display_prefs.dart:968-971`（改前）逐项比较用的是
  `backgrounds[i].sameAs(other.backgrounds[i])` —— `BackgroundImage.sameAs`
  **只看 id**（`design/background_item.dart:365-366`），而 `BackgroundImage.==` 与
  `hashCode` 都含 `id + opacity + fit + align`、都不看 `dataUrl`。
- 闸门落点：`lib/app/shell_prefs.dart:146` 的 `if (next == widget.prefs) return;`
  与 `lib/main.dart:239` 的 `if (next == _prefs) return true;`。
- `onItemChanged` **已在生产接线**（`lib/settings/sections/appearance_background.dart:393`
  → :961-964），所以 F-0034-01 是**在线**缺陷（不是潜在陷阱）：只改逐图样式 ⇒ `==` 判等
  ⇒ 闸门 return ⇒ 既不 setState 也不落盘、渲染面也不重发，**没有任何错误**。
- F-0074-01：测试目录内 `DisplayPrefs` 相等断言 = 0；唯一相邻的 `background_store_test.dart:58`
  两侧都是默认样式 ⇒ 对本轴**结构上无法失败**。

### 2.2 改了什么

- `lib/settings/display_prefs.dart`：`backgrounds[i].sameAs(...)` → `backgrounds[i] != other.backgrounds[i]`
  （附长注释说明 `sameAs` 的语义是「字节读回前后算同一项」，只配给水合去重用）。
  附带好处：`==` 与 `hashCode` 重新**对称**（改前「不等却可能同哈希」）。
- 新增 `test/display_prefs_style_change_detection_test.dart`（11 条）：
  - 聚合层：三个样式轴各一条「仅逐图 X 不同 ⇒ !=」（表驱动，加字段只需加一行）+ 哈希对称
    +「全字段相同（含样式）⇒ == 且同哈希」+「字节不同不算变」。
  - `sameAs` 语义：同 id 不同样式 ⇒ sameAs 真而 == 假（钉成**有意为之**）；id 不同 ⇒ 假；
    同 id 一个有字节一个没有 ⇒ 两者都判同一项；图案：同 id / 不同 id / 与图片永不相等。
  - **真泵接缝**：与生产闸门同形的宿主（`if (next == prefs) return;` + 记下本该落盘的每一份）
    驱动**真** `AppearanceSection`：点「样式」→「单独设置」铺法 ⇒ 断言偏好确实变化
    （`item.fit == fitCover`、`dataUrl` 原样带走）**且 toJson/fromJson 往返后样式仍在**；
    第二条覆盖「清掉逐图值」这一方向。

### 2.3 红-绿双向

**绿**

```
$ flutter test test/display_prefs_style_change_detection_test.dart
00:01 +11: All tests passed!
```

**红自证：把 :970 改回 `sameAs`**

```
-      if (backgrounds[i] != other.backgrounds[i]) return false;
+      if (!backgrounds[i].sameAs(other.backgrounds[i])) return false; // RED-PROOF

$ flutter test test/display_prefs_style_change_detection_test.dart
Failing tests:
  ...: 变更检测接缝（…） 再改回去（清掉逐图值）同样算「变了」，不是第二次被吞
  ...: 变更检测接缝（…） 只改一张图的铺法 → 偏好确实变化，且写成 JSON 再读回来仍在
  ...: 聚合层（…） 仅逐图「不透明度」不同 ⇒ 判为「变了」（旧实现走 sameAs ⇒ 判为没变）
  ...: 聚合层（…） 仅逐图「位置」不同 ⇒ 判为「变了」（旧实现走 sameAs ⇒ 判为没变）
  ... and 2 more
```

还原后连邻近的 `display_prefs_test` / `display_prefs_background_fit_test` /
`display_prefs_slide_index_test` / `background_store_test` 一起跑：**142 passed**。

---

## 3. 任务 3 · D6 假绿灯治理

### 3.1 D6-a `design_tokens_lint_test`：硬编码豁免表 → 路径前缀 + 处数上限

**改前**：`Set<String> kNamedTimingFiles = {两个 .dart 全路径}` 是**整文件豁免**，两个毛病：
① 同一文件里再加一处 UI 过渡时长（真该红的）没人知道；② 文件改名/搬家后那条**静默失效**
（死豁免 = 未来一个没人注意的放行位）。令牌声明豁免同样是 `Set<String>` 文件名名单。

**改后（两条都改成路径前缀规则）**：

- `kTokenDeclarationPrefixes`（`lib/design/tokens`、`lib/design/typography`，不带扩展名）
  + `isTokenDeclarationPath(path)`（`startsWith`）：令牌拆分/搬迁自动落在前缀下，不需要
  「往名单里再加几行」；**不是目录豁免**——`lib/design/breakpoints.dart` 仍要扫。
- `kTimingExemptPrefixCaps: Map<String,int>`（`lib/state/live_region`→1、
  `lib/voice/voice_listen_controller`→3）+ `Rule.quotaCaps` / `Rule.quotaFor(path)`：
  `scanSource` 现在**前 cap 处放行、多出来的一律算违规**（真实扫描与合成用例仍走同一条代码路径）。
- 新门禁（「豁免面本身要小且有效」组）：前缀语义（`live_region_split.dart` 也命中、
  `state/other.dart` 不命中）；**每个前缀必须命中一个现存文件**（零命中 = 死豁免，判红）；
  配额总量写死（只许变小）；合成源码 cap+1 → 报红；真实文件都在配额内。

**红自证**：把 `lib/state/live_region` 的配额从 1 改成 0：

```
Failing tests:
  ...: 裸值扫描（引用侧兜底） 规则「协议时长混用」：UI 动效时长只能取 AppDurations 的 4 档
  ...: 豁免面本身要小且有效 时长豁免是**路径前缀 + 处数上限**，不是「把文件名抄进名单」（D6）
  ...: 豁免面本身要小且有效 真实文件都在配额内（现网零违规，上限就是当前处数）
  ...: 豁免面本身要小且有效 配额真的会拦：超出上限的那一处算违规（整文件豁免不会红）
```

（真实树被配额拦住 = 这条门禁不是空转；还原后 **24 passed**。该文件 **21 → 24 条**：
+3 条新门禁；另 1 条是同名替换（「恰好是两个令牌声明前缀」），其余是既有的规则扫描与断点覆盖面用例。）

> **不在我作用域的残留**：`docs/design/web-ui-spec-v3.md:2087` 仍写着旧常量名
> `kNamedTimingFiles`（历史记录）。docs/design 不在 task-4 写作用域，未改。

### 3.2 D6-b 已删 crate 的硬编码夹具假绿灯（**换成真断言，不是搬位置**）

**根因**：夹具手抄 `{"id":"local-llm"...}` / `{"id":"pet-desktop"...}`，而这两个 crate
（`live2d-ai-mod-local-llm` / `live2d-ai-mod-pet-desktop`）已在 W2-A **物理删除**
（`ARCHIVED-mods.md` `2.4/`3.1）——前端只是解析 JSON，**删掉 Rust 后它们照样绿**。

**换成什么**：新增 `test/support/registered_mods.dart` → `registeredModIds()`：从**唯一真源**
`crates/live2d-ai-desktop/src/main.rs` 的 `AVAILABLE_MOD_FACTORIES`（先剥注释、锚在 `= &[` 上、
括号配平取数组体、抽 `live2d_ai_mod_<x>::FACTORY` 并把 `_`→`-`）解出在册 id 集合；
**解析失败返回空集**，调用方一律断言非空（防零命中空转）。当前解出 5 个：
external-input / persona / voice-input / memory / director。

| 文件 | 改动 | **现在它能红** |
|---|---|---|
| `test/mod_state_surface_test.dart`（原 `pet_desktop_state_test.dart` 改名重写，9 → 11） | 头注改成「通用 Mod 运行态面」，不再声称校验已删的 Rust 生产者；**契约夹具**（`ModsApi.state` 真端点）id 用**在册**的 `director`；`ModsSection` 的 widget 夹具改用**显式合成**的 `demo-mod`（在册五个 Mod 都有专用面板，会改变展开树，测不到通用块——注释写明它不对应任何 crate，因此不存在「删 crate 仍绿」的声明） | 新增 2 条：`registeredModIds()` 非空且 containsAll(5 个在册 id)；契约夹具 id 在册。**红自证**：把 `modStateBodyId` 改回 `'pet-desktop'` ⇒ `Failing tests: … **契约夹具**指向的 id 在册（…）会红` |
| `test/admin_api_test.dart` | `kRealModsJson` 里的 `local-llm` → `director`；`setEnabled` 路径用例 id 同步；头注改为「夹具**形状**来自真实抓包 + id 已按当前在册集合更新」 | 新增 1 条：夹具两个 id ∈ `registeredModIds()`；同样的红路径 |
| `test/mods_section_test.dart`（**任务只点名前两个，这处是我按同一意图顺手做的**） | `local-llm` / `本地大模型` → **合成** `demo-mod` / `演示 Mod` + 头注声明「不对应任何在册 crate」 | 它本来就没有后端声明（ModsSection 是注入式），改后**没有任何已删 crate 的引用**；不假装它是真断言 |
| `lib/settings/sections/dev_tools_section.dart`（注释纠偏） | `kWindowReasonNativeShellDormant` 的注释原称「守卫在 `test/pet_desktop_state_test.dart` 与 Rust 的 `window_reason_string_is_stable_and_ascii`」——**两处都已消失**（crate 删除 + Rust 测试冻结）。改为写明：跨语言守卫随 crate 冻结在台账里，常量因 `window` 是通用协议键而保留 | 注释与事实一致（审计规则：读到长注释必须反查它描述的代码还在不在） |

```
# 绿（最终）
$ flutter test test/mod_state_surface_test.dart        → +11: All tests passed!
$ flutter test test/admin_api_test.dart                → +20: All tests passed!
$ flutter test test/mods_section_test.dart             → +11: All tests passed!

# 红（临时把契约夹具 id 退回已删 crate）
$ flutter test test/mod_state_surface_test.dart
Failing tests:
  ...: 夹具的 Mod id 必须**在册**（D6：crate 被删就该红） **契约夹具**指向的 id 在册（…）会红）
```

### 3.3 D6-c `_stripComments` ×2 同名不同义 → 单一定义 + 反复制门禁补强

**逐消费点判语义后再合并**：

| 消费点 | 它要什么 | 判读 |
|---|---|---|
| `chat_bubble_labels_test.dart:223` 的 `_stripComments` | **保留**字符串字面量（断言 `contains('本轮没有文字输出')` / `contains('（生成失败）')`） | 必须保留 |
| `director_observer_test.dart:95` 的 `_stripComments` | 也保留字符串（「字符串里出现持久化名字同样是落盘信号」），**但不认识字符串**、块注释可嵌套 | 保留 + 修掉「不认识字符串」这个洞 |

**发现（是真的洞，不是等价改写）**：director 那份**不认识字符串**——`'http://x'` 会被从 `//` 起
吃到行尾，**行尾的标识符一起消失**；而那一行恰好是「有没有写盘」的证据 ⇒ 那是**假绿灯方向**。
两个受保护文件当前没有这种写法（grep 零命中），所以合并不改变它们今天的结论，但把这类洞关掉。

**改了什么**：`test/support/source_scan.dart` 里一个扫描器 `_scan(src, {keepStrings})`
（字符串认识 + 块注释**可嵌套**）+ 两个只差一个开关的出口
`stripCommentsAndStrings` / `stripCommentsKeepStrings`；两个消费点删掉私有副本改用共享版；
同时把 `error_action_opens_settings_test.dart` 的四个私有 helper
（`balancedFrom` / `closureBodyAfter` / `matchBracket` / `isSpace`）**搬进同一处**供两个文件共用。

**反复制门禁补强**（`source_scan_test.dart`，12 → 20 条）：`kSingleDefinitionHelpers` 表
（5 个共享定义）+ 顶格定义正则 + 每个名字**恰好一处**（**零命中同样判红**：`[] != [期望]`，
reason 里写明「那说明判据正则与源码形状脱节，门禁正在空转」）+ 扫描根覆盖自检 + 「判据是定义而不是提到」。

**红自证**：临时在 `test/zz_red_proof_tmp_test.dart` 抄一份定义：

```
Expected: ['test/support/source_scan.dart']
  Actual: ['test/support/source_scan.dart', 'test/zz_red_proof_tmp_test.dart']
Failing tests:
  ...: 反复制门禁：每个共享工具只允许一个定义点 每个共享定义恰好出现在 `test/support/source_scan.dart`
```

该临时文件**已删除**，随后 `flutter test test/source_scan_test.dart test/mod_state_surface_test.dart`
→ `+31: All tests passed!`。

**词法器改动的影响面**：`test/**` 下有 17 个消费文件，全部单独跑过一遍
（`background_hydration_window`、`no_backdrop_filter`、`stage_progress_wiring`、`transient_results`、
`motion_wiring`、`asset_guard_*` ×3、`glass_rim`、`design_tokens_test` 等）⇒ **228 passed / 0 failed**。

---

## 4. 逐条「结论 → 证据」

| 结论 | 证据（命令 → 原始输出摘要） |
|---|---|
| 全量门禁绿且只增不减 | `flutter analyze` → No issues found；`flutter test` → `00:44 +1564: All tests passed!`（基线 1524） |
| F-0007-2 现场成立且已修 | 源码 `error_actions.dart` 改前 `onPressed: onStop`；真泵断言 `rounds == ['你好呀，帮我看看这个']`；红自证 4 条失败 |
| 「重发」不能读输入框 | `shell_chat._send` 先 `_input.clear()`；`lastUserText` 4 条纯函数用例（红自证 3 条失败） |
| F-0034-01 在线且已修 | `display_prefs.dart:970` 改前 `sameAs`；真泵接缝 2 条 + 聚合层 5 条；红自证 6 条失败 |
| 豁免表已改路径前缀 + 配额 | `design_tokens_lint_test` 24 passed；配额置 0 ⇒ 4 条失败（含真实树被拦） |
| 假绿灯换成在册真断言 | `registeredModIds()` 从 main.rs 解 5 个 id；夹具退回 `pet-desktop` ⇒ 判红 |
| 反复制门禁有效 | 抄一份定义 ⇒ 门禁判红；临时文件已删；`+31 passed` |
| 全量条数对得上 | 1524 + 15 + 11 + 2 + 8 + 3 + 1 = 1564 |

## 5. 未决问题 / 需要 Lead 决策

1. **`chat_controller.dart` 684 行（>500）**：既有 650 行，本轮 +34。`display_prefs.dart` 1157→**1169**、
   `main.dart` / `dev_tools_section.dart` 同为既有超限。`chat_controller` 是**唯一由我推高的**。
   选项：(a) 我加「≤1000 豁免头注 + 理由」；(b) 留给 D2 拆文件；(c) 压缩注释（仍到不了 500）。
   **我没有擅自加豁免头注**，请裁决。
2. **无 code 时的「重试」分支同类缺陷**：`errorActionsFor` 的 fallback（`network_error` / 未知错）
   给的是 `重试 → onSend → _send()`，它同样读输入框——输入框空时是 no-op。
   任务只点名 busy 分支，我**没有**改（改它会把「重发上一条」与「发送框里新输入」两种语义搅在一起，
   需要先定产品语义）。要不要一并收口？
3. **文档残留（不在我的写作用域）**：`docs/architecture/ARCHIVED-mods.md:195`、
   `docs/architecture/pet-desktop-mod-v0.md`（多处）、`docs/plans/parallel-mods/*` 仍指旧文件名
   `pet_desktop_state_test.dart`；`docs/design/web-ui-spec-v3.md:2087` 仍写 `kNamedTimingFiles`。
   旧文件名在库里已不存在（已改名 `mod_state_surface_test.dart`）——是否请文档 owner 一起改？
4. **第三个假绿灯文件**：`mods_section_test.dart`（任务只点名两个）我按同一意图改成了合成 `demo-mod`。
   若 Lead 认为超出「逐条照做」的边界，请指出，我可以还原成只改两处。
5. **通用运行态块在在册 Mod 上渲染不到**：在册五个 Mod 都有专用面板（`kModPanels`），
   导演还把整块关掉（`showRuntimeState => false`）⇒ `ModsSection` 的「运行态（只读）」通用块
   目前只有第三方/无面板 Mod 能走到。我**没有**借机改产品代码，只把事实写进头注——
   这条「通用块是否还有活的生产者」请 Lead 判是否需要单独立项。

## 6. 我**没有**做的事（与原因）

- 没跑 cargo / rust-ratio / code-stats：不在 task-4 范围（Rust 侧 rust-debt / Lead 跑）。
- 没改 `crates/**`、`.github/**`、`scripts/**`：写作用域外。
- **没做仓库级 dart format**（硬约束）。只对 7 个文件做了单文件 `dart format`：
  2 个是我改脏了 baseline-clean 的文件（`design_tokens_lint_test`、`admin_api_test`），
  5 个是我新建/改名重写的文件（`support/registered_mods`、`source_scan_test`、
  `error_action_busy_resend_test`、`display_prefs_style_change_detection_test`、`mod_state_surface_test`）。
  其余被点改的老文件在基线上就不是 dart format clean（`main.dart` / `dev_tools_section.dart` 等），
  我没有顺势重排它们（避免无关 churn）。
- 没做真机 / headless 浏览器验收（属 W1-A `browser-qa`）；本轮结论全部是静态 + 单测层。
- 没做 git commit / checkout / stash / reset / worktree。

## 7. 改动文件清单（绝对路径）

**lib**
- `/home/skystar/Live2D-Ai-fe/shell/flutter/lib/ui/error_actions.dart`
- `/home/skystar/Live2D-Ai-fe/shell/flutter/lib/chat/chat_message.dart`
- `/home/skystar/Live2D-Ai-fe/shell/flutter/lib/chat/chat_controller.dart`
- `/home/skystar/Live2D-Ai-fe/shell/flutter/lib/app/shell_chat.dart`
- `/home/skystar/Live2D-Ai-fe/shell/flutter/lib/main.dart`
- `/home/skystar/Live2D-Ai-fe/shell/flutter/lib/settings/display_prefs.dart`
- `/home/skystar/Live2D-Ai-fe/shell/flutter/lib/settings/sections/dev_tools_section.dart`

**test**
- `/home/skystar/Live2D-Ai-fe/shell/flutter/test/error_action_busy_resend_test.dart`（新）
- `/home/skystar/Live2D-Ai-fe/shell/flutter/test/display_prefs_style_change_detection_test.dart`（新）
- `/home/skystar/Live2D-Ai-fe/shell/flutter/test/support/registered_mods.dart`（新）
- `/home/skystar/Live2D-Ai-fe/shell/flutter/test/mod_state_surface_test.dart`（由 pet_desktop_state_test.dart 改名）
- `/home/skystar/Live2D-Ai-fe/shell/flutter/test/support/source_scan.dart`
- `/home/skystar/Live2D-Ai-fe/shell/flutter/test/source_scan_test.dart`
- `/home/skystar/Live2D-Ai-fe/shell/flutter/test/design_tokens_lint_test.dart`
- `/home/skystar/Live2D-Ai-fe/shell/flutter/test/error_action_opens_settings_test.dart`
- `/home/skystar/Live2D-Ai-fe/shell/flutter/test/chat_bubble_labels_test.dart`
- `/home/skystar/Live2D-Ai-fe/shell/flutter/test/director_observer_test.dart`
- `/home/skystar/Live2D-Ai-fe/shell/flutter/test/admin_api_test.dart`
- `/home/skystar/Live2D-Ai-fe/shell/flutter/test/mods_section_test.dart`
- `/home/skystar/Live2D-Ai-fe/shell/flutter/test/mod_config_draft_test.dart`（仅注释里的一处路径名）

**删除**
- `/home/skystar/Live2D-Ai-fe/shell/flutter/test/pet_desktop_state_test.dart`（内容迁到 `mod_state_surface_test.dart`）
