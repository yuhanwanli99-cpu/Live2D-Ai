# FINDINGS（去重后的全部发现）

> 格式：ID F-批次-序号 / 等级 / 路径+行号或符号 / 摘录 / 调用链(P0P1) / 影响 / 改法方向 / 验证步骤 / 置信度 / 反向证据(P0P1)
> 已登记项（任务书 §8）不在此重复。

## BATCH-0001（Phase 1 · main.dart + lib/app/）

### F-0001-1 · P1 · 错误横幅「去 LLM/语音合成设置」在面板关闭态不打开面板（默认态点了没反应）
- 位置：lib/ui/error_actions.dart:27-31（goto 只调 onGoto）；lib/main.dart:1163-1170（onGoto=_gotoSection）；lib/main.dart:1004-1014（_gotoSection 只 setState(_section)+_ensureSettingsLoaded）
- 摘录：
  ```dart
  ErrorAction goto(SettingsSection section, String label) => ErrorAction(
    label: label,
    onPressed: () => onGoto(section),
  );
  ```
- 调用链：WS error 帧 → UiStateTracker.errorMessage/errorCode → main.dart:1163 `errorActionsFor(..., onGoto: _gotoSection)` → chat_panel.dart:227 `actions: errorActions` → 用户点「去 LLM 设置」→ main.dart:1004 `_gotoSection`（切 _section+清一次性结果+懒加载）——**没有任何一处调用** `AppShellState.openSettings()`（main.dart 全文件仅 :1230 快捷键 onOpenSettings 调用过）。
- 影响：用户可见。llm_upstream_401 等错误后，横幅承诺的出路在**默认状态（面板关闭）**下只改了看不见的状态；expanded/medium/compact 三种宿主全中。错误「有出路」变成假出路。
- 改法方向：main.dart 接线处 onGoto 包一层——`_gotoSection(s); unawaited(_shellKey.currentState?.openSettings());`（或在 errorActionsFor 增加 onOpen 回调由组合根注入）。
- 验证：跑服务，把 LLM key 清掉触发 401 → 横幅出现（设置面板未开）→ 点「去 LLM 设置」→ 面板未弹出即复现。可加 widget 测试：注入 errorActionsFor(onGoto:) 断言宿主 openSettings 被调。
- 置信度：高。
- 反向证据：grep `openSettings`（lib/main.dart、chat_panel.dart、error_banner.dart）仅快捷键一处；test/*.dart 中 `errorActionsFor|ErrorAction|去 LLM` **零命中**（该路径无测试覆盖，也无人钉过「先开面板」的设计）；chat_panel.dart:227 直接把 actions 传给 banner，无包装。

### F-0001-2 · P2 · 轮播双索引漂移：预览跳转不驱动 ShellSlideshow，删/清空库不重置运行时索引
- 位置：lib/main.dart:439-442（_jumpBackground 只 setState）；lib/app/shell_prefs.dart:435-438（_previewBackground→_jumpBackground）；lib/ui/shell_slideshow.dart:63-66（jumpTo 定义，全仓零调用）；lib/app/app_shell.dart:552（渲染用 clamp 兜底）
- 摘录：
  ```dart
  void _previewBackground(int index) {
    _jumpBackground(index);
  }
  ```
  （而 `_slideshow.jumpTo` 仅 test/background_logic_test.dart:206 调用过——生产路径没人按。）
- 调用链：点缩略图 → `_previewBackground` → `_backgroundIndex=N`（壳画第 N 张）；轮播定时器仍从 `ShellSlideshow._index=M` 推进 → `onAdvance` 回写 `_backgroundIndex=next(M)` → 用户刚从 N 张的预览被无端切到 next(M)。第二形态：`_clearShellImage`(库清成 0 项，setLibrary 把 _index 归 0) 后再加图，`_backgroundIndex` 仍是旧值（渲染 clamp），首轮 advance 前显示错项——与「从头开始」直觉不符。
- 影响：用户可见（轮播行为与预览选择互相打架）；维度 A「运行时索引唯一真源」被两份索引破坏。
- 改法方向：`_jumpBackground` 内同步 `_slideshow.jumpTo(index)`；库清空/减项时对 `_backgroundIndex` 做同一夹持（或改由 slideshow.index 单源读取）。
- 验证：VM 测试——泵 ShellSlideshow(2 项,间隔 1s)+调用 main 的 _jumpBackground(1) 语义：断言下一次 onAdvance 来自 1 而非 0→1；现状会给出 next(0)=1 之外序列（顺序轮播时肉眼即分得出：预览第 3 张 → 1 个间隔后跳到第 2 张）。人工路径：库≥2、间隔>0、来源=库，点第 4 张缩略图，等一个间隔。
- 置信度：高（jumpTo 零调用为 grep 实测）。
- 反向证据：grep `jumpTo`（lib/ 全量）——仅定义处；grep `_backgroundIndex` 复位逻辑——仅 `didUpdateWidget` 触发 syncSlideshow（不重设 _backgroundIndex）；AppShell clamp 挡崩溃但不修语义。

### F-0001-3 · P2 · 启动背景水合竞态：hydrate 完成时用旧 prefs 快照整体覆盖，吞掉窗口期用户改动
- 位置：lib/main.dart:165-176（_hydrateBackgrounds）
- 摘录：
  ```dart
  final BackgroundHydration hydrated = await _hydrateSafely(store, _prefs);
  if (!mounted) return;
  setState(() { _store = store; _prefs = hydrated.prefs; });
  if (hydrated.migrated) saveDisplayPrefs(hydrated.prefs);
  ```
- 调用链：initState 启动 `_hydrateBackgrounds`（await `_openStore` ≤3s + `_hydrateSafely` ≤3s，见 :148/:178-196）→ 期间用户经 `_update`(:202) 改主题/音量并已落盘 → 水合返回，`hydrated.prefs` 派生自 **initState 时刻**的 `_prefs` → setState 整体替换（用户改动回退）；若 `migrated=true` 还会把这份旧对象 `saveDisplayPrefs` **写回 localStorage**（磁盘回退）。
- 影响：数据（本机偏好静默回退）。窗口 0–6s、概率低（启动即改偏好才触发），但后果与 rc.5 P0 系列同类（偏好丢失），值得低成本修掉。
- 改法方向：水合回来后基于**当前** `_prefs` 重放（或 hydrateBackgrounds 改为纯函数返回「孤儿摘除清单+migrated 标记」，由调用方对最新 prefs 应用）。
- 验证：dart 层可测——把 _hydrateSafely 的 store 换成受控 fake：水合 await 中先调 `_update(改音量)`，水合完成后断言音量未回退、localStorage 序列化串含新音量。
- 置信度：高（纯代码推导；触发条件=水合窗口内有偏好提交）。
- 反向证据：找过 mounted/epoch 类守卫——`_update` 无代际号，水合路径无 epoch 比对（与 _resultEpoch 不同族）；找过 hydrateBackgrounds 内部 merge 逻辑（data/background_hydration.dart:164 行返回整份 prefs）——无「与当前值合并」步骤。

### F-0001-4 · P2 · 舞台加载进度永不刷新：bridge 进度通知无人接线，宿主只在自身重建时读一次快照
- 位置：lib/main.dart:1213（`stageProgress: _stageKey.currentState?.bridge?.progress`）；lib/live2d/live2d_bridge.dart:430-434（progress 分支 notifyListeners）；lib/live2d/live2d_stage.dart:301-326（_onBridgeChanged 只转发 phase/error/ack/ready）；lib/ui/stage_host.dart:91-94（覆盖层读 widget.progress）
- 摘录：
  ```dart
  case 'progress':
    final value = payload['progress'];
    if (value is num) { _progress = value.toDouble(); notifyListeners(); }
  ```
  （live2d_stage.dart 内 `bridge.addListener(_onBridgeChanged)`，但 `_onBridgeChanged` 里**没有** progress 的转发分支。）
- 调用链：渲染面 progress 消息 → bridge.notifyListeners → Live2DStageState._onBridgeChanged（无动作）→ 宿主不重建 → StageHost 收到的 `progress` 是宿主**上次**重建时的 GlobalKey 快照 → 「模型加载中… 40%」可长期冻住，进度条不推进。
- 影响：用户可见（误导性进度）；不崩不丢数据。维度 D/B（事件流断链）。
- 改法方向：Live2DStage 增 `onProgress` 回调（或直接暴露 `ValueListenable<double?>`），宿主 setState 接上；或 StageHost 覆盖层自己监听 bridge。
- 验证：真机点火时断网限速模型加载可肉眼看到冻住的百分比；或 widget 测试以假 bridge 播 progress 帧、断言 StageHost 的 LinearProgressIndicator.value 更新。
- 置信度：高。
- 反向证据：grep `addListener|ListenableBuilder`（live2d_stage.dart、stage_host.dart）——除 _pushStageColor/桥自身监听外无进度订阅；grep `bridge` 外部使用——全仓仅 main.dart:1213 一处；test 无进度推进断言。

### F-0001-5 · P3 · AppShell.onBackgroundIndex/onBackgroundJump 死参数 + 头注指向不存在的 ShellBackgroundHost
- 位置：lib/app/app_shell.dart:104-105,184-188（声明与文档）；main.dart 的 AppShell(...) 未传；全仓 grep 零使用；app_shell.dart:176-178「索引与定时器住在 [ShellBackgroundHost]」——该类不存在（实际住 _ShellRootState）。
- 摘录：
  ```dart
  /// 背景库变化 / 轮播前进 / 手动点选时回调（带新的索引）。
  final ValueChanged<int>? onBackgroundIndex;
  ```
- 影响：可维护性（误导下一位改背景域的人以为存在第四种索引通路）。
- 改法方向：删两参数与注释，或真把索引下移（配合 F-0001-2 的单源化一并做）。
- 验证：`grep -rn "onBackgroundIndex\|onBackgroundJump" lib/ test/` 仅 app_shell.dart 自身命中；`grep -rn "ShellBackgroundHost" lib/` 仅注释命中。
- 置信度：高。
- 反向证据：grep 两符号含 test 全量——无。

## BATCH-0002（Phase 1 · display_prefs + data/ 存储域）

### F-0002-1 · P1 · 每次偏好变更全量重发 stage-bg 数据串 + 全量落盘：音量滑杆每帧触发一遍（有舞台图=性能悬崖）
- 位置：lib/app/shell_prefs.dart:228（_updatePrefs→_applyPrefs）、:99-102（sendStageBg 无条件）；lib/live2d/live2d_bridge.dart（sendStageBg→_enqueueOrSend，无去重）；lib/main.dart:205（saveDisplayPrefs 全量）+ :1199（音量滑杆直通 _updatePrefs）；lib/ui/audio_bar.dart:104（Slider onChanged 每帧）
- 摘录：
  ```dart
  // shell_prefs.dart:100
  unawaited(
    _stageKey.currentState?.sendStageBg(prefs.stageImage
  // main.dart:205
  return saveDisplayPrefs(next);   // ← jsonEncode(整份 prefs 含 ≤1.5M 字符 stageImage)+setItem
  ```
- 调用链：音量 Slider.onChanged（拖拽每帧）→ main.dart:1199 `_updatePrefs(copyWith(volume))` → ①`widget.onPrefsChanged`→根 setState（全壳重建）②`_applyPrefs(next)`→`sendStageBg(prefs.stageImage)`→bridge `_enqueueOrSend('stage-bg', {dataUrl:≤1.5M字符})`→postMessage 整串跨 iframe→渲染面 base64→Blob→createImageBitmap→write_texture（rc.5 §⑦ 解码路径，每张重跑）③`saveDisplayPrefs` 把含 stageImage（与存量 stagePlaylist，总预算可到 ~3M 字符）的整份 prefs jsonEncode+同步 setItem。
- 影响：性能（拖音量/透明度/任何滑杆时主线程同步 MB 级序列化+同步 localStorage 写+每帧 iframe 大串拷贝+渲染面重解码纹理）；「设了舞台壁纸的用户一拖滑杆就卡」与产品核心场景（有背景是常态）直接冲突。2026-09-27 把背景字节搬出 prefs 时明确为了消灭「拖一次音量滑杆重写所有图片字节」（background_store.dart 头注第 3 条），**但 stageImage/stagePlaylist 两条旧通道原样保留**，同族问题只治了一半。
- 改法方向：①_applyPrefs 只在 `prefs.stageImage` **变了**时才 sendStageBg（比对 lastSent，sync 同理字段级）；②滑杆类落盘改 onChangeEnd 或防抖；③终态：stageImage 也走字节库（与 backgrounds 同机制），prefs 只存 id。
- 验证：widget 测试——注入假 transport 计 stage-bg 帧数：`_updatePrefs(volume:+0.01)` 两次，断言 stage-bg 增量==0（现状==2）；真机：装一张 ~1MB 舞台图，DevTools Performance 拖音量看每帧 long task 与 iframe 网络/解码活动。
- 置信度：高（Flutter 侧三处链路全读实；渲染面重解码按 rc.5 文档记账，标 部分未核实）。
- 反向证据：查过 _enqueueOrSend 有无同值去重/合并（无，只有 mouth 帧清理与 maxQueue 截断）；查过滑杆是否 onChangeEnd 才提交（audio_bar:104 直连 onChanged）；查过 stageImage 是否已搬出 prefs（仍在，budget 1.5M 字符，:61-68）；查过 saveDisplayPrefs 有无防抖（无）。

### F-0002-2 · P2 · 启动 store 置换窗口：早期加图写进占位内存库，「已记住」是假话，下次启动图被判孤儿摘除
- 位置：lib/main.dart:155（_store 初始=MemoryBackgroundStore 占位）、:165-176（_hydrateBackgrounds 完成才 setState 换真 store）；lib/app/shell_prefs.dart:330/408（storeBackground/forgetBackground 用 `widget.store` 快照）；lib/data/background_hydration.dart:114-119（missing→摘项）
- 摘录：
  ```dart
  // main.dart:155
  BackgroundStore _store = MemoryBackgroundStore();
  // :174 —— 要到水合完成才换真
  setState(() { _store = store; _prefs = hydrated.prefs; });
  ```
- 调用链：启动后 0–3s（hydrate 至多 3s，_openStore 内 _open 至多再叠 3s 串行）内用户加图 → `_pickImage` 走 `widget.store`＝**内存占位** → `storeBackground` 成功 → UI「已加进背景库（已记住）」+ prefs 存 id → 水合完成换入 IDB 实例，**内存里的字节无任何搬运** → 下次启动该 id 在 IDB `missing` → hydrate 摘项（设计如此）→ 图丢。删除路径同窗：forgetBackground 删的是占位库，真库字节暂成孤儿（下次启动 retainOnly 回收，无数据损失只有窗口错报）。
- 影响：数据（丢用户图）+ 误导文案。窗口小（≤3–6s）但确定可达（启动后立刻验证壁纸是常见手顺）。F-0001-3 是「旧 prefs 覆盖」，本条是「字节进错库」——同一竞态窗口的第二形态。
- 改法方向：_store 在 initState 立即换成真 store（createBackgroundStore 本就是同步构造，open 惰性在实现内部）——占位库在水合前不该暴露给 UI；或水合前禁用选图控件并说明「背景库对接中」。
- 验证：VM 可测——把 main 的初始 _store 换成可断言的 fake：在 _hydrateBackgrounds 未完成时调 _pickImage 路径（受控注入 dataUrl），断言 put 落在真 store 而不是占位。真机复现慢（需 3s 内点完）可先做静态：grep `widget.store` 调用点是否全部在置换之后被守护（现在没有守护）。
- 置信度：高（代码级确定；实际命中概率=人为窗口操作，标 中）。
- 反向证据：找过「占位→真 store 的字节搬运」——main.dart 无；找过 UI 在水合前的禁用态——appearance 分区无 _storeReady 门；查过 createBackgroundStore 是否 await open（同步返回句柄，open 惰性）——即 main 根本不必等水合才换 store，占位暴露纯属接线顺序。

### F-0002-3 · P3 · main._openStore 的超时/退路在 web 上是死代码，注释与真实兜底位置不符
- 位置：lib/main.dart:147-152；lib/data/background_store_web.dart:66（createBackgroundStore 同步构造，不 open、不抛）
- 摘录：
  ```dart
  try {
    return await Future<BackgroundStore>.value(createBackgroundStore())
      .timeout(const Duration(milliseconds: 3000));
  ```
- 影响：可维护性——`Future.value(...).timeout(3s)` 永远立即完成，catch 只挡同步抛（web 实现不抛）；注释承诺「3s 挂死→退路内存库」在线上不可能发生（真实兜底是 _IdbBackgroundStore 内部把 open 失败缓存为 null 后全操作诚实失败——行为安全但和这条注释说的不是一回事）。误导后来者以为存在内存退路切换。
- 改法方向：删掉这层假兜底（构造即成功是事实），或把 timeout 挪到第一次 `probe()` 上并把退路切换真的做出来（连带 F-0002-2 一起重排接线）。
- 验证：`grep -n "createBackgroundStore" lib/data/background_store_web.dart`——返回 `=> _IdbBackgroundStore()` 无异步；main 的 catch 分支在 web 构建里不可达（flutter test 里走 stub 同样立即完成）。
- 置信度：高。
- 反向证据：查过 web 版构造是否可能同步抛（纯 new，无）；查过是否有别的路径把 _store 换回内存（仅测试 seed）。

## BATCH-0003（Phase 1 · settings/sections/）

### F-0003-1 · P1 · 壳背景图每次宿主重建全量重解码：base64Decode 重跑 + MemoryImage 恒 cache-miss
- 位置：lib/ui/shell_backdrop.dart:181-189（_imageLayer）；lib/app/app_shell.dart:510/523-524（ShellBackdropHost 在每次 build 里重建）；lib/main.dart:1148-1155（ListenableBuilder listen=_live/_chat/_ui/_settings/_voiceListen/解锁）；lib/ui/shell_backdrop.dart:47-59（decodeDataUrlBytes 无缓存）；SDK 证据：~/flutter/packages/flutter/lib/src/painting/image_provider.dart:1725-1735（MemoryImage ==/hashCode 按 **bytes 对象身份**）
- 摘录：
  ```dart
  Widget _imageLayer(String dataUrl) {
    final Uint8List? bytes = decodeDataUrlBytes(dataUrl);   // 每次 build 全量 base64Decode
    ...
    child: Image.memory(bytes, ..., gaplessPlayback: true,  // 新 Uint8List ⇒ 新 cache key
  ```
- 调用链：WS text_delta → ChatController.notifyListeners → main.dart ListenableBuilder 重建 AppShell（设计注释自认「会被 WS 事件频繁重建」）→ ShellBackdropHost→ShellBackdrop.build → _imageLayer：MB 级 base64Decode → Image.memory(新 bytes) → MemoryImage key 按 bytes.hashCode(身份) → ImageCache 恒 miss → **全尺寸壁纸重解码**，流式期间帧率级重复。同族第二处：appearance_section.dart:1131-1148 缩略图（库开屏时 ≤8 张每帧重解码，96px 但源串照常全量 base64 解码）。
- 影响：性能（设了壁纸 + 收到流式回复 = 稳定掉帧；与 wasm 渲染面抢主线程）；「资产链路在用」的常态路径。接近红线 O 的精神（高频信号的重活不该进 Widget 树）。
- 改法方向：按 dataUrl 身份备忘化解码结果（Map<String,Uint8List> 或 item.id→bytes 小缓存，水合时本来就有稳定 id 可用）；或自定义 ImageProvider 以 id 为 cacheKey。
- 验证：VM 确定性自证：`final a=decodeDataUrlBytes(u)!; final b=decodeDataUrlBytes(u)!; expect(MemoryImage(a)==MemoryImage(b), isFalse);`（恒 miss 的最小证明）；widget 性能验证：pump 含壁纸的 AppShell 后 bump chat 通知 N 次，以 debug 计数或 `debugPrint` 插桩（只读复现时临时本地看，不入库）看 base64Decode 重复；真机：设 1080p 壁纸→发消息→Performance 面板看逐帧 DecodeImage task。
- 置信度：高（miss 机制=SDK 源码级；帧率×代价=由 ListenableBuilder 接线推得，幅度未真机量，标部分推断）。
- 反向证据：找过解码缓存/_decoded memoize——无（grep cache/decoded）；RepaintBoundary(:162) 只免重绘不免 didUpdateWidget→resolve；gaplessPlayback 只防闪烁不防重解码；Image 同 Element 复用仍走 provider 比较（key=身份）→ 必 miss；no_backdrop_filter_test/theme 测试均不涉及。

### F-0003-2 · P1 · 「连通性自检」成功结果永不显示；resultIsError 用显示文案猜错误类型
- 位置：lib/ui/field_row.dart（FieldActionRow.build：`error: resultIsError ? result : null` → _FieldShell 仅 hasError 时渲染 InlineNotice）；lib/settings/sections/llm_section.dart:171-184、tts_section.dart:161-180（文案承诺「结果内联显示」+ `contains('ok'/'毫秒'/'ms')` 启发式）；lib/main.dart:992-1002（_testLlm/_testTts 只落 result 字符串，无 code 通道）
- 摘录：
  ```dart
  description: '只做连通性，不会真的生成回复；结果内联显示',   // llm_section:175
  ...
  error: resultIsError ? result : null,                      // field_row FieldActionRow
  ```
- 调用链：点「测试连接」→ _testLlm → outcome.ok → `_llmTestResult = outcome.message`（含 "ms"）→ resultIsError=false → `error:null` → _FieldShell 什么都不渲染——**成功唯一反馈是按钮 busy 一闪**；失败态反例：若成功文案改写为不含 ok/毫秒/ms（文案会改、码才是契约——AGENTS 自家红线），成功会被标红。两个方向都错。
- 影响：用户可见（成功=无反馈，用户反复点/以为坏了）；契约面（前端从显示文案猜错误类型，正是项目明令禁止的模式）。
- 改法方向：main 接线传结构化 `(ok, code, message)`，FieldActionRow 增设成功槽（InlineNotice info 档），resultIsError 由 ok/code 决定而不是扫文案。
- 验证：widget 测试（现状必红）：给 FieldActionRow `result:'连接正常（136 ms）', resultIsError:false` → 断言界面出现该文案（现状 findsNothing）；手测：LLM 正常时点自检 → 只见按钮闪、无任何结果行。
- 置信度：高。
- 反向证据：grep test：field_row_test 只测 busy 与「错误走错误槽」（:434-479 两条），无成功显示用例；grep main：自检无 SnackBar/notice 旁路（只有 _adminMessage 那类才进横幅）；asset_guard_tts_config 只钉「点一次回调一次」与「无试听」。设计意图按文档「内联结果行（ok/latency_ms/error）」=成功应显示。

### F-0003-3 · P2 · 背景库批量删除按「下标」选删，重排后不重对齐 → 删错图
- 位置：lib/settings/sections/appearance_section.dart:832-835（`_selected` 存下标 + 自相矛盾注释）、:1003-1025（勾选复选框按 index）、:974-978（删除所选→widget.onRemoveMany(selected.toList())→shell_prefs._removeBackgrounds）、onReorderItem→prefs 重排（下标语义已变而 _selected 原封不动）
- 摘录：
  ```dart
  /// 选中的下标集合。**存下标不存项**：重排之后下标会变，
  /// 存项才能在重排后仍然指对东西——所以每次操作前按当前顺序重新解释。
  final Set<int> _selected = <int>{};
  ```
  （注释论证的是「存项才对」，实现却存下标，且没有任何「操作前重新解释」步骤。）
- 调用链：勾选中第 0 项 → 把原第 2 项拖到第 0 位（onReorder→prefs 重排，_selected={0} 不变）→ 复选框现在落在**原第 2 项**上 → 「删除所选」→ _removeBackgrounds([0]) → forgetBackground + prefs 摘除 → 删掉的不是用户勾选的那张。
- 影响：数据（删错用户资产，需重新导入）；误导型 UI（勾选状态说谎）。
- 改法方向：_selected 改存 BackgroundItem（id）集合，或 onReorder 后按「项跟随位移」重映射下标 / 直接清空选择并提示。
- 验证：widget 测试：3 项库→勾选 0→模拟重排（调 onReorderItem(2,0)）→ 点删除所选 → 断言被删的是原勾选那张（现状删错位）。
- 置信度：高。
- 反向证据：grep _selected 的清理点——只有 _exitManage（手动退出）；onReorder 回调链（:884-889）无任何清理/重映射；appearance_background_library_test 未见对「重排后批量删」的组合用例。

### F-0003-4 · P2 · EnvKeyField 成功消息的 Markdown 星号必上屏（绕过裸 Text 扫描）
- 位置：lib/settings/sections/env_key_field.dart:79（`'已写入 ${status.key}，**立即生效**（不用重启）'`）→ :169-179 `Text(_message!)` 裸渲染；扫描盲区：test/visual_language_test.dart 只扫 `Text(` **字面实参**里的 `**`，本条经 `_message` 变量中转。
- 影响：用户可见（保存密钥后必见两颗星）；rc.3 修过的「15 处露星号」同族回归从**变量通道**溜回来。
- 改法方向：_message 去星号或该行换 EmphasizedText；扫描规则加一条「赋值给消息变量的字符串字面量含 **」粗筛（或 lint 掉 `_message =` 右侧星号）。
- 验证：widget 测试：泵 EnvKeyField+注入成功 save 回调 → 断言渲染文本不含 '**'（现状红）。
- 置信度：高。
- 反向证据：EmphasizedText 消费者列表（field_row/section_header/group_card/appearance/dev_tools）不含 env_key_field；窗口扫描器实读确认按 `Text(` 括号配对取实参。

### F-0003-6 · P2 · _ModConfigTile：config 对象身份触发 re-seed 吞草稿；保存结果离开分区即丢
- 位置：lib/settings/sections/dev_tools_section.dart:654-662（didUpdateWidget `!identical(old.config,new)` → _values 重灌、message 清掉）；:630-633（注释自己承认「面板重建会丢掉——要常驻得把状态提到 tile 外」）；触发器：shell_admin._saveModConfig/_notifyModChanged 后 `_loadAdmin()` 重取列表（新 Map 实例同内容）+ 分区切换（pane 子树卸载重建）
- 影响：数据输入（填了一半的表单被静默重置）；反馈丢失（保存失败带错误码的文案在切走后消失，与「界面要有可搜的码」契约相悖的短暂性）。
- 改法方向：didUpdateWidget 用内容相等（Map == 浅比较即可）而不是 identical；保存消息上移到宿主（同 transient results 的 epoch 族机制）。
- 验证：widget 测试：泵 _ModConfigTile → 改一个字段 → 用**同内容新实例**的 mod 重泵 → 断言字段仍是用户改的值（现状被重灌回 remote）。
- 置信度：高（re-seed 分支代码级确定；触发概率中等——需要保存/他处刷新恰逢填写中）。
- 反向证据：找过草稿外置（宿主只存 `_adminMessage`，Mod 表单值只在 tile state）；找过内容比较守卫——无。

### F-0003-5 · P3 · 动作调试块「归零」不清面板侧基础表情状态
- 位置：dev_tools_section.dart:1780-1788（动作块 none 按钮只 `_applyExpression('none')`，不调 `_setBaseExpression('none')`）对比 :1713-1722（表情块清两者）
- 影响：开发者面板内部状态与渲染面分叉（归零后「当前基础表情」仍显示旧值、叠开时 ttl 内会把手势+旧表情再发一次）——dev-only。
- 验证：widget 测试：设基础表情→到「动作调试」点归零→表情块仍显示旧 id（现状）。
- 置信度：高。反向证据：注释只解释了 sweep 不碰基础表情，没有解释归零按钮为何不同步。

## BATCH-0004（Phase 1 · Mod 面板域 + settings_controller + settings_sections）

### F-0004-1 · P2 · 记忆面板列表不随「会话桶」切换刷新：切会话后文案说桶 B、列表仍是桶 A，行内编辑/删除变成跨桶 id 操作
- 位置：lib/settings/mods/memory_panel.dart:220-242（`_MemoryPanelBodyState` 只有 initState 拉列表，**无 didUpdateWidget**）；:244-281 `_refreshList` 是唯一取数路径（:255 读 `widget.ctx.activeSessionId`）；:576-582 桶提示行在 build 里**现读** `ctx.activeSessionId`；:651-653 列表直接渲染陈旧 `_records`；:389/:420/:498 编辑/删除/回滚时又现读 `widget.ctx.activeSessionId`
- 摘录：
  ```dart
  @override
  void initState() {
    super.initState();
    // 进入面板就拉一次列表；不在 initState 里 setState（首帧还没建）。
    unawaited(_refreshList(initial: true));
  }
  ```
  （整个 class 到 :694 结束无 `didUpdateWidget`；`grep -n didUpdateWidget lib/settings/mods/memory_panel.dart` 零命中。）
- 调用链：`shell_settings.dart:355 activeSessionId: _chat.sessions.activeId`（活值，随会话切换变）→ `dev_tools_section.dart:960-970` 每次 build 新建 `ModPanelContext` → `MemoryPanel.build` → `_MemoryPanelBody(ctx)`；State 因 widget 类型/位置未变而**复用**，`_records` 停在旧桶 → 桶提示行立刻改口（:577 现读）而列表不换 → 用户在桶 B 的界面点「删除」→ `_delete`(:396) 取行内 `record['id']`（桶 A 的 id）+ `_send('delete', memoryCommandArgs(sessionId: <桶 B>))`(:419-422) → 跨桶删除
- 影响：数据（可能误删另一会话的记忆条目）+ 用户可见（界面自相矛盾：提示行说 B、列表是 A）
- 改法方向：给 `_MemoryPanelBodyState` 加 `didUpdateWidget`，比较 `oldWidget.ctx.activeSessionId != widget.ctx.activeSessionId` 时 `unawaited(_refreshList())`（或把桶 id 做成 `ValueKey` 强制重建）；另一种更稳的收口是**所有动作参数用列表所属的 `_bucketSessionId` 快照**而不是现读 ctx——即使刷新有竞态也不会跨桶操作
- 验证：widget 测试——构造 ctx(activeSessionId:'A') 泵面板（fake onCommand 返回两条记录）→ 用 ctx(activeSessionId:'B') 重泵同一 State → `await tester.pump()` 后断言发过第二次 `list` 命令且 `args['session_id']=='B'`（现状：无第二次 list，列表仍是 A）；第二段断言：切桶后点第一条的删除键 → 断言命令 args 的 session_id 与该记录所属桶一致（现状发 B）
- 置信度：高（缺 didUpdateWidget 为 grep 实测；跨桶删除为代码级可达路径）
- 反向证据：查过 ModPanelContext 是否被 ValueKey 强制重建（dev_tools_section.dart:958-971 无 Key，且 ModsSection 卡片/ExpansionTile 路径无 activeSessionId 相关 Key）；查过是否有别的机制触发重取（`ctx.onRefreshState` 只由 `_send` 成功链与宿主「刷新运行态」按钮调用，均与切会话无关）；查过宿主是否在切会话时整棵设置子树重建（shell_settings.dart 是 `_ShellRootState` 的 extension，随 shell 重建但 pane 保存在同一 State 里，`_MemoryPanelBody` 的 State 仍复用）；test/memory_panel_test.dart 无「切桶后列表刷新」用例。

### F-0004-2 · P3 · 记忆面板输入反馈两瑕疵：导入失败仍清空输入框；busy 期间后续动作静默吞
- 位置：lib/settings/mods/memory_panel.dart:325-344（`_import` 无条件 `if (mounted) _importController.clear();`，而 `_send` 自己吞掉失败只写 `_message`）；:291（`if (_busy) return;` 无反馈）
- 摘录：
  ```dart
    await _send('导入', 'import', ...);
    if (mounted) _importController.clear();
  ```
- 影响：用户可见/数据（失败后草稿丢失，违项目「失败保草稿」家法——与 F-0003-1 settings_controller 的失败保草稿是同一纪律的例外点）；busy 期间点击静默无响应
- 改法方向：`_send` 返回 bool（成功 true），`_import` 只在成功时 clear；busy 早退时把按钮置 `onPressed: null`（`canAct` 已含 `!_busy`，但对话框确认后的二次点击仍走这里）
- 验证：widget 测试：fake onCommand 抛 ApiException → 导入一条 → 断言 `memory-import-field` 的 TextField 文本**非空**（现状空）
- 置信度：高。反向证据：查过 `_send` 是否把结果回传给调用方（返回 `Future<void>`，无）；查过是否有别的清空路径（无）。

### F-0004-3 · P3 · mod_panels.dart 注释里 sed 残留 `@@final@@`
- 位置：lib/settings/mods/mod_panels.dart:17
- 摘录：`函数 tear-off 在不同目标上是不同的常量，故这里用 @@final@@。`（应为 `final`）
- 影响：无功能影响；注释误导
- 验证：`grep -n "@@" lib/settings/mods/*.dart`
- 置信度：高。反向证据：全仓扫 `@@` 仅此一处（非批量 sed 未清干净的普遍现象）。

## BATCH-0005（Phase 1 · lib/ui/ 上半：聊天与消息呈现面 + 共享组件族）

### F-0005-1 · P1 · 「舞台被 RepaintBoundary 包住」这条红线守卫是空转断言 ×2，命中的是一条**说「不要这么做」的注释**
- 位置：test/no_backdrop_filter_test.dart:246-252；test/semantics_test.dart:310-313；被扫对象 lib/app/app_shell.dart（**全文件只有第 592 行出现该词，且是注释**）；真正的包裹在 lib/ui/stage_host.dart:68
- 摘录（断言）：
  ```dart
  test('`AppShell` 里舞台外层有 RepaintBoundary', () {
    // 30 Hz 的口型如果让整棵聊天列表跟着重绘，长会话下会明显掉帧。
    final String source = File('lib/app/app_shell.dart').readAsStringSync();
    expect(source.contains('RepaintBoundary'), isTrue);
  });
  ```
  摘录（被命中的那一行，app_shell.dart:591-592）：
  ```dart
  // **必须走 `StageHost`**：加载/错误覆盖层与舞台语义都在它里面。
  // 直接铺 `RepaintBoundary(child: widget.stage)` 会让那些东西全部失效。
  ```
- 调用链：两条测试 → `grep -n RepaintBoundary lib/app/app_shell.dart` → **唯一命中是注释** → 真实保护在 `lib/ui/stage_host.dart:68 return RepaintBoundary(child: FocusTraversalGroup(...))`（O/P 红线：30 Hz 口型不进 Widget 树的兜底）——删掉这一行，两条测试**仍然全绿**
- 影响：红线 O/P 的回归防护形同虚设；且这两条测试互为重复，覆盖的却是**错误**的文件（真正的 host 在 ui/stage_host.dart）。测试名与断言对象不一致，未来谁按名字去改 stage_host 会找不到对应断言。
- 改法方向：断言对象换成 `lib/ui/stage_host.dart`，并且断言「`RepaintBoundary(` 出现在 `Widget build(` 的返回值上」而不是全文 contains（用剥注释的词法器，项目里已有现成实现：test/chat_bubble_labels_test.dart:129-173 的 `_stripComments`）；删掉 app_shell 侧那条重复断言。
- 验证：把 `lib/ui/stage_host.dart:68` 的 `RepaintBoundary(` 改成 `FocusTraversalGroup(` 跑 `flutter test test/no_backdrop_filter_test.dart test/semantics_test.dart`——现状**两条都通过**（这就是反例本身）；改完断言后必须变红。
- 置信度：高（grep 实测：app_shell.dart 内 RepaintBoundary 仅 592 一处且为注释）。
- 反向证据：查过是否有第三条测试真正断言 stage_host 的 RepaintBoundary（`grep -rn RepaintBoundary test/` 只有这两处）；查过 `stage_keepalive_test.dart` 是否顺带覆盖（它只数 initState，不看 RepaintBoundary）；查过 `asset_guard_*` 是否覆盖（无）。

### F-0005-2 · P1 · 每个 `text_delta` 重建整棵 AppShell 子树，**包括常驻树内、处于折叠态的设置分区**（项目自述该信号是「毫秒级」）
- 位置：lib/chat/chat_controller.dart:341-344（`_appendDelta` 末尾无条件 `notifyListeners()`）；lib/main.dart:1072-1081（`ListenableBuilder(listenable: Listenable.merge([_live,_chat,_ui,…]))` → builder 里 return `AppShell`）；lib/app/app_shell.dart:638-645（`if (inlineSettings) … child: _settingsSurface(...)` —— **在 AppShell.build 里即时求值**）；lib/app/app_shell.dart:508-531（`_settingsSurface` = `ListenableBuilder(settingsChanges)` → `ValueListenableBuilder(sectionNotifier)` → `SettingsScaffold(child: widget.sectionBuilder(context, section))`）；SDK 证据 `~/flutter/packages/flutter/lib/src/widgets/transitions.dart:111-137`（`ListenableBuilder extends AnimatedWidget`，`_AnimatedState.didUpdateWidget` **只换监听、不跳过 rebuild**）
- 摘录：
  ```dart
  void _appendDelta(String text, int? epoch) {
    …
    bubble.text += text;
    _assistant = bubble;
    _streaming = true;
    notifyListeners();     // ← 每个 delta 一次
  }
  ```
  频率自述（四处同源）：lib/ui/message_bubble.dart:47-49「`text_delta` 是毫秒级的」；lib/state/live_region.dart:5「一个 SSE chunk 一个 delta」；lib/main.dart:589「逐条写会把主线程拖垮」
- 调用链：WS `text_delta` 帧 → `ws_frame` 解析 → `ChatController._onWsEvent` → `_appendDelta`（:240-243 派发）→ `notifyListeners()` → main.dart:1072 外层 `ListenableBuilder.setState` → `AppShell.build()` → **`:643 _settingsSurface(...)` 被重新求值**（内层 `ListenableBuilder` 收到新闭包 → 必然 rebuild）→ `ValueListenableBuilder.build` → `widget.sectionBuilder(context, section)` 被调用 → 当前分区（如 appearance_section 1489 行那棵树）整棵重建 + 布局（`InlineSettingsDock` 常驻树内，折叠只关指针/焦点/语义/动画四条通路，**子树照建照布局**）——**即使设置面板处于关闭态**
- 影响：性能。设置面板越复杂、回复越长，单帧里要重建的无关子树越大；同一信号也驱动 `_live` 节流器与聊天列表。与 main.dart:589 自己承认的「逐条做会拖垮主线程」是同一条纪律的两面（那里省掉的是 localStorage 写，这里没省掉的是整棵子树）。
- 改法方向：把 `_settingsSurface` 从「AppShell.build 里即时求值」改成**只在 `settingsOpen`/浮层打开时构造的稳定子树**（如 `InlineSettingsDock` 的 child 用一个 `const` 包裹的 Builder，或把 section 内容包进 `RepaintBoundary` + 提升到 `State` 缓存）；更彻底的做法是让聊天列表与设置分区各自订阅自己的 listenable（`ChatPanel` 内部 `ListenableBuilder(listenable: _chat)`，外壳不再整体订阅 `_chat`）。
- 验证：widget 测试（确定性，不需真机）——给 `AppShell` 传一个**计数用**的 `sectionBuilder`，泵完后 `controller.stream.add(TextDelta)` 或直接 `chat.notifyListeners()` 10 次，断言计数增长 ≤1（现状每次 delta +1）。辅证：真机开 DevTools Performance，回复期间录 5 s，看 Frames；展开的设置分区越大越明显。
- 置信度：机制高（SDK 源码 + 接线图闭环）／幅度未核实（是否肉眼掉帧需真机 profile，标注为推断）。
- 反向证据：查过是否有 `RepaintBoundary` 隔离设置子树（全仓 RepaintBoundary 只有 shell_backdrop / background_patterns / stage_host 三处）；查过内层 `ListenableBuilder` 是否会因 listenable 未变而跳过 rebuild（`transitions.dart:120-126` 的 `didUpdateWidget` 只做监听转移，框架随后仍 `updateChild` 触发 build）；查过 `_settings` 是否在 delta 期间也 notify（不是——问题恰恰是它不 notify，但父重建照样带着它重建）；查过 CollapsiblePanel 是否折起时跳过子树构建（不跳过，且这是**刻意**的，见其头注「折起来的时候，子树不重建」是为了保活，不是为了省 CPU）。

### F-0005-3 · P1 · 「内容为空的气泡不该出现在界面上」这条测试的断言结构上恒真；而被它声称防止的现象其实**存在**（16 px 空白气泡面）
- 位置：test/chat_bubble_labels_test.dart:102-118（断言）、:108（测试名）；被测实现 lib/ui/message_bubble.dart:147-167（气泡面 `Container` 带 `padding: EdgeInsets.symmetric(horizontal: Space.s3, vertical: Space.s2)`）、:190-196（`message.text.isEmpty` 时**仍然**构造 `_MessageBody`）、lib/design/tokens.dart:750（`Space.s2 = 8`）
- 摘录：
  ```dart
  testWidgets('内容为空且非流式的气泡不该出现在界面上', (WidgetTester tester) async {
    …
    final Finder container = find.byType(Container);
    final Size size = tester.getSize(container.first);
    // 只要没有可见字符就算达标（这里不断言具体宽度，只断言没有文字）。
    expect(size.height, greaterThan(0));
  });
  ```
- 调用链：`ChatMessage(text: '')` → MessageBubble.build → `onlyReasoning=false`（:55-56，无 reasoning）→ `:190 message.text.isNotEmpty || !onlyReasoning` = `false || true` → **仍进 `_MessageBody(text: '')`** → 表面 Container 高 = 上下内边距 2×Space.s2 = **16 px > 0** ⇒ 断言恒真。注释说「只断言没有文字」，但代码里**没有任何一处断言「没有文字」**（既没查 `find.text('')`，也没读 RichText 的 text）
- 影响：测试自身（假绿灯，§6-J 判 P1）；且它掩盖了一个真实的小缺陷——`text==''` 的消息在界面上仍会画出**带底色、带圆角、带描边的 16 px 空泡**，与「模型回了一句空白」在视觉上同形（主链由 `settleTurn` 拦住了，但气泡组件本身没有这层保护）
- 改法方向：① 断言改成「没有任何 Text/RichText 节点的非空内容」或直接断言表面高度 ≤ 某个小值并**同时**在实现里对 `text.trim().isEmpty && !streaming && !failed` 返回 `SizedBox.shrink()`；② 顺手补一条：`_MessageBody('')` 根本不该被构造（把 `:190` 的条件改成 `message.text.isNotEmpty`）
- 验证：把断言换成 `expect(find.byType(SelectableText), findsNothing)`（或 `expect(tester.widget<SelectableText>(...).data, '')` 之外再断言无可见字形）——现状红；或在 message_bubble.dart:190 的条件里去掉 `|| !onlyReasoning` 后，本测试应能捕获「空泡仍渲染」。
- 置信度：高（表达式与内边距常量均可实读推导；`Space.s2 = 8` 见 tokens.dart:750）。
- 反向证据：查过是否有别的测试真的守住「空消息不上屏」（`turn_liveness_test.dart` 测的是纯函数 `settleTurn`，不渲染；`chat_controller` 因 `package:web` 在 VM 测试里加载不了——该文件自己在 :78-80 承认这一点，才用源码扫描补）；查过 `container.first` 会不会取到别的东西（MessageBubble 里第一个 Container 就是气泡面，其上只有 Padding/Semantics/SizedBox，无 Container）。

### F-0005-4 · P2 · 气泡整条 `excludeSemantics` ⇒ **失败轮的「重试」按钮与「复制」对读屏完全不可达**（与 stage_host 已修过的同类 bug 同型）
- 位置：lib/ui/message_bubble.dart:107-133（`Semantics(excludeSemantics: true, …)` 包住 LayoutBuilder→Container→Column 整棵子树）、:241-247（`_BubbleActions` 在该子树内）、:498-499（「重试」`TextButton`）、:497（复制）、:319-321（思考折叠标题 `GestureDetector`，且**无** `Semantics(button:)`）；框架语义：`RenderSemanticsAnnotations.visitChildrenForSemantics` 在 `excludingSemantics` 时不访问子树
- 摘录：
  ```dart
  Semantics(
    liveRegion: announcement != null && message.streaming,
    label: <String>[ … ].join(),
    excludeSemantics: true,      // ← 整条气泡折成一个节点
    child: LayoutBuilder( … _BubbleActions(text:…, onRetry: failed ? onRetry : null, …) ),
  ```
- 调用链：失败轮（`isPlaceholder`）→ MessageBubble.build → `_BubbleActions`(:243) → `TextButton('重试')`(:499) —— 但它在 `excludeSemantics` 子树内 ⇒ 语义树里**没有**这个节点 ⇒ 读屏用户无法聚焦/激活；触摸与键盘 Tab 不受影响（语义排除不影响命中测试与焦点遍历），所以**肉眼与键盘测试都测不出来**
- 影响：无障碍。失败后的恢复路径（重试）与复制按钮对读屏用户不存在。注释(:123-124)只承认了「思考折叠开关不可达」这一个缺口，**没意识到同一处也吃掉了重试/复制**。同类 bug 在本项目**已修过一次**——lib/ui/stage_host.dart:71-76 的注释写着「语义只包舞台本体，不能包整个 Stack：……在 Stack 上写 `excludeSemantics: true` 会把它们一起吃掉，读屏用户就再也点不到「重试」了（第一版就是这么写的）」
- 改法方向：把 `excludeSemantics` 收到最内层（只包正文那段 `SelectableText`），让动作条与折叠开关各自成节点；或保留排除但**显式补** `Semantics(button: true, label: '重试这条消息', child: TextButton(...))`。折叠标题也要加 `Semantics(button: true, label: 展开/收起思考)`。
- 验证：widget 测试——`tester.ensureSemantics()` 后泵一条 `ChatMessage(text:'出错了', failed: true)` + `onRetry`，断言 `find.bySemanticsLabel(RegExp('重试'))` **findsOneWidget**（现状 findsNothing）。同类再补一条复制按钮与思考开关。
- 置信度：高（`excludeSemantics` 的框架行为是定义级的；子树位置可实读）。
- 反向证据：查过是否有测试守住动作条的语义可达（message_bubble_test.dart 的 `find.text('复制')` / `tester.tap(find.text('重试'))` 都是 **widget 树**查找器，`semantics_test.dart` 只断言了气泡那一个 liveRegion 节点，均不覆盖）；查过是否有 `ExcludeSemantics` 被反套回来（无）；查过 `Semantics(container: true)` 会不会救回（不会——`container` 只影响本节点是否成边界，不影响父级 `excludingSemantics` 对子树的裁剪）。

### F-0005-5 · P2 · 字体子集门禁只覆盖源码里的字符串字面量；**模型输出 / 后端文案 / Mod 运行态值**不在门禁内 → 运行时仍会拉 fonts.gstatic.com（断网豆腐块）
- 位置：test/font_subset_test.dart:295-309（扫描范围 = `Directory('lib')` 下的 `*.dart` **源码字面量**）；运行时文本入口：lib/ui/message_bubble.dart:422-475（`_MessageBody` 渲染模型原文）、lib/ui/inline_notice.dart:131-137（渲染 WS `error` 帧的 message/hint）、lib/settings/sections/dev_tools_section.dart:939-941（`formatModStateValue` 渲染 Mod 上报的运行态值）、lib/ui/session_sheet.dart:287（会话标题 = 模型首句）；缺字后果的实证：同文件 :9-13 头注记录的真机 `GET https://fonts.gstatic.com/s/notosanssc/v37/…woff2 [200]`
- 摘录（门禁的覆盖面）：
  ```dart
  final List<File> sources = Directory('lib')
      .listSync(recursive: true)
      .whereType<File>()
      .where((File f) => f.path.endsWith('.dart'))
      .toList();
  for (final File file in sources) {
    for (final String literal in stringLiterals(file.readAsStringSync())) { … }
  }
  ```
- 调用链：模型回一句含 emoji/生僻字的正文（远端数据，不是源码）→ `_MessageBody` 用 `textTheme.bodyMedium`（继承 `kAppFontFamily`，见 lib/ui/theme.dart:101）→ CanvasKit 发现子集里没有该码点 → 引擎的字体回退去 `fonts.gstatic.com` 取 Noto → **有网时看不出问题，断网即豆腐块**（= K/L 红线的原始故障形态，AGENTS 记载真机抓到过 200 响应）
- 影响：无障碍/红线 K 与 L。子集 22 036 码点覆盖常用汉字，但**覆盖不了 emoji、CJK 扩展 B、部分符号**；聊天内容不可预测 ⇒ 这条门禁给的是「源码安全」的假安心。
- 改法方向：① 在 CanvasKit 侧关掉网络字体回退（引擎级 `FontFallbackManager` 可禁用，或自托管一份兜底字体并在 `fontFamilyFallback` 里显式声明——**注意** AGENTS 已写明 `fontFamilyFallback` 在 CanvasKit 下不会命中系统字体，所以要打包而不是指向系统）；② 或在 `_MessageBody` 入口对非子集码点做一次显式替换（不可预测性太高，不推荐）；③ 至少把这条缺口写进 AGENTS 的断网红线一节，别让「字体门禁全绿」被读成「运行时不会拉外部字体」。
- 验证：真机（需 CDP，本环境不可用）——起服务、让模型回一句含 emoji 的消息、Network 面板过滤 `gstatic.com`；有请求即成立。纯代码侧可先做：`grep -rn "fontFamilyFallback" lib/` 现状为零命中，可作为「没有本地兜底」的佐证。
- 置信度：机制高／触发条件中（未核实：具体哪些码点会触发回退需真机；标 未核实，置信度 中）。
- 反向证据：查过是否有运行时字符过滤（`grep -rn "sanitiz|runes" lib/chat/ lib/ui/message_bubble.dart` 只有 `chat_session.dart:412` 的标题截断，不是字符集过滤）；查过 `fontFamilyFallback` 是否有配置（全仓零命中）；查过 `web/index.html` 是否有兜底（归 B11 批，尚未核）。

### F-0005-6 · P2 · 接线守卫「气泡层把 `ChatRole.system` 单独分流」在删掉分流分支后**仍然通过**
- 位置：test/chat_notice_test.dart:171-177；两个使其失效的同词出现：lib/ui/message_bubble.dart:64（真正的分流 `if (message.role == ChatRole.system)`）与 :403（语义 label 里的 `'${ChatRole.system.label}提示：$text'`）
- 摘录：
  ```dart
  test('气泡层把 `ChatRole.system` 单独分流', () {
    expect(
      bubble.contains('ChatRole.system'),
      isTrue,
      reason: '没有这条分流，系统行会被画成 assistant 气泡（=伪造台词）',
    );
  });
  ```
- 调用链：删掉 `message_bubble.dart:64-66` 的 `if (… == ChatRole.system) return _SystemRow(…)` → 测试读 `bubble` 源串 → `:403` 仍含 `ChatRole.system` → **断言仍为真**。它声称守护的失效模式（系统行被画成 assistant 气泡）已真实发生
- 影响：测试自身（假绿灯）。同组另外三条（`settleTurn(` / `kWordlessTurnNotice` / `mustReleaseTurnOnWsLoss(`）在 chat_controller.dart 里各只出现一次，删掉调用会红——**这三条有效**，问题只在第四条
- 改法方向：改成行为断言（系统角色的消息不出现 `kMessageBubbleSurfaceKey` 的气泡面 / 出现 `_SystemRow` 的居中结构），或把断言收紧成 `contains("role == ChatRole.system")` 这类**结构片段**而非标识符
- 验证：删 `message_bubble.dart:64-66` 跑 `flutter test test/chat_notice_test.dart`——现状**绿**（反例成立）；改断言后必须红
- 置信度：高（两处同词出现均为实读）。
- 反向证据：查过该行为是否另有守卫（`chat_notice_test.dart:91-121` 断言系统行「居中 + 弱色 + 小字级」、:123-143 断言语义是「系统提示：」而非「助手说：」，删掉分流分支后**这些会红**）——所以实际风险被别处兜住，本条的危害是「守门人指错了文件」而不是「无人守」。

### F-0005-7 · P2 · `semantics_test` 的 reduced-motion 用例没有泵 `StatePill`：它断言的是框架而不是本项目的行为
- 位置：test/semantics_test.dart:291-308（组名「规格 §9.4：reduced motion 下不跑持续动画」，用例名「thinking 呼吸光在 disableAnimations 时用静态图标」）；被测实现 lib/ui/state_pill.dart:165/:175
- 摘录：
  ```dart
  await tester.pumpWidget(
    MediaQuery(data: const MediaQueryData(disableAnimations: true),
      child: wrap(const SizedBox())),          // ← 树里没有 StatePill
  );
  expect(MediaQuery.disableAnimationsOf(tester.element(find.byType(SizedBox))), isTrue);
  ```
- 影响：测试自身。注释自称「这里断言的是『渲染不崩且状态标签还在』」，但**连一个 StatePill 都没渲染**，「状态标签还在」这半句没有任何断言支撑；真正要守的 `if (widget.phase == thinking && !reduced)` 分支（state_pill.dart:175）此刻零覆盖
- 改法方向：泵 `StatePill(phase: UiPhase.thinking)` 进 `MediaQuery(disableAnimations: true)`，断言渲染里**没有** `AnimatedBuilder`（有则说明动画还在跑），并补一条 `disableAnimations: false` 时**有** `AnimatedBuilder` 的对照
- 验证：把 `state_pill.dart:175` 的 `!reduced` 删掉跑本用例——现状**绿**（反例成立）
- 置信度：高。反向证据：查过是否另有 reduced-motion 守卫（`test/motion_wiring_test.dart` 扫的是 `appMotion`/`appRaisedShadow` 的**接线**，不覆盖 StatePill 的这条分支；`text_scale_test.dart` 无关）。

## BATCH-0006（Phase 1 · lib/ui/ 下半 + lib/design/ 全量）

### F-0006-1 · P1（**升级 F-0003-1**）· 壳背景图在**每个流式 delta** 上被 base64Decode + 整图重解码
- 位置：lib/app/app_shell.dart:765-774（`ShellBackdrop(...)` 在 `AppShell.build` 里**即时构造**）；lib/ui/shell_backdrop.dart:181-193（`_imageLayer` → `decodeDataUrlBytes(dataUrl)` → `Image.memory(bytes, …)`）；:47-59（`base64Decode` 无缓存）；SDK `~/flutter/packages/flutter/lib/src/painting/image_provider.dart:1726-1734`（`MemoryImage.==` 比的是 `other.bytes == bytes`，`Uint8List` 按**身份**）；触发频率由 F-0005-2 给出
- 摘录：
  ```dart
  Widget _imageLayer(String dataUrl) {
    final Uint8List? bytes = decodeDataUrlBytes(dataUrl);   // 每次 build 全新 Uint8List
    …
    image = Opacity(opacity: opacity, child: Image.memory(bytes, fit: …, gaplessPlayback: true, …));
  ```
  频率链（项目自述）：`lib/state/live_region.dart:5`「`text_delta` 是**毫秒级**到达的（一个 SSE chunk 一个 delta）」
- 调用链：WS `text_delta` → `ChatController._appendDelta` → `notifyListeners()` → `main.dart:1072` 外层 `ListenableBuilder` → **`AppShell.build()`** → `:765 ShellBackdrop(...)` → `ShellBackdrop.build` → `_imageLayer` → ① `base64Decode`（预算上限 `kBackgroundTotalMaxChars = 1.5 M` ⇒ 约 1.1 MB 字节）→ ② 新 `Uint8List` ⇒ `MemoryImage` 身份变 ⇒ `ImageCache` **key 变** ⇒ ③ 整张图**重新解码** + 重新上传 GPU
- 影响：性能（仅当用户设了壳背景图/图案时）。**这不是「每次宿主重建」那种低频浪费，而是「每个 delta 一次」**——比 F-0003-1 的登记严重，故升级。F-0003-1 记的是「每次宿主重建全量重解码」，当时**没有把频率算进去**。
- 改法方向：① `decodeDataUrlBytes` 加一层「dataUrl 串 → 已解码 `Uint8List`」的进程内备忘（1–2 条即可，命中即返回同一实例 ⇒ `MemoryImage` 命中 ImageCache）；② 或让 `Image.memory` 走一个自定义 `ImageProvider`，key 用 dataUrl 的内容哈希；③ 或把 `ShellBackdrop` 提成 `StatefulWidget`，只在 `item` 身份变化时重建图片层。任一条都能同时消掉 F-0005-2 的放大效应。
- 验证：widget 测试——构造带背景图的 `AppShell`，在测试里对 chat 控制器 `notifyListeners()` 20 次，断言 `Image` 的 `bytes`（或 `MemoryImage.bytes`）在 20 次前后是**同一实例**（现状每次都不同）。耗时侧：真机开 Performance trace，回复期间看 Main-isolate 帧时间（有背景图 vs 无背景图 A/B）。
- 置信度：机制高（代码 + SDK 双重证据）／幅度未核实（需真机 profile）。
- 反向证据：查过 `Image.memory` 是否有内容级缓存（`gaplessPlayback: true` 只管**解码完成后的旧帧**，不参与 provider key）；查过 `ImageCache` 会不会因 key 相同而命中（key 里含 `Uint8List` 身份，必不命中）；查过 `ShellBackdrop` 是否有 `RepaintBoundary` 之外的隔离（`RepaintBoundary` 只挡**重绘**，不挡 **build/解码**——:162 的注释容易让人误以为隔离了这条路径）；查过背景图是否只在设置面板里渲染（`app_shell.dart:765` 是壳根，每一帧都在）。

### F-0006-2 · P2 · `contentFaint`（令牌自注「仅装饰/图标，不得承载文字信息」）被当**文字色**用三处；白色主题实测对比度 3.96–4.07 < WCAG AA 4.5
- 位置：lib/design/tokens.dart:479-482（令牌自注）+ :451（`contentFaint: on.withValues(alpha: 0.60)`）；使用点 lib/ui/theme.dart:301（`hintStyle` = **全应用输入框占位文字**）、lib/ui/field_row.dart:225-229 与 :231-237（滑杆两端的 min/max 刻度标签，11 px）
- 摘录：
  ```dart
  // tokens.dart:479-482
  /// 更弱的色（**仅装饰/图标**）。
  final Color contentFaint;
  // theme.dart:301（InputDecorationTheme）
  hintStyle: text.bodyMedium?.copyWith(color: colors.contentFaint),
  ```
- 调用链：白色主题（`AppPalette.white`：ink `#242423`，输入框填色 = `surfaceAlt #EDECE9`）→ `contentFaint` = ink@0.60 → 合成后 `#747271` → **对 #EDECE9 的对比度 3.96**（占位文字 14 px）、**对 surface #F7F5F3 为 4.07** → 均低于 4.5；刻度标签 11 px 更严格
- 实算（四套主题，WCAG 相对亮度公式，与 `theme_palette_test.dart:21` 的 `contrast()` 同一算法）：黑 5.91 / 白 **3.96** / 蓝 5.33 / 灰 4.89（均对 surfaceAlt）。作为对照，`contentMuted`（0.74）在同样四套上是 8.29 / 6.04 / 7.27 / 6.51，全部达标
- 影响：无障碍。输入框占位文字与滑杆刻度是**承载信息**的（提示格式、标注量程），不是装饰；白色主题下低视力用户读不清。`theme_palette_test.dart` 的对比度清单里**没有** `contentFaint` 压 `surfaceAlt` 这一项，所以门禁不会红
- 改法方向：把这两处改成 `contentMuted`（0.74，四套都 ≥6.0），或给 `contentFaint` 单独一档「文字用」的弱色（0.66 左右）并把 `AppColors.fieldCount`/copyWith/lerp/==/hashCode/toValuesMap 一起补齐（该项目对「新增字段漏进五件套」有结构枚举测试，会替你钉住）；同时在 `theme_palette_test.dart` 补一条 `contentFaint` 压 `surfaceAlt` 的断言
- 验证：把 `theme.dart:301` 与 `field_row.dart:228/:236` 改成 `colors.contentMuted`，跑 `flutter test test/theme_palette_test.dart` 并加新断言 `contrast(contentFaint 合成色, surfaceAlt) >= 4.5`——现状红（3.96）
- 置信度：高（颜色取值、色值合成与对比度均为确定性计算；三个使用点可实读）。
- 反向证据：查过 `contentFaint` 的其它使用点是否都是图标/装饰（`chat_panel.dart:513` 图标、`theme.dart:372` 拖拽把手色、`theme.dart:415` switch 拇指、`state_pill.dart:67` 空闲态 tone、`appearance_section` 3 处、`dev_tools_section:125` 图标 —— **都是非文字**）；查过 `hintStyle` 是否被别处覆盖（`field_row`/`session_sheet` 的 `InputDecoration` 只改 `isDense`/`hintText`/`border`，不传 `hintStyle`，所以主题这一处**全局生效**）；查过白色主题下输入框是否另有更亮的填色（`inputDecorationTheme.fillColor = surfaceAlt.withValues(alpha: dark ? 0.9 : 1)`，亮色下就是 `surfaceAlt` 原值）。

### F-0006-3 · P3 · 断点 lint 规则只匹配字面量 `maxWidth`，最自然的旁路写法不在拦截面内
- 位置：test/design_tokens_lint_test.dart:99-103（`pattern: RegExp(r'maxWidth\s*[<>]=?')`）；对照 lib/design/breakpoints.dart:10-11（规则自己写「任何 `maxWidth >= 1280` 这类比较都必须走 sizeClassOf」）
- 摘录：`pattern: RegExp(r'maxWidth\s*[<>]=?'),`
- 影响：门禁面。当前代码是合规的（`grep -rnE "\.width\s*[<>]=?\s*[0-9]+" lib/` 零命中，三处断点判断都走 `Breakpoints.sizeClassOf`），所以**不是现存缺陷**；但 `if (MediaQuery.sizeOf(context).width >= 1280)` 这种最自然的写法不会被拦——规则的字面量锚点选窄了
- 改法方向：模式改成同时覆盖 `maxWidth` 与 `size.width`/`sizeOf(context).width` 两种写法；或在规则旁补一条「`lib/` 下 `MediaQuery.sizeOf(...)` 只能在 `Breakpoints.sizeClassOf` 里出现一次」的引用侧对账
- 验证：在 `lib/ui/state_pill.dart` 临时写 `if (MediaQuery.sizeOf(context).width < 900) return …;` 跑该 lint——现状**绿**（反例成立）
- 置信度：高。反向证据：查过是否有别的测试覆盖（`test/breakpoints_test.dart` 只测纯函数 `sizeClassOf` 的边界，不扫调用点）。

### F-0006-4 · P3 · `GlassRim` 把指针频率的信号灌进 `setState`（每帧一次重建 + 重启动画）
- 位置：lib/ui/glass_rim.dart:100-113（`MouseRegion.onHover` → `setState`），:114（`onExit` 同理）
- 摘录：
  ```dart
  onHover: trackPointer ? (PointerHoverEvent event) {
      …
      setState(() => _light = Offset(…));
  } : null,
  ```
- 影响：性能（小）。每次鼠标移动事件一次 `setState` + 重启 120 ms 的 `TweenAnimationBuilder`。**子树的 `widget.child` 不会被重建**（`Element.updateChild` 的 `child.widget == newWidget` 短路，SDK `framework.dart:4027-4034`），所以代价限于 Stack/IgnorePointer/TweenAnimationBuilder/CustomPaint 四个 widget —— 不是灾难，但与项目「高频信号不进 Widget 树」的纪律（P6/O 同族）不一致
- 改法方向：把 `_light` 改成 `ValueNotifier<Offset>`，用 `ValueListenableBuilder` **只**包 `CustomPaint`；或者给 painter 传 `repaint: notifier`（`CustomPainter(super(repaint: …))`），指针移动只触发重绘、不触发 build
- 验证：真机把指针在设置面板上快速移动，Performance 面板看 build count（现状每次移动 +1 build）。纯代码侧可加测试：连续 `tester.sendEventToBinding(PointerHoverEvent(...))` 20 次，统计 `build` 调用次数
- 置信度：机制高／幅度小（推断）。
- 反向证据：查过 `trackPointer` 是否已在 reduced-motion 下关掉（是，`MediaQuery.disableAnimationsOf` 时 `onHover: null`，:98-113 —— 该守的已守住）；查过 child 是否被连带重建（否，见上）。

## BATCH-0007（Phase 1 · lib/chat/ + lib/state/）

### F-0007-1 · P1 · `POST /api/v1/chat` 本地失败不清理 UI 相位 ⇒ 状态胶囊**永久**停在「思考中」、发送键永久变「停止本轮」
- 位置：lib/app/shell_chat.dart:38-46（`_send` 先 `_ui.markTurnAccepted()` 再 `await _chat.send(text)`，**失败后没有任何补偿**）；lib/chat/chat_controller.dart:130-136 与 :144-164（`_failLocalTurn` 只改自己的 `_streaming/_error`，不通知 `_ui`）；lib/state/ui_state_tracker.dart:117-124（`markTurnAccepted` 置 `_turnActive = true`，清相位只有 4 个入口）；lib/state/ui_phase.dart:123-130（`deriveUiPhase`：`turnActive` ⇒ `thinking`）；渲染点 lib/main.dart:1121 `phase: _ui.phase`、lib/ui/chat_panel.dart:353-367（thinking/speaking ⇒ 图标变 stop、回调变 `onStop`）
- 摘录：
  ```dart
  Future<void> _send() async {
    final String text = _input.text;
    if (text.trim().isEmpty) return;
    if (_chat.streaming) return;
    _input.clear();
    _ui.markTurnAccepted();          // ← 相位置为「思考中」
    await _chat.send(text);          // ← 失败只走 _failLocalTurn，_ui 无人复位
    _syncLiveRegion();
  }
  ```
- 调用链（可达性由后端源码确认，非猜测）：本地 POST 失败 → `chat_routes.rs:154 no_supervisor()`=503 / `:487-497 busy_response()`=**429** / 网络异常 → 这三条**都不广播任何 WS 帧** → `UiStateTracker` 的四个清相位入口（`onWsStatus` 断开、`consume(TextDelta/TurnState)`、`markStopped`）**一个都不会被调用** → `_turnActive` 恒为 true → `deriveUiPhase` 恒返回 `thinking` → 状态胶囊「思考中」+ `StreamingIndicator` 三点常驻呼吸 + 圆形键显示「停止本轮」
- 影响：用户可见。失败轮的错误横幅是出的（`error: _ui.errorMessage ?? _chat.error`，main.dart:1162），但**唯一的恢复出口**（「打断并重发」）只调 `onStop`，用户必须先按停止、再按一次发送才能继续；而在此之前界面一直在说「思考中」。`busy` 的服务端文案本身就写着「请等待 turn_state 收口后再发」，而本轮压根不会有 turn_state。
- 改法方向：① 把「本地发送失败 ⇒ 相位回落」收进 `ChatController` 与 `UiStateTracker` 的接缝——最小改法是在 `shell_chat._send` 的 `await` 之后按 `_chat.error != null` 调 `_ui.markTurnFailed()`（新增一个「失败且未在飞」的入口，不要复用 `markStopped`——那会把服务端忙碌说成「已打断」）；② 更好：抽成 `turn_liveness.dart` 的纯函数（那里已有 8 个判据 + 30 条真断言的模式），并让 `errorActionsFor` 的 busy 分支给出「重发」而不是「打断并重发」。
- 验证：把判据抽成纯函数后，VM 单测：`settleTurn` 同级新增 `phaseAfterLocalSendFailure(turnActive: true, sendFailed: true) == idle`；接缝侧（需要真机）——开两个页面，A 正在对话时在 B 点发送 → B 应立即回到「空闲」且可再次发送（现状：永久「思考中」）。
- 置信度：高（可达路径逐跳可读，后端「不广播帧」已实读 `chat_routes.rs`）。
- 反向证据：查过是否有别处清 `_turnActive`（`grep -n "_ui\." lib/main.dart lib/app/*.dart` 只有 5 处：`onWsStatus` / `consume` / `markTurnAccepted` / `markStopped` / `clearError`+`dispose`，无一覆盖本地失败）；查过 `_ui.errorMessage ?? _chat.error` 是否会把相位带成 error（**不会**——`deriveUiPhase` 的 `errorActive` 只由 `consume` 置位，`_chat._error` 不参与）；查过 WS 掉线能否自愈（能，但 429 busy 时 WS 是**连着的**，自愈路径不触发）；查过 `_abandonTurn`（切会话）是否同病（同病但**会自愈**——那一轮仍在服务端跑，`turn_state` 到达即清）；查过测试是否覆盖（`ui_state_tracker_test.dart` 30+ 条真断言，但**结构上覆盖不到**这条接缝：`ChatController` 依赖 `package:web`，VM 测试加载不了）。

### F-0007-2 · P2 · 错误动作「打断并重发」只调 `onStop`，**从不重发**——文案承诺与实现不符
- 位置：lib/ui/error_actions.dart:39-41（`if (code == 'busy') return [ErrorAction(label: '打断并重发', onPressed: onStop)]`）与 :44-46 的文案回退分支；接线 lib/main.dart:1167-1170（`onStop: () => unawaited(_stopWithCancellation())`）；`_stopWithCancellation` main.dart:758-761（只 `_cancelStageForTurnBoundary` + `await _stop()`，**无重发**）
- 摘录：
  ```dart
  if (code == 'busy') {
    return <ErrorAction>[ErrorAction(label: '打断并重发', onPressed: onStop)];
  }
  ```
- 调用链：服务端 429 busy（`chat_routes.rs:487`）→ 前端 `_failLocalTurn(…, 'busy')` → `errorActionsFor(code:'busy')` → 按钮文案「打断并重发」→ 按下 → `_stopWithCancellation()` → `_stop()` → `_chat.stop()` + `_ui.markStopped()` ⇒ **只打断，不重发**；用户看到「已打断」两秒后回到「空闲」，那条消息**并没有被重新发出去**
- 影响：用户可见（文案与实现不一致，P2 档明列）。这是 busy 场景**唯一**的出路（见 F-0007-1：此时相位还卡着），按钮承诺了做不到的事；而 `errorActionsFor` 的 `onSend` 参数**已经传进来了**却没被 busy 分支用上
- 改法方向：busy 分支要么真的重发（`onPressed: () { onStop(); onSend(); }` 或新增一个组合回调），要么把文案改成「打断本轮」（并另给一个「重试」动作）。按 F-0007-1 一起改最省：本地失败时本来就不该留在 thinking，按钮应该是「重试」。
- 验证：widget/单元测试——`errorActionsFor('busy 错误', code: 'busy', onGoto: …, onStop: () => stops++, onSend: () => sends++)` 返回的**第一个**动作，按下后断言 `sends` 或 `stops` 的实际行为与标签一致（现状：标签说「重发」而 `sends == 0`）。注意 `errorActionsFor` **全仓零测试命中**（F-0001-1 已记同一处零覆盖）。
- 置信度：高（标签与回调逐行可读；接线点唯一）。
- 反向证据：查过 `onStop` 里是否间接触发重发（`_stop()` → `_chat.stop()` 只 POST `/chat/stop` 并本地收口，`grep` 全仓无「stop 后自动重发」路径）；查过是否有第二个动作同时提供重发（busy 分支 `return` 的是单元素列表，`_send` 传进来的 `onSend` 在这条分支上**完全没用上**）。

## BATCH-0008（Phase 1 · lib/api/ 全量）

### F-0008-1 · P1 · 心跳被解析、被下发、然后**被两个消费者丢弃**：前端没有任何看门狗 ⇒ 半开连接不可检测，「POST 成功但收不到回复」的幽灵态可复现
- 位置：lib/api/ws_frame.dart:38-41 + :428-429（`HeartbeatEvent` 已实现并解析）；lib/chat/chat_controller.dart:235-236（`case HeartbeatEvent():` → `break`，无动作）；lib/state/ui_state_tracker.dart:185-186（同样 `return false`）；lib/api/ws_client.dart:109-122（`ensureConnected` **只**看 `readyState`）与 :63-66（`connect` 无看门狗）；全仓 `grep "Timer(" lib/main.dart lib/app/*.dart` **零命中**（没有任何周期任务）；后端每 10 s 发一次心跳（`crates/live2d-ai-desktop/src/web_api/ws/connection.rs:56` 「服务端主动 heartbeat 间隔（10s…）」）
- 摘录（两个消费者的处理）：
  ```dart
  // chat_controller.dart:235
  case SubscribeAckEvent():
  case HeartbeatEvent():        // ← 解析了、收到了，然后什么也不做
  case ActionCueEvent():
  case UnknownWsEvent():
    break;
  ```
- 调用链（故障路径）：半开连接（笔记本休眠/唤醒、NAT/代理静默丢包、服务端被 SIGKILL 且没有 FIN/RST 送达）→ 浏览器 `WebSocket.readyState` **仍是 OPEN**（规范只在「尝试收发并失败」时才改状态，而本客户端**从不发送**任何帧）→ `ensureConnected()` 走 `isLiveSocket(OPEN)` → **直接 return，不重连**；`_status` 仍是 `connected` → 连接徽标写「已连接」；用户点发送 → `POST /api/v1/chat` **成功**（HTTP 是另一条连接）→ 但 `text_delta`/`turn_state` 永不到达 → `_turnInFlight` 恒为真 → `turn_liveness.mustReleaseTurnOnWsLoss` 的前提（`status.isProblem`）**永远不成立** → 界面停在「思考中」，与用户报的「发出去没回应」完全同形
- 影响：用户可见的**卡死态**。这正是 2026-09-10/11 两次修过的那个历史 bug 的**另一种成因**：前两次修的是「socket 对象还在但连接已废」（`readyState` 判据），这一次是「socket 状态机自己也以为一切正常」。服务端心跳是**已经送到前端**的存活信号，前端解析了却从不使用
- 改法方向：① 在 `WsClient` 内记 `_lastFrameAt`（每收到任何一帧都更新），加一个 30 s 周期 `Timer`（≈3 个心跳周期）：若 `now - _lastFrameAt > 30s` 且 `isLiveSocket` → 视同僵尸，`_detach` + `_open()`（复用 `ensureConnected` 的现成路径，判据抽成 `ws_liveness.dart` 的纯函数并配回归，那里已经有 `isLiveSocket`/`isZombieSocket` 的模式与 53 行测试）；② 或者反过来做**双保险**：`ensureConnected()` 额外要求「最近 3 s 内见过心跳」才认为连接可用。
- 验证：① 纯逻辑侧——新增 `isStaleFrame(lastFrameAt: DateTime, now: DateTime, timeout: Duration)` 并在 `ws_liveness_test.dart` 逐边界断言（刚收到 / 29.9 s / 30.1 s）；② 真机侧（需 CDP，本环境不可用）——开着页面把服务端 `SIGSTOP` 掉（或用 `tc netem` 丢包），30 s 后看连接徽标是否变「已连接→重连中」。
- 置信度：机制高（三个事实均可实读：心跳被丢弃 / 无 Timer / `ensureConnected` 只看 readyState）／**触发频率未核实**（半开连接多久出现一次需真机观察，标 未核实）。
- 反向证据：查过是否有别处的心跳/超时机制（`grep -rn "lastHeartbeat|watchdog|Timer(" lib/api/ lib/main.dart lib/app/` 零命中；`ws_liveness.dart` 只有 readyState 判据，没有时间判据）；查过 `onerror`/`onclose` 能否兜住（规范上浏览器最终会因 TCP 超时触发 close，但**没有上界**——这正是幽灵窗口）；查过 `mustReleaseTurnOnWsLoss` 能否兜住（它的前提是 `status.isProblem`，而 status 不会变）；查过服务端是否在 `shutdown_ready` 时主动断（会断，但只覆盖「优雅重启」，覆盖不了崩溃/静默丢包）；查过是否有轮询兜底（前端只轮询 HTTP 状态/诊断，**不轮询 WS**）。

## BATCH-0009（Phase 1 · lib/audio/ + lib/voice/）

### F-0009-1 · P3 · PTT 没有「松手事件丢失」的兜底：`_ListenButtonState.dispose` 不补发 release ⇒ `_ptt` 可永久为 true
- 位置：lib/ui/chat_panel.dart:411-419（`_ListenButtonState.dispose` **只** `_holdTimer?.cancel()`，没有「若正在按住则补发 `onPressRelease`」）；lib/voice/voice_listen_controller.dart:206-237（`pressStart` 置 `_ptt = true`）与 :240-258（`pressRelease` 是**唯一**把 `_ptt` 置回 false 的正常路径；另两个复位点是 `stop()`:192 与 `_onError(fatal)`:429）
- 摘录：
  ```dart
  @override
  void dispose() {
    _holdTimer?.cancel();
    super.dispose();          // ← _holding 为 true 时不通知宿主
  }
  ```
- 调用链：按住「听」→ 计时器到 150 ms → `_holding = true; onPressStart()` → 控制器 `_ptt = true`、识别器开始监听 → **此刻按钮从树上消失**（断点切换导致 ChatPanel 重建 / 浮层遮挡 / 或浏览器的 pointerup 因窗口失焦未送达）→ `dispose` 只取消计时器 → 宿主的 `_ptt` **永远为 true**
- 影响：① 麦克风持续占用（浏览器的录音指示灯不灭），而用户以为已经松手——这是隐私面；② 常驻听被挡（`_onEnd`:443 与 `start()`:185 都以 `_ptt` 为不启动条件）；③ 按钮显示「松」。恢复只能靠再点两次「听」（走 `toggle() → stop()` 复位 `_ptt`），用户没有任何线索知道要这么做
- 改法方向：① `_ListenButtonState.dispose` 里 `if (_holding) { _holding = false; widget.onPressRelease?.call(); }`（1 行，与 `_onCancel` 的语义一致）；② 更稳的是在控制器里加一道兜底：`pressStart` 时记时间戳，超过 N 秒仍无 release 且无定稿 ⇒ 自动收口（避免任何 UI 侧漏事件都卡死）
- 验证：widget 测试——`tester.startGesture` 按住 `听` 超过 `kVoiceTapThreshold`，不抬手，直接 `pumpWidget(SizedBox())` 把按钮从树上摘掉，断言宿主收到的最后一次回调是 `onPressRelease`（现状：**没有任何回调**）。
- 置信度：中（机制确凿；触发路径「按钮在按住期间被销毁」是推断，未在真机上复现过 pointerup 丢失，标 未核实）。
- 反向证据：查过是否有其他复位路径（`stop()`:189-192 与 `_onError(fatal: true)`:424-435 都会清 `_ptt`，但前者要用户主动操作、后者要识别器报错）；查过 GestureDetector 的 `onTapCancel`（拖出按钮范围时会补发，覆盖「拖出」但不覆盖「元素被销毁」与「窗口失焦吞事件」）；查过 `chat_panel` 的按钮在断点切换时是否真会被销毁（`AppShell.build` 只改 `Flex.direction` 与 `flex`，ChatPanel 元素大概率复用——**这条让触发概率进一步降低**，但浮层/页面切换等场景未逐一验证）。

## BATCH-0010（Phase 1 · lib/live2d/ 全量）

### F-0010-1 · P3 · `sync(paused:)` 是死线字段：前端三处签名都留着，渲染面全树从未读取，且无调用方
- 位置：lib/live2d/live2d_bridge.dart:192（参数声明）+ :211（`if (paused != null) payload['paused'] = paused;`）；lib/live2d/live2d_stage.dart:512 + :525（透传）
- 摘录：
  ```dart
  void sendSync({ … bool? paused, … }) {
    …
    if (paused != null) payload['paused'] = paused;
  ```
- 反向链（两端都空）：`grep -rn "paused" lib/` 的其余命中只有 `audio_player.dart` 里 **HTMLMediaElement 自己的** `paused` 属性（与本字段无关）；`grep -rn "paused" crates/ --include=*.rs` **零命中**（渲染面 `stage-config` 分支读 `scale/dark/stageColor/lipSync/mouthSensitivity/actionScales/idleEnabled/clickEnabled/tier`，没有 `paused`）；`grep -rn "paused" test/` **零命中**——即**没有任何调用方传过它**
- 影响：契约漂移（E 维度）的一种形态：协议表面宣称支持「暂停舞台」，实际是空操作。下一个读 `sendSync` 签名的人会以为有暂停能力。与 Q 红线「只增不改」不冲突（删一个从未被实现的字段不违反），但它让 `sendSync` 的 10 个参数里有一个是幻觉
- 改法方向：删掉三处 `paused` 参数（前端无调用方，删了不会有行为变化）；若产品确实需要「暂停舞台」（拖动时冻结？最小化时停渲染？），则在 `main.rs` 的 `stage-config` 分支实现 `payload["paused"]`（落 `st.bridge.paused`，渲染循环里跳过 rAF 提交）并在协议文档登记
- 验证：删掉 `live2d_bridge.dart:211` 后 `flutter analyze` 应无引用错误（当前零调用方）；反向验证——在 `main.rs` 的 `stage-config` 分支加一行 `if payload.get("paused")…`，前端发一次应当能看到行为变化（现状：无论发不发，渲染面毫无反应）
- 置信度：高（两侧 grep 均为空，可复现）。
- 反向证据：查过是否有别的通道传暂停语义（`grep -rn "pause" lib/live2d/` 无命中）；查过历史提交里是否曾有实现（本批不做 git 考古，仅记为待确认）。

### F-0010-2 · P2（**升级 F-0001-4**）· bridge 的 `progress` 与 `fps` 都不触发任何重建 ⇒ 加载百分比与 FPS 徽标显示陈旧值
- 位置：lib/live2d/live2d_bridge.dart:430-435（`case 'progress'` → `_progress = …; notifyListeners();`）、:439-444（`case 'fps'` → `_fps = …; notifyListeners();`）；lib/live2d/live2d_stage.dart:301-321（`_onBridgeChanged` 只转发 phase/error/ack/ready，**没有 progress，也没有 fps**）；lib/live2d/live2d_stage.dart:586-591（FPS 徽标在 `build` 里**现读** `bridge.fps`）；lib/main.dart:1104-1108（唯一的重建触发点 `onPhaseChanged` **仅在相位变化时** `setState`）+ :1213（`stageProgress: …bridge?.progress` 在外壳 build 里现读）；lib/ui/stage_host.dart:132-138（`LinearProgressIndicator(value: progress)` + 「模型加载中… N%」）
- 摘录：
  ```dart
  // main.dart:1104 —— 桥的全部通知唯一能引起重建的出口
  onPhaseChanged: (Live2DBridgePhase phase) {
    if (mounted && phase != _stagePhase) setState(() => _stagePhase = phase);
  },
  ```
- 调用链：渲染面每秒发 `fps`（`crates/l2d-wasm-demo/src/main.rs:922-938`「v1：每秒上报一次 fps」）→ `_onRaw` → `notifyListeners()` → `_onBridgeChanged` 跑一遍（**没有任何 setState**）→ `Live2DStage` 不重建 → FPS 徽标停在**上次外壳重建时**的数值；应用安静时（没有 WS 消息、没有设置变更）**永久冻结**。`progress` 同理：模型加载期间百分比停在外壳上次重建时的值，`LinearProgressIndicator` 拿到的是陈旧 `value`
- 影响：用户可见的「数字说谎」。与 F-0001-4 同一根因（桥有 notify、宿主只对 phase 敏感），本条把后果补全为**两处**并给出确定频率（fps 1 Hz）
- 改法方向：① 在 `_onBridgeChanged` 里对「progress/fps 变了」调 `setState`（最低成本，只重建 `Live2DStage` 自身，几十个 widget，不触发外壳）；② 或者把 FPS/进度改成 `ValueListenable`（更符合本项目「高频不进 Widget 树」的纪律——虽然 1 Hz 远不算高频）
- 验证：widget 测试——构造 `Live2DStage`（注入 fake transport），推进桥的 `fps` 事件后 `await tester.pump()`，断言 FPS 徽标文本从 `30 FPS` 变成新值（现状：不变）；`progress` 同理断言「模型加载中… N%」的数字变化。
- 置信度：高（两处读点 + 唯一重建出口 + 渲染面 1 Hz 频率，三者均可实读）。
- 反向证据：查过是否有别处订阅桥（`live2d_stage.dart:341` 确实 `addListener(_onBridgeChanged)`，但那个回调**只转发四类事件**）；查过 `Live2DStage` 是否用 `AnimatedBuilder(animation: bridge)`（:563 的 `AnimatedBuilder` 绑的是 `_stageColorMotion`，**不是** bridge）；查过外壳是否有别的周期重建源（`ListenableBuilder` 监听的是 `_live/_chat/_ui/_audio.unlockState/_settings/_voiceListen`，安静时都不动）；查过 `stage_overlay_single_test.dart` 是否覆盖（它只钉「只有一套覆盖层」，不覆盖数值新鲜度）。

## BATCH-0011（Phase 1 · web/ + pubspec + tool/）

### F-0011-1 · P3 · pubspec.yaml 头注的版本号与实际值不同步（rc.4 vs rc.5+10）
- 位置：shell/flutter/pubspec.yaml:4-5（注释）与 :7（`version: 0.2.0-rc.5+10`）
- 摘录：
  ```yaml
  # 与后端 `Cargo.toml` 的 `version` 对齐（0.2.0-rc.4）。
  # `+8` 是 Flutter 的 build number，与后端版本语义无关——…
  version: 0.2.0-rc.5+10
  ```
  （注释里的 build number 也从 `+8` 漂到了 `+10`，两处都没跟上。）
- 影响：可维护性。**版本线是三处同步的东西**（`Cargo.toml` / `pubspec.yaml` / README 首屏，见 AGENTS.md），注释写错会让人以为线停在 rc.4；与 §8 已登记的「AGENTS.md 的当前版本仍是 rc.4」同族，但那是另一个文件，**这一处此前未登记**
- 改法方向：注释里的版本号删掉具体值，改成「与后端 `Cargo.toml` 的 `version` 对齐（build number 与后端版本语义无关）」——让它**结构上无法过期**；顺带把 `+8` 那句去掉
- 验证：`grep -n "0.2.0-rc" shell/flutter/pubspec.yaml` 应只命中 `version:` 一行（现状命中两行）
- 置信度：高。反向证据：查过是否 `Cargo.toml` 也停在 rc.4（`Cargo.toml` 的版本是 0.2.0-rc.5 系，与 pubspec 一致 ⇒ **不是**版本漂移，只是注释没跟上）。

## BATCH-0012（Phase 1 收口 · test/ 覆盖盘点）

### F-0012-1 · P1 · 连通性自检的成败判定靠**显示文案**（`contains('ok'|'ms'|'毫秒')`）⇒ 上游错误里带 "ms" 时失败被当成成功，而成功结果根本不渲染 ⇒ 点「测试连接」毫无反应
- 位置：lib/settings/sections/llm_section.dart:178-183 与 lib/settings/sections/tts_section.dart:174-179（**两处逐字同款**）；分类依据的字符串在 lib/main.dart:949-952（`_testLlm`）与 :971 起（`_testTts`）；渲染端 lib/ui/field_row.dart:617-618（`error: resultIsError ? result : null`，**`result` 在 `resultIsError == false` 时没有任何去处**）；结构化数据本来就在 lib/api/settings_models.dart:762-776（`ok` / `latencyMs` / `errorCode` / `errorMessage`）
- 摘录：
  ```dart
  result: testResult,
  resultIsError:
      testResult != null &&
      !testResult!.toLowerCase().contains('ok') &&
      !testResult!.contains('毫秒') &&
      !testResult!.contains('ms'),
  ```
- 调用链：上游 5xx/非 2xx → 后端 `test_endpoints.rs:271-285` `TestOutcome::failure("protocol_error", format!("上游 {status}: {snippet}"))`，`snippet` 是**上游响应体前 512 字符**（OpenAI 兼容端点的超时错误里 `…timed out after 30000ms` / `retry in 200ms` 极常见）→ 前端拼成 `'失败：上游 502: {"error":"…ms…"}'` → 分类器命中 `'ms'` ⇒ `resultIsError = false` → `FieldActionRow` **只把 `result` 喂给 error 槽** ⇒ **整条结果行不渲染** ⇒ 用户点了「测试连接」，按钮转一圈后**什么都没有**
- 影响：用户可见的「诊断按钮静默失效」，且发生在**用户最需要它的时候**（上游坏了的时候）。AGENTS.md 写明「**自检与产品链路抢资源的代价是『自检说谎』——那比没有自检更坏**」，本条是那个家族的具体形态。与 F-0003-2（成功态无渲染槽）同组件、同一次点击的**两个独立缺陷**
- 改法方向：把 `ok` 当**数据**传下去（`LlmSection(testResult: …, testOk: bool?)` / 或干脆传 `SettingsTestOutcome`），由 `FieldActionRow` 决定槽位；判据用 `o.ok` 这一个布尔，**禁止**从展示串里猜类型（AGENTS 的错误码纪律原话：「禁止让前端从显示文案里猜错误类型（文案会改，码是契约）」）。顺带给 `FieldActionRow` 补成功槽（F-0003-2 的改法）
- 验证：widget 测试——泵 `LlmSection(controller: …, view: …, onTest: () async {}, testResult: '失败：上游 502: {"error":"request timed out after 30000ms"}', testOk: false)`，断言界面**出现**该文案（现状：整行不渲染，红）。第二段：`testResult: 'ok · 12 ms'`, `testOk: true` ⇒ 断言出现内联结果行（现状同样不渲染）
- 置信度：高（两处代码逐字可读；`FieldActionRow` 只有一个去处可证；后端 `snippet` 取响应体前 512 字已实读）。
- 反向证据：查过失败文案是否可能**永远**不含 "ms"（后端 `failure()` 的两个调用点：`auth_failed` 固定文案「鉴权失败 (401)」不含 ms，但 `protocol_error` 拼的是**上游响应片段** ⇒ 含 ms 完全由上游决定，不受本仓控制）；查过 `resultIsError=false` 时是否有别处渲染（`FieldActionRow` 全函数已读，只有 `error:` 一条通道）；查过是否有测试钉住这条（`test/asset_guard_tts_config_test.dart:147-178` 只钉「是按钮、会回调、文案不宣传成试听」，**没有**结果槽断言——与 F-0003-2 的覆盖缺口同源）。

### F-0012-2 · P2 · `LlmSection`（189 行）整块零测试覆盖，而同族的 `tts_section` 有 5 条 asset_guard
- 位置：lib/settings/sections/llm_section.dart（全文件，类名 `LlmSection` 在 test/ 中零命中）；对照 lib/settings/sections/tts_section.dart（同族，有 `asset_guard_tts_config_test.dart` 5 条）
- 证据：`grep -rn "LlmSection" test/` **零命中**；`grep -rln readAsStringSync test/` 之外的 93 个测试里，也没有任何一个以「渲染 LLM 分区」的方式间接覆盖（`compact_settings_page_test` 用的是 `SettingsSection.appearance` 固定分区）
- 影响：覆盖缺口。该分区承载的是**最常被报告的失败路径**（401「保存了还是 401」）、密钥写入入口（`EnvKeyField`）、思考开关（`show_reasoning`）与 token 上限（4096 的历史教训）——这些都只有「模型层」测试（`settings_api_test`）而**没有「界面层」测试**（按钮在不在、点了会不会回调、文案会不会骗人）
- 改法方向：照抄 `asset_guard_tts_config_test.dart` 的骨架给 LLM 分区补 4–5 条：① 四件套字段都在；② 自检是按钮、会回调、文案不宣传成「试听」；③ 改 base_url/model 只落 llm 草稿不碰别的段；④ `show_reasoning` 开关三态显示（草稿优先于服务端值）；⑤ 输出 token 上限的 description 明确写「思考也占这份预算」（这是 2026-09-12 那次 512→4096 事故的界面侧防线）
- 验证：新测试跑起来应当全绿（现状不存在这些测试）；反向价值用第 ② 条证明——把 `resultIsError` 的分类改成文案猜（F-0012-1）时，若有 ⑤ 那种「文案诚实」断言就能抓到分类错位
- 置信度：高（grep 实测；对照组的 5 条已读过）。
- 反向证据：查过是否有快照/集成测试间接覆盖（全仓无 golden 测试，`tool/visual_review_test.dart` 是人工产物不是门禁）；查过 `confirm_discard_test.dart:325` 是否渲染 LLM 分区（它只把 `SettingsSection.llm` 当**跳转目标**用，不渲染内容）。

## BATCH-0013（Phase 2 · 横扫 A：状态源唯一性）

### F-0013-1 · P1（**升级 F-0001-3**）· 启动水合窗口内导入的背景图会「先说已加入、随后凭空消失」，删除/重排/偏好改动一并被回滚
- 位置：lib/main.dart:141（`_store = MemoryBackgroundStore()` 起始）/ :167-174（`_hydrateBackgrounds`：`_hydrateSafely(store, _prefs)` 取**当时**的 prefs 快照 → `setState { _store = store; _prefs = hydrated.prefs; }` **整体覆盖**）/ :176-192（`_openStore` 的 3 秒超时）；lib/app/shell_prefs.dart:247（`storeBackground(widget.store, …)` —— `widget.store` 就是 `ShellRoot.store`，水合完成前是**内存库**）/ :249-262（成功后立刻把新项写进 prefs，并提示「已加进背景库」）；lib/data/background_hydration.dart:86-125（**每张图一次 `readChecked` 读回 base64** ⇒ 窗口长度 ∝ 库体积）
- 摘录：
  ```dart
  Future<void> _hydrateBackgrounds() async {
    final BackgroundStore store = await _openStore();
    final BackgroundHydration hydrated = await _hydrateSafely(store, _prefs);  // ← 快照
    if (!mounted) return;
    setState(() {
      _store = store;
      _prefs = hydrated.prefs;      // ← 窗口期里用户的一切改动被这一行整体抹掉
    });
  ```
- 调用链（数据丢失那条）：水合进行中 → 用户在「外观与互动」导入一张图 → `storeBackground(**MemoryBackgroundStore**, id, dataUrl)` 返回 true（内存库当然写得进去）→ 偏好加了一项、界面出现缩略图、提示「已加进背景库」→ 水合完成 → `setState { _store = 真实 IndexedDB 库（内存库被丢弃）; _prefs = 旧快照（那一项不存在） }` ⇒ **字节在已废弃的内存库里、清单项被覆盖 ⇒ 图消失，而用户刚被告知它已加入**
- 同窗口内的另外两种回滚：**删除**（`shell_prefs.dart:303/377` 删的是内存库里的字节，prefs 项被快照加回 ⇒ 删了又出现）；**任意偏好改动**（主题/音量/透明度的 `_update` 全部被整体覆盖 —— 这就是 F-0001-3 记的那条，本条补上了「库」这一维，升级理由在此）
- 影响：数据丢失 + 界面说谎。窗口长度 = IndexedDB 打开 + **每张背景一次 `readChecked`**；项目自己给这个窗口设了 **3 秒超时上限**（`main.dart:_hydrateTimeout`，因为 IndexedDB 在某些隐私配置下既不 resolve 也不 reject）——库越大、磁盘越慢，窗口越接近 3 秒，而这正是用户打开设置、导入背景的合理时间窗
- 改法方向：① **不要整体覆盖**：完成时只回填背景相关字段（`_prefs = _prefs.copyWith(backgrounds: hydrated.prefs.backgrounds, backgroundSource: …)`），其余字段一律保留最新值；② 窗口内**禁用**背景库写操作并说清原因（`_hydrating` 标志透到设置面板，「正在读回背景库…」），比事后修复更诚实；③ 两者都做最佳：① 防数据丢失，② 防说谎
- 验证：写一个可控 store 的测试——`FakeStore` 的 `readChecked` 用 `Completer` 挂起 → 启动 → 在窗口内调 `storeBackground` + 断言 prefs 含新项 → `completer.complete()` → `pump` → 断言**新项仍在**且字节落在真库里（现状：项消失）。第二段：窗口内改主题 → 水合完成 → 断言主题仍是用户改的那个（现状：被回退）
- 置信度：机制高（四个写入点逐跳可读，`widget.store` 的来源已追到 `main.dart:224`）／**窗口时长未核实**（需真机：库越大越明显）。
- 反向证据：查过 UI 是否有 `hydrating` 态在挡写操作（`main.dart` 全文无该标志，设置面板也无禁用逻辑）；查过 `MemoryBackgroundStore` 是否在换接后被复用（`:170` 直接替换引用，旧实例无引用者）；查过导入是否会因为「先存字节后改偏好」而自愈（那是 P4 的教训、方向是对的，但本条的病在**存到了哪个库**，不在顺序）；查过 B2 的 P0-1…P0-5 是否覆盖这条（它们覆盖的是**存储故障**下的误删，本条是**换库窗口**下的误丢，机制不同）。

## BATCH-0015（Phase 2 · 横扫 C：竞态 / 取消 / 三态）

### F-0015-1 · P3 · `shell_admin` 的 5 条用户可见操作只捕 `on ApiException`、靠分支内手动复位 busy 标志，缺 `SettingsController` 那套「兜底 catch + finally」样板
- 位置：lib/app/shell_admin.dart:56-93（`_loadAdmin`：`on ApiException` @87，`_adminLoading = false` 在 83 与 90 各写一次）、:141-178（`_activateModel`）、:185-202（`_importModel`）、:204-220（`_toggleMod`）、:115-130（`_loadLogs`）；对照样板 lib/settings/settings_controller.dart `load()`（`on ApiException` + `catch (e)` + **`finally { _loading = false; notifyListeners(); }`**）
- 摘录：
  ```dart
  // shell_admin.dart:_loadAdmin —— 两个分支各写一次复位，没有 finally
  } on ApiException catch (e) {
    if (!mounted) return;
    _adminError = e.toString();
    _adminLoading = false;
    _refresh();
  }
  // settings_controller.dart:load() —— 样板
  } on ApiException catch (e) { _error = e.toString(); }
    catch (e) { _error = '加载设置失败：$e'; }
    finally { _loading = false; notifyListeners(); }
  ```
- 影响：**结构性**缺口。任一非 `ApiException` 逃逸 ⇒ `_adminLoading`/`_busyId` 永久停在「处理中」，用户既不能重试也看不出发生了什么，而同一个应用里的设置面板早就用对了写法。**现实概率低**（本批逐个核过 `ModelInfo`/`ModInfo`/`AppCapabilities`/`LogLine`/`EnvKey`/`SettingsTestOutcome` 的解析器全部宽容、`ApiClient._guard` 又把传输异常统一包成 `ApiException('network_error')`）——所以是 P3 而不是 P1/P2；但它把 0001 批次留下的未核实候选**证伪到了「概率低 + 结构仍缺」**这一步
- 改法方向：把这 5 处统一成 `SettingsController.load()` 的形状（`on ApiException` 给带码文案 + `catch (e)` 给兜底 + `finally` 复位标志）；`_saveEnvKey`（:102）已经是这个形状，可作行内样板
- 验证：给 `_loadAdmin` 注入一个抛 `StateError('boom')` 的 fake api（models_api.list），调用后断言 `_adminLoading` 回到 false 且 `_adminError` 非空（现状：异常逃出、标志卡死）
- 置信度：高（结构差异可实读）／可达性低（已给证据链）
- 反向证据：查过是否所有解析器都宽容（6 个 fromJson 全用 `is String`/`is num` 三元，零强转）；查过 `_guard` 是否漏包（`api_client.dart:229-235` `catch (error) → ApiException('network_error')`，`env_api`/`mods_api`/`diagnostics_api` 各自也有一份）；查过是否有全局 zone 兜底（`main.dart` 无 `runZonedGuarded`）。

### F-0015-2 · P3 · `swapModel` 超时后 `stream.first` 的订阅不取消，桥 dispose 时以未处理的 `StateError` 收场
- 位置：lib/live2d/live2d_bridge.dart:150-165（`final Future<String> loaded = _modelLoads.stream.first; … return await loaded.timeout(timeout) == url;` —— `catch` 只捕 `TimeoutException`）；`_modelLoads` 在 :496-498（`dispose` 里 `unawaited(_modelLoads.close())`）
- 摘录：
  ```dart
  final Future<String> loaded = _modelLoads.stream.first;
  await sendSync(model: url);
  try {
    return await loaded.timeout(timeout) == url;
  } on TimeoutException {
    return false;                 // ← 此时 loaded 仍挂着，没人再听它
  }
  ```
- 调用链：激活一个卡住的模型 → 15 s 无 `loaded` 回执 → `timeout` 抛 → catch 返回 false（界面如实说「舞台未回执」✓）→ **但 `loaded` 那个 future 还挂在 broadcast 流上** → 随后宿主 dispose（刷新/退出/重试）→ `_modelLoads.close()` → `.first` 以 `StateError('No element')` 完成 ⇒ 该错误**无人处理**（await 已结束）⇒ 逃到 zone
- 影响：开发期噪音为主（浏览器控制台 uncaught；`flutter test` 环境下会直接判测试失败）。不造成状态损坏，但它是「超时后不取消订阅」这一类问题的**可复现实例**，而项目自己在别处（`ws_client` 的 `unawaited(subscription.cancel())`、`main` 的五个 `unawaited(x?.cancel())`）对这条纪律执行得很严
- 改法方向：保留 `StreamSubscription` 并在两条路径都 `cancel()`（超时分支与正常分支）；或改用 `Completer` + `_onRaw` 主动 complete（与 `_modelLoads` 的 broadcast 语义等价且不产生悬挂订阅）
- 验证：widget/单测——用 fake transport 建桥 → `swapModel('x', timeout: 50ms)`（不发 `loaded`）→ `bridge.dispose()` → `expect(tester.takeException(), isNull)`（现状：未处理的 `StateError` 会被测试框架捕获 ⇒ 红）
- 置信度：中（机制确凿；`broadcast stream.first` 在关闭时的完成语义为 `StateError('No element')` 属标准行为，但**未在本环境实跑**（禁跑 flutter test），标 未核实/中）
- 反向证据：查过 `dispose` 是否先 cancel 了这条订阅（`:495-500` 只 close 五个流，没有任何 per-subscription 记录）；查过 `.first` 是否自带取消（`first` 在**完成时**自动取消，但超时路径下它没完成 ⇒ 不取消）；查过 `sendSync` 失败是否更早抛出（`sendSync` 内部 `_send` 把异常吞进 `_fail`，不会逃出）。

## BATCH-0016（Phase 2 · 横扫 D：build 重活 / 列表 / 同步重计算）

### F-0016-1 · P2（与 F-0006-1 同根因）· 背景库每张缩略图在每次 build 重解码 base64，并让 ImageCache 的键每秒抖动 N 次
- 位置：lib/settings/sections/appearance_section.dart:1131-1150（`_ImageTile.build` → `decodeDataUrlBytes(dataUrl)` → `Image.memory(bytes, cacheWidth: 96, …)`）；触发频率来自 F-0005-2（设置分区每个 `text_delta` 重建）；同款解码器在 lib/ui/shell_backdrop.dart:47-59（无缓存）
- 摘录：
  ```dart
  @override
  Widget build(BuildContext context) {
    final Uint8List? bytes = decodeDataUrlBytes(dataUrl);   // ← 每 build 一次
    if (bytes == null) return ColoredBox(color: palette.surfaceAlt);
    return Image.memory(bytes, fit: BoxFit.cover, cacheWidth: 96, gaplessPlayback: true, …);
  }
  ```
- 调用链：流式回复中（外观分区开着）→ 每个 `text_delta` → AppShell 重建 → 背景库 `ReorderableListView.builder` 重建可见项 → 每项一次 `base64Decode`（400 KB 图约 0.5 ms）+ 一次**新** `MemoryImage`（`bytes` 身份变 ⇒ key 变 ⇒ `ImageCache` 未命中 ⇒ 96 px 重解码 + 重上传）
- 影响：性能 + 可见抖动。① 8 张图 × 30 delta/s ≈ 240 次 base64 解码/秒（纯浪费）；② `ImageCache` 默认上限 300 条，`cacheWidth` 让每条条目很小但**条目数**照涨 ⇒ 仍在用的条目会被挤掉（Lru 淘汰）⇒ 缩略图偶发重新解码/闪一下；③ 与 F-0006-1（壳背景整图）同根因：**`decodeDataUrlBytes` 没有任何备忘层**
- 改法方向：与 F-0006-1 同一个修法即可一并解决 —— 给 `decodeDataUrlBytes` 加一层「dataUrl 串 → 已解码 `Uint8List`」的进程内备忘（1–3 条 LRU 即可，命中即返回**同一实例** ⇒ `MemoryImage` 命中 ImageCache，两个症状一起消）
- 验证：widget 测试——构造含 3 张图的背景库，泵出后记录 `find.byType(Image)` 的 `image.bytes` 身份，触发 10 次宿主重建，断言三次取到的是**同一实例**（现状：每次都不同）；顺带断言 `ImageCache` 的条目数不随重建次数增长
- 置信度：机制高／幅度未核实（需真机 profile）
- 反向证据：查过 `cacheWidth: 96` 是否把解码成本压到可忽略（只压**像素**不压 **base64 解码**，后者才是每 build 必付的那一份）；查过 `ListView`/`ReorderableListView.builder` 是否只建可见项（是，但**可见项每次重建都重新 build**）；查过 `Image.memory` 是否有内容级缓存（`MemoryImage.==` 按 `Uint8List` 身份，SDK `image_provider.dart:1726`）。

## BATCH-0017（Phase 2 · 横扫 E：契约漂移）

### F-0017-1 · P2 · 后端 `SettingsView.performance` 段前端**一个字段都不解析**，LLM 分区把表演层写死成「（默认关）」——用户开了也在说关着
- 位置：后端 `crates/live2d-ai-runtime/src/settings/view.rs:45-48`（`pub performance: PerformanceView`）+ :180-189（七个字段：`enabled` / `wired` / `base_url` / `model` / `has_api_key` / `timeout_ms` / `structured`）；前端 `lib/api/settings_models.dart:421-435`（`SettingsView.fromJson` **只**读 `llm` / `tts` / `persona` / `action` / `dev_mode` / `active_model_id`）；界面 `lib/settings/sections/llm_section.dart:144-151`（`ReadonlyField(label: '表演层（默认关）', text: kLlmPerformanceText, description: kLlmPerformanceDescription)`，两个文案是 :31/:36 的**顶层常量**）
- 摘录：
  ```dart
  // settings_models.dart —— 六段，performance 不在其中
  return SettingsView(
    llm: LlmSettingsView.fromJson(_obj(j['llm'])),
    tts: TtsSettingsView.fromJson(_obj(j['tts'])),
    persona: PersonaSettingsView.fromJson(_obj(j['persona'])),
    action: ActionSettingsView.fromJson(_obj(j['action']), activeModelId: _str(j['active_model_id'])),
    devMode: j['dev_mode'] == true,
  );
  // llm_section.dart —— 标签里的「默认关」是写死的字面量
  ReadonlyField(label: '表演层（默认关）', …)
  ```
- 调用链：`GET /api/v1/settings`（返回含 `performance` 的完整视图）→ `ApiClient.fetchSettings` → `SettingsView.fromJson` **丢弃该段** → `shell_settings` 传给 `LlmSection` 的 `view` 里没有它 → 界面只能显示常量标签「表演层（**默认关**）」与常量说明「表演层默认关」⇒ 用户在 `live2d-ai.toml` 里 `[performance] enabled = true` 且端点配齐（`wired = true`，即**每轮真的会发 HTTP**）时，界面仍在说「默认关」
- 影响：用户可见的**说谎型 UI**（正是 AGENTS 里「控件说一套、画面做另一套」那一类）。且这是 2026-09-22 用户亲自敲定的分工展示位，恰恰是最不该说错的地方；用户会据此以为表演层没开、或去 toml 里反复确认
- 改法方向：① `settings_models.dart` 加 `PerformanceSettingsView`（七个字段，照 `LlmSettingsView` 的形状，缺省安全：`enabled: false` / `wired: false`），`SettingsView.fromJson` 读 `j['performance']`；② `LlmSection` 的标签按真值渲染（「表演层：已开 / 未开 / 开着但没接线（缺端点或密钥）」——`wired` 这一位尤其该露出来，因为它是「会发 HTTP」的唯一判据）；③ 顺手在 `test/settings_api_test.dart` 加一条 `performance` 段的解析断言（该文件已有 32 条同形状用例）
- 验证：单测——`SettingsView.fromJson({'performance': {'enabled': true, 'wired': true, 'base_url': 'http://x', …}})` 断言 `view.performance.enabled == true`；widget 测试——泵 `LlmSection`（view 带 `performance.enabled = true`），断言界面上**不**出现「默认关」而出现「已开」（现状：两个断言都红）
- 置信度：高（后端七个字段与前端六段解析均可实读；界面标签是字面量）
- 反向证据：查过是否有别处读 `performance`（`grep -rn performance lib/api/settings_models.dart lib/settings/` 只命中**注释与文案常量**，无字段读取）；查过是否后端把该段藏在 dev_mode 后面（`settings_to_view` 无条件投影，注释写明「只读展示」）；查过 `director_panel` 是否补了这个信息（它只在**Mod 说明文案**里提到表演层，不是状态展示）。

## BATCH-0018（Phase 2 · 横扫 F：无障碍 / 键盘 / 字体门禁覆盖面）

### F-0018-1 · P3 · 响应体非 JSON 时合成 `http_<status>` 码：用户拿它去日志里搜**搜不到**，且 message 是整段 HTML
- 位置：lib/api/api_client.dart:247-264（`_errorFrom`：有 `error.code` 用它，否则合成 `'http_${response.statusCode}'` 并把 `response.body` 整段当 message）；同样的形状在 `env_api.dart:126-130` / `mods_api.dart` / `diagnostics_api` 各有一份
- 摘录：
  ```dart
  return ApiException(
    'http_${response.statusCode}',
    response.body.isEmpty ? '请求失败' : response.body,   // ← 代理/网关的整页 HTML
    status: response.statusCode,
  );
  ```
- 影响：违反「码是契约、拿码搜日志」这条纪律（后端所有 4xx/5xx 都发 `{"error":{"code",…}}`，只有**非本服务**的响应（反向代理、网关、DevTools 的拦截页）才走这条回退）。两种表现：① 用户/开发者在日志里搜 `http_502` 零命中；② 整页 HTML 被塞进 `ErrorBanner` 的 `InlineNotice` 文案里（几千字符塞进一个单行提示）
- 改法方向：合成码换成**前端本地码**并在文案里说清来源（`upstream_unparsed_${status}` + 「响应不是本服务返回的 JSON（前面有代理？），下面是原始响应前 200 字」+ **截断** body）
- 验证：单测——`MockClient` 返回 `<html>502 Bad Gateway</html>` 状态 502 → 断言 `e.code` 以 `upstream_unparsed_` 开头且 `e.message.length <= 200`（现状：code = `http_502`、message = 整段 HTML）
- 置信度：高（代码级确定）。**严重度低**：只在非本服务的响应上触发。
- 反向证据：查过本服务是否有非 JSON 错误响应（`chat_routes`/`settings_routes` 的 `ErrorDetail` 路径统一 JSON）；查过是否有全局响应体长度上限（无）。

## BATCH-0019（Phase 2 · 横扫 G：安全）

### F-0019-1 · P3 · 文件选择不查体积：上限检查在「读完之后」，选到超大文件会先崩/先卡
- 位置：lib/app/browser_io.dart:119-156（`pickFileDataUrl`：`input.accept = accept;` 之后直接 `reader.readAsDataURL(files.item(0)!)`，**全程不读 `files.item(0).size`**）；调用方的体积检查在读完之后：lib/app/shell_prefs.dart:234（`DisplayPrefs.canAddBackground`）、:247（`storeBackground`）与 persona 卡导入
- 摘录：
  ```dart
  input.accept = accept;                 // 只是对话框的筛选提示，用户仍可选「所有文件」
  …
  reader.readAsDataURL(files.item(0)!);  // ← 整个文件进内存并 base64 编码（约 ×1.37）
  ```
- 影响：可用性/资源。一个 500 MB 的「图片」会在体积检查之前就被读成 ~680 MB 的字符串（dataURL），标签页可能直接 OOM 或长时间无响应；用户看到的不是「图片太大」而是**界面卡住**。项目的三条预算（`kStageImageMaxChars` / `kBackgroundImageMaxChars` / `kBackgroundTotalMaxChars`）只防住了**存储**，没防住**读取**
- 改法方向：在 `reader.readAsDataURL` 之前读 `files.item(0).size`，超过一个明确上限（按目标预算反推，如 8 MB）就 `finish(null, '这个文件太大了（X MB）——请换一张小一点的')`，**不读进内存**；上限做成参数（图片与角色卡两条路径不同）
- 验证：widget/浏览器测试较难（`FileReader` 不可注入），可把体积判据抽成纯函数 `bool fileTooLarge(int bytes, int maxChars)` 并单测边界；真机侧手工验证——选一个 >8 MB 的文件，期望立刻得到「太大」提示（现状：长时间无响应后要么崩要么报「读不出内容」）
- 置信度：中（机制确凿；`accept` 不是强制约束、浏览器行为各略有差异，标 未核实/中）
- 反向证据：查过是否有 `input.size` / `files.item(0).size` 的任何读取（全文件零命中）；查过调用方是否在读之前拦（`:234` 的 `canAddBackground` 需要 dataUrl 才能算长度 ⇒ 必然在读之后）；查过是否有全局读超时（无）。

## BATCH-0021（Phase 2 · 横扫 I：样式令牌 / 主题覆盖）

### F-0021-1 · P2 · 行内码强制 `fontFamily: 'monospace'` 且无 `fontFamilyFallback` ⇒ 码段里的中文触发**网络字体回退**（红线 L）
- 位置：lib/ui/message_bubble.dart:441-449（`_MessageBody.spanOf` 的 code 分支）；对照 lib/ui/theme.dart:101（`ThemeData.fontFamily: kAppFontFamily`，组件层唯一入口）与 lib/design/typography.dart:101-103（`buildAppTextTheme` 刻意不设 family）
- 摘录：
  ```dart
  if (span.code) {
    return TextSpan(
      text: span.text,
      style: (base ?? const TextStyle()).copyWith(
        fontFamily: 'monospace',          // ← 全仓唯一一处组件层字体族覆盖
        backgroundColor: codeBackground,
      ),
    );
  }
  ```
- 调用链：模型输出含 `` `超时` `` / `` `未配置` `` 这类**反引号包中文**的片段 → `parseChatMarkdown` 判为 code span → `TextSpan(fontFamily: 'monospace')` → CanvasKit 在等宽字体里找不到汉字 → 走**引擎的字体回退**（Web 端 = 从 `fonts.gstatic.com` 取 Noto）⇒ **断网时那一段变豆腐块**
- 证据来源：项目自己的真机记录（`test/font_subset_test.dart:9-13`：实测到 `GET https://fonts.gstatic.com/s/notosanssc/v37/…woff2 [200]`，并把「有网时完全看不出来」写进注释）；以及 AGENTS 的原话「CanvasKit 取不到设备字体，`fontFamilyFallback` 也不会命中系统字体」——**也就是说：不给 fallback，就等于把缺字交给联网回退**
- 影响：红线 L 的运行时破口。`font_subset_test` 门禁**看不到它**（它管的是「字符是否在子集内」，而这里的字符**在**子集内，只是被换到了另一个字体族上）——这正是门禁覆盖面 < 规则适用范围的又一个例子（CONSOLIDATION 组 D）
- 改法方向：给这个 span 补 `fontFamilyFallback: const <String>[kAppFontFamily]`（`kAppFontFamily` 在 `ui/theme.dart:21` 导出）——等宽字体有 ASCII、没有中文时**回落到自托管子集**，零网络请求；顺带对 `FontWeight` 变体（bold span）也核一遍（它不换族，安全 ✓）
- 验证：widget 测试——`MessageBubble(message: ChatMessage(role: assistant, text: '跑 `超时` 这一步'))` → 取 code span 的 `style` → 断言 `fontFamilyFallback` 含 `'NotoSansSC'`（现状：null，红）。真机侧：让模型回一句含中文反引号的内容，Network 面板过滤 `gstatic.com`（现状：应有一次请求）
- 置信度：高（机制与项目自证一致）／触发频率未核实（取决于模型输出习惯）
- 反向证据：查过是否有全局兜底（`ThemeData` 没有 `fontFamilyFallback`；`typography.dart:27-30` 的注释明确说 CanvasKit 下 `fontFamilyFallback` 命中不了**系统**字体——注意那说的是「系统字体」，而本条要的是**自打包字体**作为 fallback，这是可行的且正是它该用的场景）；查过代码 span 是否只含 ASCII（无法保证，模型输出不可控）；查过 CanvasKit 是否会把 `ThemeData.fontFamily` 当作 span 级回退（不会——span 上的 `fontFamily` 会**覆盖**继承值，除非显式写 `fontFamilyFallback`）。
