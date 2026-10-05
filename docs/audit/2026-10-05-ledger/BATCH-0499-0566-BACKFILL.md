# BATCH-0499..0566 · 补落（回填）

> ⚠ **本文件不是一批**。B0499–B0566 这 68 批曾在对话里逐批汇报，**未落盘**；
> 对话消息不跨轮保留 ⇒ 那 68 批的结论**等于不存在**。本文件按原汇报**逐条回填**，
> 每条保留 `file:line`，以便**复核**。
> 回填于 BATCH-0568（第二次同类故障；第一次 B0485–B0487，已在 B0488 补过）。

## 回填原则
- **只记可核的**（文件 + 行号 + 原文片段）；凡「未核」项**照原样保留为未核**，不补写结论。
- 68 批中 **0 findings 者 61 批**、**含 1 条 P3 观察者 7 批**；**无一条进入 FINDINGS.md**。

---

## 面板 / 外观轴

**B0499–B0500** `lib/settings/sections/pane_helpers.dart:1-30` — 头注「草稿优先、服务端兜底」；收敛理由是**「显示的值」与「将要提交的值」走同一个判据**，不是省代码；六个助手（`effString` / `effNullableString` / `effInt` / `effBool` / `effTriInt` / `effTriBool`），三态版把 `Tri.clear()` 归进「没有草稿」。

**B0501** `:31-39` — `isDrafted(Object?) => drafted != null`；⚠ 与 `effTriBool` 对 `TriClear` **给出相反答案**（一个算「改过」、一个算「没有草稿」）；两答各自可辩护，**缺一句「这是有意的」**。

**B0502** `test/settings_api_test.dart:65/117/171/213` — `TriClear` 的**写侧**四道断言（`:213` 测试名「`TriClear()` 发出的是 `null`」）；⚠ `isDrafted` **全 `test/` 零命中** ⇒ 「写侧有测试、显示侧没有」。

**B0503** `test/` 七个 `*_panel_test.dart`、**零个** `pane_helpers` 测试 ⇒ 共享判据无单测；落点是新文件。

**B0504–B0505** `lib/settings/sections/appearance_background.dart:1-9`、`:44` — 头注「**这个文件是怎么来的**」；拆分原因是 `appearance_section.dart` 曾 **1489 行 > ≤1000 上限**；`:44 part of 'appearance_section.dart';` ⇒ 「不是独立库」成立。

**B0506** `lib/` 115 个 `.dart`：**5 个 `part of` · 101 个有 `library` · 9 个两者都没有** ⇒ 「名实相符」要问三条（叫什么 / 能否被 import / 靠什么归属）。

**B0507** 那 9 个里 **7 个是平台配对**（`*_web.dart` / `*_stub.dart`）⇒ 靠「文件名 + import 条件」归属；`live2d_transport.dart` 靠 `///` 头注。

**B0508–B0510** `appearance_background.dart:846-850`、`:856-864`、`:908-911`、`:1006-1013` —
- 旧判据 `selected.isNotEmpty || items.length >= 2` 被点名（满 2 项后预览永不出现）；新判据 `bool _managing = false`（**模式**，非数据）。
- `didUpdateWidget` 里 `_selected` / `_expanded` **各清一次**、判据同一个 `live`、按 `backgroundItemIdentity` 而非下标。
- `_managing` **全文件唯一写入点**（`:908`）；退出时 `if (!_managing) _selected.clear();`；按钮文案「**完成**」/「**管理**」（按**目的**写，不是「退出管理模式」）。
- `_tap(int index)`：`if (_managing) { _toggle(identity); return; }`，非管理支注释「**任意库大小下可用**」；管理支**无注释**（老行为、不需论证）。

**B0511** `chat_panel.dart:354-366` — 同一条件 `(phase == thinking || phase == speaking)` **逐字写三遍**（icon / tooltip / onPressed），**无共享谓词** ⇒ B0432「三者全在同一条件下变形」**是观察、不是保证**（靠复制成立）。

**B0512** 6 句引文核验：`按钮说小可爱`=1 · `自检说谎`=11 · `两遍`=17 · `修法是换用`=0 · `一次成型`=0 · `既然它在别处`=0 ⇒ **6 句里 3 句是我自己的话** ⇒ 台账层面需「引用要么 grep、要么标注是转述」。

**B0513** `chat_panel.dart:532-538` — `_RoundActionButton` 头注「**两个必须一起成立的性质**」：① 形状恒定为圆（不成立的症状是「**那会跳一下**」）② 尺寸固定 40（比 Material 48 小一档，「**不会把输入行撑高**」）。

---

## 背景预览链（B0514–B0517）

五跳、四个文件：`appearance_background`（`part`）→ `appearance_section:226`（`onPreviewItem`）→ `appearance_section:81/154`（`onPreviewBackground`）→ `shell_settings:303`（持有）→ `shell_prefs:458 _previewBackground`（一行转调，头注「**只改运行时索引，不落盘**」）→ **`main.dart:532 _jumpBackground`**（本体）。
`main.dart:528-541` 头注「刷新后从头开始才符合直觉」+ 行内「必须**同时**推给轮播控制器（**F-0001-2**）……下一次 `onAdvance` 会从**旧的** `_index` 往前走，**把用户刚预览的那张无端切走**」。

---

## 密钥写入链（B0518–B0561，13 个位置、零逻辑）

`llm_section.dart:52/74/163`（`onSaveKey` **可选**、`Future<void> Function(String, String)`）· `tts_section.dart:117-123`（四参**逐字相同**，只差 `sectionLabel`）· `env_key_field.dart:29/41/63/107/140/150/60-86` —
- `:107 final bool canSave = !_saving && widget.onSave != null;` 只用于 **`:140 enabled: canSave`**（输入框）+ **`:150 if (canSave) _save();`**（防竞态）⇒ 同一判据两处各判一次。
- `:60-86 _send`… 实际是 `_save()`：两道 null 早退 → `_message = null` → 成功 `_value.clear()` + `value.isEmpty ? '已清除 KEY' : '已写入 KEY，立即生效（不用重启）'`；`catch` 是 **`'保存失败：$error'`**。
- ⚠ 三处 `'保存失败：$e'`（`settings_controller.dart:314`、`env_key_field.dart:85`）；同仓另两种写法（`shell_admin.dart:126` 按 `code` 分流、`main.dart:1050/1074` 消息优先码兜底）⇒ **同一判据三套形状**。
- `llm_section.dart:161-169` 守卫是 `if (envKey != null)`、**不含** `onSaveKey != null`。
- `shell_settings.dart:259/272` 两个 `onSaveKey: _saveEnvKey` ⇒ **实现唯一定义在 `shell_admin.dart:102`（跨文件）**。
- `shell_admin.dart:96-112`：头注「`.env` 是唯一密钥真源……**不该**提示『重启』或『重跑 ignite.sh』」「值**只在参数里出现**」；`await _envApi.write(...)` **不 try**（抛给 `EnvKeyField`）⇒ 抛的是 `ApiException`（B0459 核过 `toString()` 含 `code`）⇒ ⚠ B0523「没有码」**降级为「码与消息挤在一句里」**。
- `shell_admin.dart:110` `try { _envApi.list() } on ApiException { /* 写成功了但读状态失败：不动现有状态 */ }` ⇒ **两处失败刻意分开**。
- Rust 侧：`web_api/env_routes.rs:46-54 EnvKeyEntry`（`section` / `key` / `set`，**裸 `Serialize` 零注解**、注释列了第三个分区 `"performance"`）· `runtime/src/secrets.rs:7` 与 `runtime/src/settings.rs:124` **各写一次**「`GET/PUT /api/v1/env` 的唯一真源」⇒ **真源 ≠ 序列化**。
- Dart 侧 `api/env_api.dart:25-60`：`EnvKey` 头注「**没有 value 字段，这是刻意的**」；`forSection(section)` = `k.section == section && k.key.isNotEmpty`（**两种成因落到同一个 `null`**）；`envFile` 头注「排障用：告诉用户该改哪个文件」。
- `settings.rs:631/636` 对 `"performance"` 有 `validate_base_url` / `validate_env_name`；`llm_section.dart:34-43` 表演层说明（三态判据：`speak` 有值/`cues` 空 = 只说；`speak` 空/`cues` 有值 = 只动；**都是空 = `noop`**）+「表演层默认关」；`test/performance_layer_copy_test.dart:19/22/23` **两个文案各有一条逐字 `expect`**。

---

## 发送链（B0539–B0543）

`chat_panel.dart:342-350 TextField`（`minLines: 1 / maxLines: 4`、`onSubmitted: (_) => onSend()`、**无 `enabled`** ⇒ 门控在 `onSend` 里）→ `app_shell.dart:932` 透传 → **`shell_chat.dart:38 _send()`**（`if (text.trim().isEmpty) return;` · `if (_chat.streaming) return;` · `_input.clear();` ⇒ **两道早退都在清空之前** ⇒ 失败不丢用户输入）。
`main.dart:862 _sendWithCancellation` = `_cancelStageForTurnBoundary('new-message')` + `await _send()`；`_stageCancellation` 头注「**同步取消、不补帧**……不会**重放任何旧 cue**」。
⚠ `shell_chat.dart:30-37` 留着**动作子系统墓志铭**（`action_state` / `_onActionState` / `archive/action-trigger-p5`）。
⚠ 三个私有 `_send` **同形不同义**（`shell_chat.dart:38` / `live2d_bridge.dart:417`（带 `String json`）/ `memory_panel.dart:285`）。

## 播报链（B0544–B0549）

`shell_chat.dart:16-24 _syncLiveRegion`：三道早退（空 · 非 `assistant` · 非 `streaming`）→ `_live.finish(last.text)`，注释「**收口**：把最后一小段补播一次（末尾往往没有句末标点…）」。
`state/live_region.dart:70-87`：`finish` **只动 `_text`、不动 `_lastAnnounce`**（节流状态留给 `reset`）；`reset()` 两字段都清、早退条件是**两者都空**；`:63-66` 的 ≥1.5 s 门与 `_lastAnnounce = now`。
`main.dart:555 _live` · `:1019 _live.dispose()` · `:1171`「正文一变就同步播报（**节流在 `_live` 里做**）」· **`:1174-1180 ListenableBuilder(listenable: Listenable.merge([_live, _chat, _ui, _audio.unlockState, …]))`**（⚠ 注释写「**三处**」而列表**四个**）· `:1321 announcement: _live.hasAnnouncement ? _live.announcement : null`。
⇒ B0547「零 `addListener` ⇒ 无订阅者」**推论错误**（隐式订阅），已在本串中修正。

## 静音观测链（B0528–B0538、11 处）

`tts_section.dart:125-130 ReadonlyField('服务端静音', text: serverMuted ? '静音中（LIVE2D_AI_MUTE_AUDIO=1）…', description: '这是**观测值**，不是开关：它由服务端启动参数决[定]')` → `app_shell.dart:126/208/941` → `shell_settings.dart:269` → **`main.dart:350/730`**（唯一写入：`if (event is AudioEvent && event.muted != wasMuted) setState(…)`）→ **`api/ws_frame.dart:254/262 AudioEvent { this.muted = false }`** → `web_api/ws/audio.rs:102`（**句尾空块** `"audio": ""` / `"volume": 0.0` / `"end": true` + 注释「空块的 RMS 恒为 0：句尾闭嘴是**真话，不是猜测**」）与 `:136`（正常块）→ `audio.rs:32 resolve_muted(force_unmute, force_mute) = !force_unmute && force_mute`（UNMUTE 优先）→ 唯一生产调用点 **`cli_entry.rs:460-466`**（`force_unmute && force_mute` 时 `println!` 告警；`emit_bc.set_muted(…)` 后**回读** `emit_bc.is_muted()` 「确认 set/get 这条链路是通的」）。
`audio_tests.rs:157/181/184` 有断言。⚠ 那条 `println!` 冲突告警**不进 tracing sink**（记为观察）。

---

## 判据与工具层（B0511–B0512、B0566 已并入上）

- **B0511**「同一条件 vs 同一个表达式」是两件事。
- **B0512** 引文纪律：**6 句里 3 句零命中** ⇒「我引的多数是自己的话」被量化。
- **B0566** `render_events.dart:186-194`：段头注（纯逻辑、不 import Flutter、hub 住 director_observer、缓冲只住内存）+ `kObserverBufferCapacity = 200` + 注释「阶段5 §2 **B 栏冻结**」+ `:194` 「『生效』侧的两条渲染面 ack（**C 栏配对只看这两条**）」。

## 未回填为结论的项（照原样保留「未核」）
C 栏两列如何配对 · `kObserverBufferCapacity` 是否有第二处注释 · `blurExceptionFile` 的 `:124` 断言形状之外的三态布尔测试 · `director_observer_test.dart` 是否绕过门控 · `clean_transcript` 尾部（`:270` 之后）· `lower_eq` / `is_separator` 本体 · `wake_gate_open` 本体 · `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 · 永久不可核 4 条。
