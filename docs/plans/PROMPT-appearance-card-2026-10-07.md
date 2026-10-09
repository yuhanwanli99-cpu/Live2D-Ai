# 外观卡片收口 · 交给下一个 agent

> 2026-10-07。管理员已改完库侧，**停在测试编译之前**。下一个 agent 只收口测试，并修 `flutter analyze` 在 `shell/flutter` 里报出的真错误。
> 不要提交、不要打 tag、不要推送、不要升版本。

## 现状（先读，再改）

工作树脏。下面这些**已经在树上，不要回退**：

- T1：上屏 `text_delta` 才带 `sentence_seq`。`GenerationFinished` 的收尾锚点不加。`reasoning_delta` 不加。
- T2：`DialogueHistoryKey`。失败或停止不提交。不落盘。
- T3：朗读高亮（`spoken_highlight.dart`）。已审，收。
- T4：`docs/DOC-MAP.md` 第 1 节三行指针。已审，收。
- 设置手感：动作幅度滑条跟手、本模型覆盖的 epoch 闩、人设 / LLM / TTS 文案。
- 版本号已写成 `0.2.2`（`Cargo.toml`、`pubspec.yaml`、README 徽章、`docs/releases/v0.2.2.md`）。tag 仍是 `v0.2.1-rc.1`。
- **外观库侧已按批准方案改完**，测试还在用已删除的字段，`flutter test` 现在编不过。

外观产品口径（已落在库里，测试要钉住它，不要再设计一遍）：

1. 外观组卡片只剩：四套配色；背景库（添加 / 清空 / 不透明度；**至少两项**才出现轮播）；内置图案；一句「当前这一项同时铺在舞台上。选了内置图案时，舞台回到纯色，壳仍画图案。」；界面透明度在这块的最底下。
2. 不再有、也不再落盘：`edgeStrength`、`backgroundSource`、整张「舞台单图轮播」、逐图 opacity/fit/align、四档铺法、贴片边长、九宫格、四档遮罩、模糊。发丝线固定：暗色 alpha `0.12`，浅色 `0.10`。产品路径铺满（cover）、遮罩自动（0）、模糊 0、对齐居中。
3. 保留：舞台与口型、动作幅度（含跟手滑条）、互动。`backgroundEnabled` 保留。
4. 背景只有库这一份。舞台图是当前库项经 `DisplayPrefs.stageProjectionUrl` 投影，走既有 `sendStageBg`。不要把 wasm 的两条绘制通道并成一条。
5. `fromJson`：库为空且 `stageImage` 是长度不超过 `kStageImageMaxChars` 的 data URL 时，收成库的第一项。库里已有项时不插入。`shellImage` 只在**没有** `backgrounds` 列表时迁成一项。读入忽略、写回不再产生：`edgeStrength`、`backgroundSource`、`stagePlaylist`、`imageFit`、`imageAlign`、`tileSize`、`backgroundBlur`、`backgroundScrim`、逐图 opacity/fit/align、`shellImage`、`syncShellStageBg`。未知键本来就会忽略。
6. 超限大图（壳可以画，最长到约 24 MB）**不要**塞进 `stage-bg`：`stageProjectionUrl` 在 url 长度大于 `kStageImageMaxChars`（1,500,000）时返回 null。图案、关背景、空库、非 data URL 也返回 null。

落盘 **15** 键，与 `const DisplayPrefs({...})` 一一对应：

`theme`、`backgrounds`、`backgroundOpacity`、`backgroundEnabled`、`slideInterval`、`slideRandom`、`uiTransparency`、`scale`、`mouthSensitivity`、`lipSync`、`idleEnabled`、`muted`、`volume`、`allowDragZoom`、`tier`。

`ShellBackdrop` 仍接受 fit / align / tile / scrim / blur 参数，供渲染单测直接传入。产品外壳传的是常量：`fitCover`、`defaultImageAlign`、`defaultTileSize`、`defaultBackgroundScrim`、`defaultBackgroundBlur`。`fitName`、`alignTable`、`boxFitFor`、`scrimAlphaFor`、`tileScaleFor`、轮播纯函数（`stagePlaylistIndexOf` 等）和 `readStagePlaylist` 都还在，不要删。

`Live2DStage.stageImage` 这个 **widget 形参名保留**。`didUpdateWidget` / `_attach` 里的 `sendStageBg(widget.stageImage)` 不要改。`asset_guard_stage_attach_resend_test.dart` 钉的就是这个调用形状，默认不要动它。

`_clearTransientResults` 现在清的是 `_shellImageMessage` / `_shellImageFailed`，外加原来的 LLM / TTS / 管理结果，以及 `_modelOverrideMessage`。`_stageImageMessage` 与 `_stageImageFailed` 已从 `main.dart` 删除。

`AppearanceSection` 构造函数已不再接受 `onPickStageImage`、`onClearStageImage`、`onAddToPlaylist`、`onClearPlaylist`、`stageImageMessage`。

## 要改的测试

只改测试，外加 `cd shell/flutter` 之后 `flutter analyze` 点名的库告警（未使用 import、多余 `!`、未使用私有成员）。不要为了让旧断言复活而把旋钮加回 UI。

| 文件 | 改成什么 |
| --- | --- |
| `test/display_prefs_persist_keys_test.dart` | 手写全集、样本表、逐键往返都改成上面 15 键。`kPersistedKeyCount = 15`。构造函数扫描保留。迁移：`shellImage` → 一项；空库 + 合法 `stageImage` data URL → 第一项；库已有项时忽略 `stageImage`；超长或非 data URL 不收；`toJson` 不含上表那些旧键。 |
| `test/display_prefs_test.dart` | 删掉对 `stageImage` / `edgeStrength` / `backgroundSource` / `clearStageImage` / `clampEdgeStrength` / `clampBackgroundBlur` 字段的访问。舞台图那组改成 `stageProjectionUrl` 与「空库才迁入」。描边那组改成：`toJson` 无 `edgeStrength`，外观源码无 `描边强度`。背景来源那组改成：非空库 `hasBackground` 为真；不透明度 0 为假；图案不投影；超限 data URL 不投影。`fitName` / `alignTable` 纯函数断言保留。源码扫描里「设置读了 imageFit / backgroundSource」改成：读 `backgroundOpacity`、`uiTransparency`、轮播间隔常量，以及那句舞台投影文案；源码里没有 `描边强度`、`舞台单图轮播`、`铺法（图）`。 |
| `test/display_prefs_background_fit_test.dart` | 直接把 `fit:` / `tileSize:` 传给 `ShellBackdrop` 的渲染断言保留。`_host` 的默认值改用 `DisplayPrefs.defaultImageFit` 等常量，不要读已删字段。`DisplayPrefs(imageFit:)`、`DisplayPrefs(tileSize:)`、`clampTileSize`、`fromJson(...).imageFit`、`app_shell 把 prefs.tileSize 传下去` 改成：旧 JSON 键被忽略；外壳传的是 `defaultTileSize`（64）。 |
| `test/display_prefs_style_change_detection_test.dart` | 不再点「样式 / bg-style-set-fit」。逐图 `fit` 不再落盘：`BackgroundImage.toJson` 只有 kind+id，`fromJson` 忽略 opacity/fit/align。保留「改一个仍在的字段（不透明度或轮播）会穿过 `==` 闸门并往返」这种真断言，避免 F-0034-01 的闸门测试整段消失。 |
| `test/design_tokens_test.dart` | `AppMaterial` 只有 `radiusScale` 与 `uiTransparency`。`toString` 带上这两个。另断言暗色发丝线 alpha 0.12、浅色 0.10（读 `AppColors.of` 或主题扩展上的 `hairline`，不要再乘 edgeStrength）。 |
| `test/setting_wiring_test.dart` | `boxFitFor` 四档结果不同、`scrimAlphaFor` 四档不同、`alignmentFor` 九档不同：这些是渲染函数，保留。UI 暴露四档铺法 / 遮罩档 / 位置垫的断言改成：**界面里不再出现**这些控件（`铺法（图）`、`kBackgroundAlignPadKey`、遮罩的 `FieldOption`）。「字段必须有读者」只保留仍存在且被外壳读取的键：`backgroundOpacity`、`slideInterval`、`slideRandom`、`uiTransparency`。已删字段放进「不许复活」：构造函数里不得再出现 `this.edgeStrength`、`this.imageFit`、`this.imageAlign`、`this.tileSize`、`this.backgroundBlur`、`this.backgroundScrim`、`this.backgroundSource`、`this.stageImage`、`this.stagePlaylist`。 |
| `test/transient_results_test.dart` | 清理清单改成源码里真实清掉的字段，包含 `_shellImageMessage = null` 与 `_shellImageFailed = false`。不要再要求 `_stageImageMessage` / `_stageImageFailed`。 |
| `test/shell_backdrop_test.dart` | `_host` 的 `scrim:` 默认改为 `DisplayPrefs.defaultBackgroundScrim`。 |
| `test/shell_backdrop_decode_cache_test.dart` | `fit:` 默认改为 `DisplayPrefs.defaultImageFit`。 |
| `test/appearance_background_library_test.dart` | 去掉 `stageImage` / `backgroundSource` 构造。`铺法（图）` 始终不出现。有库时仍有「添加图片」，没有「换一张」「清除舞台背景图」。 |
| `test/appearance_background_runtime_test.dart` | `_pane` 去掉 `onAddToPlaylist` / `onClearPlaylist`。DEC-6 只保留「当前标记跟运行时索引」。位置垫与「舞台单图轮播」改为找不到。轮播开关仅在 `backgrounds.length >= 2` 时出现。预览、坏图、水合中若仍点得到现有控件，就保留。 |
| `test/appearance_background_style_test.dart` | 删掉点「平铺 / 描边强度 / 对齐垫 / 逐图样式」的用例。保留仍作用于现有控件的用例（批量删除按身份、不透明度或界面透明度的「松手才提交」）。界面源码与泵出来的树上都没有「描边强度」。 |

新增两条迁移断言（放进 persist-keys 或 display_prefs_test，不要第三份重复文件）：

- 空库 + 合法 `stageImage` data URL → `backgrounds` 第一项就是这张图，`stageProjectionUrl(prefs, 0)` 等于该 URL。
- 库已有一项时，JSON 里的 `stageImage` 不插入。

## 禁止

- 不要改 `crates/`、wasm、`persona` 角色卡导入、`mods.json`、`.env`、`live2d-ai.toml`（含 example）。
- 不要改 `AGENTS.md`、docs 预算常量、`docs/audit/**`。
- 不要动动作幅度滑条、`ModelOverrideCoalescer`、口型 / 待机 / 拖拽、`audio_player.dart`、朗读高亮。
- 不要新增 WS 帧，不要改已有帧的字段含义。
- 不要把测试改成恒真，不要整文件删除后不留行为断言。
- 不要为了编译通过把已删旋钮加回界面。
- 新文案必须落在 Noto Sans SC 子集内。已有那句舞台投影文案不要改字。
- 不要在仓库根目录跑 `flutter analyze`。根目录会扫进 `docs/audit/**/*.dart`，那些报错不是本包的。
- 不要提交。

## 验收

Flutter 不在 PATH 上。在 `shell/flutter` 里跑：

```bash
cd /home/skystar/Live2D-Ai/shell/flutter
/home/skystar/flutter/bin/flutter analyze
/home/skystar/flutter/bin/flutter test
```

两条都要 exit 0。把各自最后 30 行贴回。analyze 把 warning 当失败。

没有浏览器工具。不要声称做过肉眼点击。运行中的 `/app/` 要看到这次界面变化，需要另跑 `flutter build web --release --base-href /app/ --no-web-resources-cdn`，那不在本任务里。

做不到就写明卡在哪条断言，不要换产品方案。

## 提示词（整段复制给下一个 agent）

```text
你在 /home/skystar/Live2D-Ai，分支 main。只做一件事：让外观卡片简化之后的 Flutter 测试重新编译并通过。库侧产品代码已经按批准方案改完。不要重新设计外观，不要把删掉的旋钮加回去，不要提交。

先读 docs/plans/PROMPT-appearance-card-2026-10-07.md，再读这些库文件确认口径，然后只改测试（以及 flutter analyze 在 shell/flutter 里点名的未使用 import / 多余 !）：

- shell/flutter/lib/settings/display_prefs.dart 与五个 part（codec / copy / derived / limits / playlist）
- shell/flutter/lib/settings/sections/appearance_section.dart 与 appearance_background*.dart
- shell/flutter/lib/design/tokens.dart（发丝线 0.12 / 0.10）与 tokens_material.dart（AppMaterial 无 edgeStrength）
- shell/flutter/lib/app/app_shell_state.dart（ShellBackdrop 传常量）
- shell/flutter/lib/app/shell_section_wiring.dart 的 _clearTransientResults
- shell/flutter/lib/main.dart 里 Live2DStage.stageImage: DisplayPrefs.stageProjectionUrl(...)

必须钉住的行为：

1. toJson 恰好 15 键：theme, backgrounds, backgroundOpacity, backgroundEnabled, slideInterval, slideRandom, uiTransparency, scale, mouthSensitivity, lipSync, idleEnabled, muted, volume, allowDragZoom, tier。kPersistedKeyCount = 15。构造函数字段集合与这 15 键相同。每个键都有非默认往返，并参与 == / hashCode。
2. 没有 backgrounds 列表时，shellImage 迁成一项。backgrounds 已是列表（含空列表）时不读 shellImage。库为空且 stageImage 是长度 ≤ kStageImageMaxChars 的 data URL 时，收成第一项，stageProjectionUrl 返回它。库非空时忽略 stageImage。超长、空串、非 data URL 不收。toJson 不再写出 stageImage、backgroundSource、edgeStrength、stagePlaylist、imageFit、imageAlign、tileSize、backgroundBlur、backgroundScrim、shellImage、syncShellStageBg。
3. stageProjectionUrl：关背景、空库、图案、dataUrl 为空、非 data URL、长度 > kStageImageMaxChars → null。否则返回该 data URL。
4. 界面没有「描边强度」「舞台单图轮播」「铺法（图）」「换一张」「清除舞台背景图」，也没有 kBackgroundAlignPadKey。有库时有「添加图片」。背景项不少于 2 时才有「轮播」。有这句文案：「当前这一项同时铺在舞台上。选了内置图案时，舞台回到纯色，壳仍画图案。」界面透明度滑杆仍在背景块底部。
5. 发丝线不乘强度：暗色 alpha 0.12，浅色 0.10。AppMaterial 只有 radiusScale 与 uiTransparency。
6. ShellBackdrop 的四档铺法、遮罩、九宫格、tileScaleFor 仍是渲染函数。测试要直接把 fit/tileSize 传给 ShellBackdrop，不要再从 DisplayPrefs 字段读取。外壳传 defaultTileSize / fitCover / defaultImageAlign / defaultBackgroundScrim / defaultBackgroundBlur。
7. _clearTransientResults 清 _shellImageMessage 与 _shellImageFailed，不再有 _stageImageMessage。
8. Live2DStage 上 sendStageBg(widget.stageImage) 保持不动。不要改 asset_guard_stage_attach_resend_test.dart，除非它自己失败。

要改的测试文件：display_prefs_persist_keys_test.dart、display_prefs_test.dart、display_prefs_background_fit_test.dart、display_prefs_style_change_detection_test.dart、design_tokens_test.dart、setting_wiring_test.dart、transient_results_test.dart、shell_backdrop_test.dart、shell_backdrop_decode_cache_test.dart、appearance_background_library_test.dart、appearance_background_runtime_test.dart、appearance_background_style_test.dart。

禁止：改 crates/、wasm、persona 导入、mods.json、.env、live2d-ai.toml、AGENTS.md、docs 预算、docs/audit、动作滑条、口型、朗读高亮、音频播放。不要在仓库根目录跑 flutter analyze。不要把断言改成恒真。不要整文件删掉还不留行为断言。新字符串必须在字体子集内。

验收（必须真跑，贴出各自最后 30 行）：

cd /home/skystar/Live2D-Ai/shell/flutter
/home/skystar/flutter/bin/flutter analyze
/home/skystar/flutter/bin/flutter test

两条都要 exit 0。没有浏览器，不要声称做过点击验收。做不到就写出卡住的断言，不要改产品方案。
```
