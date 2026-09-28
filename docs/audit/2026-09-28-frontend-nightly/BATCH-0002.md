# BATCH-0002 — Phase 1 · 偏好与存储域（display_prefs + data/ + background_item）

## 本批读过（全读）
| 文件 | 行 | 一句话 |
|---|---|---|
| lib/settings/display_prefs.dart | 1006 | 22 字段五件套（ctor/copyWith/toJson/fromJson/==+hashCode）逐项核对**齐全**；clamp 纪律一致 |
| lib/design/background_item.dart | 274 | djb2 mod 2^31-1 跨 VM/web 稳定；== 与 hash 都按 id，自洽 |
| lib/data/background_store.dart | 124 | 三态读契约（found/missing/failed）设计正确；probe 默认 false 保守 ✓ |
| lib/data/background_store_web.dart | 240 | IDB 永不抛/自带超时/等 oncomplete；open 失败**永久缓存 null**（激进但安全） |
| lib/data/background_store_stub.dart | 60 | 内存语义定义 ✓ |
| lib/data/background_hydration.dart | 164 | P0-1/P0-2 防线在位且有测试（故障→不摘项、不剪枝、不写回） |
| lib/data/background_reorder.dart | 36 | P0-3 双重调整已修，语义注释与 SDK 一致 |
| 补读 | — | audio_bar.dart 滑杆段(90-110)、live2d_bridge.sendStageBg/_enqueueOrSend、live2d_stage.sendStageBg(472)、main 音量接线 1199 |

## 发现
- **F-0002-1（P1）** 每次偏好变更都全量重发 stage-bg（≤1.5M 字符 dataURL）+ 全量 jsonEncode+setItem：
  音量滑杆每帧触发一遍——有舞台图/轮播存量时是明显性能悬崖（渲染面还每张重解码 createImageBitmap）。
- **F-0002-2（P2）** 启动 store 置换窗口：窗口内 `_pickImage`/删除走占位 MemoryBackgroundStore，
  「已记住」为假话，字节永不进 IndexedDB → 下次启动按孤儿摘除丢图。
- **F-0002-3（P3）** `main._openStore` 的 3s timeout+catch 在 web 上是死代码（createBackgroundStore 同步构造、永不抛/永不挂）；
  注释宣称的「退路内存库」线上到不了。真正的兜底在 `_IdbBackgroundStore._open` 内部（那条是活的）。

## 核对记录（不成发现但值得钉的证据）
- DisplayPrefs 22 字段：`theme, stageImage, backgrounds, backgroundSource, backgroundOpacity, backgroundBlur, backgroundScrim, imageFit, imageAlign, slideInterval, slideRandom, uiTransparency, stagePlaylist, scale, mouthSensitivity, lipSync, idleEnabled, muted, volume, allowDragZoom, tier, edgeStrength` —— copyWith 22/22、toJson 22/22、fromJson 22/22、== 22/22（列表用 sameAs）、hashCode 22/22。**无漏网字段**。
- `_readBackgroundSource` 旧 syncShellStageBg 迁移只在 backgroundSource 缺席时生效，显式值不被覆盖 ✓。
- toJson 只写 id 不写 dataUrl —— display_prefs_test.dart:566 有钉；hydrate 的故障矩阵在 background_store_test.dart:124-206 有真实失败注入用例（非空转）。
- IDB put 以 `transaction.oncomplete` 定成败（配额在提交时判）✓；`onblocked` 立即判失败（激进：另一连接没让位就放弃，本会话所有操作降级为诚实失败——安全侧）。
- `backgroundFingerprint` 上界论证成立：h<2^31-1，h*33+c < 2^37 < 2^53，web double 精确 ✓。
- DEC-1（slideInterval 越界→0）、DEC-5（坏 dataURL isRenderable）、DEC-6（第 0 项）均**已登记不重报**。

## 本批命令
read 全量 7 文件；grep（openSettings/jumpTo/dataUrl/onChanged/sendStageBg/_enqueueOrSend）；
test 抽查：display_prefs_test 192 expect 具体断言、background_store_test 故障矩阵、无 expect(常量,常量)。

## 本批未核实
- 渲染面对重复 stage-bg 是否真的每张重解码（rc.5 changelog 如此描述：base64→Blob→createImageBitmap→write_texture；Rust 侧不在审计范围，只按「会重解码」记账）。
- IDB open 永久缓存 null 后，用户随后恢复存储权限→本会话永不恢复（行为保守但符合注释；未验证浏览器是否真允许会话中途恢复）。
- F-0002-2 的实际窗口长度（hydrate ≤3s 上限 + _open 内部 3s 串行）——未真机测。
