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
- 影响：用户可见/数据（失败后草稿丢失，违项目「失败保草稿」家法——settings_controller.dart:18 头注第 3 条明写「保存失败不清空草稿」，本面板是同一纪律的例外点）；busy 期间点击静默无响应
- 改法方向：`_send` 返回 bool（成功 true），`_import` 只在成功时 clear；busy 早退时把按钮置 `onPressed: null`（`canAct` 已含 `!_busy`，但对话框确认后的二次点击仍走这里）
- 验证：widget 测试：fake onCommand 抛 ApiException → 导入一条 → 断言 `memory-import-field` 的 TextField 文本**非空**（现状空）
- 置信度：高。反向证据：查过 `_send` 是否把结果回传给调用方（返回 `Future<void>`，无）；查过是否有别的清空路径（无）。

### F-0004-3 · P3 · mod_panels.dart 注释里 sed 残留 `@@final@@`
- 位置：lib/settings/mods/mod_panels.dart:17
- 摘录：`函数 tear-off 在不同目标上是不同的常量，故这里用 @@final@@。`（应为 `final`）
- 影响：无功能影响；注释误导
- 验证：`grep -n "@@" lib/settings/mods/*.dart`
- 置信度：高。反向证据：全仓扫 `@@` 仅此一处（非批量 sed 未清干净的普遍现象）。
