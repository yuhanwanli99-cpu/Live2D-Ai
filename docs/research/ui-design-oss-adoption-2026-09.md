# Flutter Web 前端「开源可复用件与许可合规」调研（2026-09）

> 调研员：开源可复用件与许可合规调研员（只读 + 一次性产物）
> 日期：2026-09-10
> 范围：`shell/flutter/`（Flutter Web 外壳：Live2D 舞台 + 聊天 + 设置/外观控制）
> 产物：本文件（**只写这一个文件，不改仓库其他任何文件**）
> 纪律：每条候选给出可核实的许可结论与维护状态；【已验证】= 有 URL / 本机实测支撑，【推断】= 调研员判断；核不到写「未核实」，不猜。
> 说明：本项目**不需要** Flutter 侧任何 Live2D 渲染库（渲染在 iframe 里的 Rust/wasm 渲染面），本报告不涉及任何 Live2D 渲染包。

---

## 0. 结论先行

1. **建议本轮不新增任何运行时依赖**：`pubspec.yaml` 维持 `flutter` + `http` + `web`（dev 仅 `flutter_test` + `flutter_lints`）。
   8 个方向里 **6 个**（消息渲染/音频可视化/图标/动效/玻璃拟态/基础控件）用 **Material 3 内建 + 少量自绘 CustomPainter** 即可覆盖；第 4 个（**字体**）是「自托管 OFL 子集字体资产」的工程问题，不是加包能解决的；第 7 个（**UI 壳**）的结论是「只借鉴结构，不引入依赖」。
2. **最值得引入的不是包，而是一份字体资产策略**（见 §5）：Flutter Web 在遇到中文字形时会**自动去 `fonts.gstatic.com` 下载 Noto 兜底字体**（证据链见 §5.1，官方 issue 报告单次 6.3 MB），这与本项目「本地优先、不依赖 Google CDN、必须 `--no-web-resources-cdn`」的治理约束直接冲突。**推荐方案 = 自托管 OFL 子集 woff2**，首选**霞鹜文楷 Lite**（其 OFL 附加许可明文允许子集化并保留字体名），次选 Noto Sans SC（**须改内部字体名**，因其 RFN 为 `'Source'`）。
3. **最高风险的两个候选已标红拒绝**：`flutter_markdown`（已被 pub.dev 标记 discontinued，官方**未接手**，由第三方 `flutter_markdown_plus` 接续）、`google_fonts`（运行时 HTTP 拉取 gstatic）。
4. **官方接手问题的答案**：`flutter_markdown` 已 discontinued（pub.dev 原文「This project has been discontinued」，并指向第三方替代），`flutter/packages` 仓库 `packages/` 目录下**已无 markdown 相关包**（41 个包中命中 0 个），`flutter-team-archive` 组织 17 个仓库中**无 markdown 仓库** → **没有被 Flutter 官方接手**。
5. 「玻璃拟态」在本项目主场景（Live2D 舞台之上）**技术上无效**：iframe 属于 web platform view，不参与 Flutter 场景合成，`BackdropFilter` **模糊不到它**，却照付离屏渲染代价（§7）。
6. 可复用的开源 Flutter 桌面 AI 壳：**有可借鉴者，无值得整壳引入者**；桌宠/Live2D 形象层**没有** Flutter 开源可复用件，必须自建（§8）。

**最终建议引入清单（新增运行时依赖）：无。**
**条件性可选（仅在触发条件成立时才引入）**：`material_symbols_icons`（缺图标/需要 fill·weight 轴）、`flex_color_picker`（需要自由取色）、`flutter_svg`（必须矢量描边精确控制且愿意自管 SVG 资产）。三者许可均为可合并进 AGPL 的宽松许可，且都通过 Web + wasm 平台标记。

---

## 1. 方法与基线

### 1.1 本机环境（所有实测结论的运行环境）

| 项 | 值 |
|---|---|
| Flutter | 3.47.3 stable（2026-09-04，engine `06a2e2a110`） |
| Dart | 3.13.3 |
| 构建 | `flutter build web --release`（web renderer：js→canvaskit；另可 `--wasm`→skwasm） |
| 现有依赖（`shell/flutter/pubspec.yaml` 实读） | `http ^1.6.0`、`web ^1.1.1`；dev：`flutter_test`、`flutter_lints ^6.0.0`；`uses-material-design: true` |

### 1.2 许可判定基线（AGPL-3.0-only 项目）

仓库自有代码为 **AGPL-3.0-only**（根 `LICENSE`），第三方保留各自许可（`LICENSE-SCOPE.md`）。判定依据（FSF 原文）：

| 许可 | 与 AGPL-3.0-only 合并 | 依据（原文摘录 + URL） |
|---|---|---|
| MIT (Expat) / BSD-2 / BSD-3 | ✅ 可以 | 「This is a lax, permissive non-copyleft free software license, compatible with the GNU GPL.」[gnu.org license-list#Expat](https://www.gnu.org/licenses/license-list.html#Expat) |
| ISC（Lucide 图标用的就是 ISC） | ✅ 可以 | 「It is a free software license, and compatible with the GNU GPL.」[#ISC](https://www.gnu.org/licenses/license-list.html#ISC) |
| Apache-2.0 | ✅ 可以（**注意不是 GPLv2**） | 「This is a free software license, compatible with version 3 of the GNU GPL. Please note that this license is **not compatible with GPL version 2**…」[#Apache2](https://www.gnu.org/licenses/license-list.html#Apache2)；FSF 兼容性页：「you can include source code under the GNU GPL version 3 together with other source code under the GNU Affero GPL… the effect is that the GNU AGPL applies to the combined program.」[license-compatibility](https://www.gnu.org/licenses/license-compatibility.html) |
| GPL-3.0-only | ✅ 可以（结果整体 AGPL-3.0） | 同上 FSF 兼容性页（GPLv3 与 AGPLv3 明文互认） |
| **GPL-2.0-only** | 🚩 **不可以** | GPLv2-only 无法升级到 GPLv3/AGPLv3，且与 Apache-2.0 亦不兼容（FSF 兼容性页） |
| **SSPL / Commons Clause / CC-BY-NC / 自定义非商业 / 无 LICENSE** | 🚩 **不可以** | 非自由许可或「保留全部权利」 |
| SIL OFL-1.1（字体） | ✅ 可以（**聚合关系，非合并**） | OFL FAQ 1.2：「Fonts licensed under the OFL can be freely included alongside other software under FLOSS licenses. Since fonts are typically **aggregated with, not merged into**, existing software, there is little need to be concerned about incompatibility with existing software licenses.」[openfontlicense.org/ofl-faq](https://openfontlicense.org/ofl-faq/) |

**Apache-2.0 的额外义务（合规动作，不是禁止）**：保留版权与许可声明；被修改文件需加醒目变更说明；若上游带 `NOTICE` 需随附（仓库已有根 `NOTICE`）。
**OFL 的额外义务**：分发字体资产时随附 OFL 许可文本（OFL 条件 2；条件 5 要求「字体须整体以 OFL 分发」）；**若做了子集化/格式转换（= OFL 语境下的 Modified Version）**，需要遵守字体声明的 Reserved Font Name（RFN）改名规则 —— RFN 规定在 **OFL 条件 3**（不是条件 5），OFL FAQ 2.5 明确：「these optimized versions are Modified Versions and so must follow OFL requirements like appropriate renaming」。
**「只借鉴结构不抄代码」**：版权保护表达不保护思想/功能，净室独立实现不构成侵权；但不得搬运代码、注释、命名、图标/字体/模型/文案。

### 1.3 pub.dev / GitHub 数据的读取方式

- 维护状态与许可标签统一取自 **pub.dev API**：`/api/packages/<pkg>`（版本、发布时间、`isDiscontinued`/`replacedBy`）与 `/api/packages/<pkg>/score`（`license:*`、`platform:*`、`is:wasm-ready`、`is:dart3-compatible`）。
- 仓库级许可取自 **GitHub API** `/repos/{owner}/{repo}` 的 `license.spdx_id`，必要时回读 LICENSE 原文。
- 本机实测：在 `/tmp/probe` 新建一次性工程（**不在仓库内**）验证图标 tree-shaking 与构建失败行为。

---

## 2. 方向 1：聊天消息渲染（Markdown / 代码高亮 / 打字机）

### 2.1 产品前提复核

`AGENTS.md` 的语音约定是「一句一单元、不断句」；`ChatController`（`shell/flutter/lib/chat/chat_controller.dart`）已经把流式片段拼进 `ChatMessage.text`，UI 侧 `main.dart` 只用 `Text(message.text)` 渲染（`main.dart:473`）。也就是说：**当前产品形态下 LLM 回复是口语短句，没有任何 Markdown 渲染需求**。引入 Markdown 解析器属于「为不存在的需求增加解析开销 + 依赖 + 许可面」。

### 2.2 候选核实

| 候选 | 版本 / 最后发布 | 许可（来源） | 维护状态 | 结论 |
|---|---|---|---|---|
| `flutter_markdown` | 0.7.7+1 / 2025-05-06 | BSD-3-Clause（[pub](https://pub.dev/packages/flutter_markdown)） | 🚩 **`isDiscontinued=true`，`replacedBy=flutter_markdown_plus`**；pub 页面 HTML 实读含「This project has been discontinued」与「This package is discontinued, but author has suggested package:flutter_markdown_plus as a replacement」 | 🚩 **REJECT（已废弃）** |
| `flutter_markdown_plus` | 1.0.12 / 2026-07-10 | BSD-3-Clause（pub score 标签；[repo](https://github.com/foresightmobile/flutter_markdown_plus) 亦 BSD-3-Clause） | 活跃（publisher `foresightmobile.com`，69★） | **REJECT（无需求；且为第三方接续项目，非官方）** |
| `markdown_widget` | 2.3.2+8 / 2025-04-26 | MIT（[repo](https://github.com/asjqkkkk/markdown_widget)） | 458★，仓库最后 push 2026-01-17 | REJECT（无需求） |
| `gpt_markdown` | 1.2.1 / 2026-08-23 | BSD-3-Clause | 活跃（179★） | REJECT（无需求） |
| `flutter_highlight` / `highlight` | 0.7.0 / **2021-03-07** | 未核实（pub 未给 license 标签） | 🚩 **`sdk: >=2.12.0 <3.0.0` → 与 Dart 3.13 不可解析** | 🚩 **REJECT（Dart 3 不兼容）** |
| `re_editor`（代码编辑器） | 0.10.0 / 2026-07-01 | MIT + web/wasm | 活跃，130/160 | REJECT（无需求） |
| `animated_text_kit`（打字机等） | 4.3.0 / 2025-10-04 | MIT | pub 包归档 10.5 MB（夹带 GIF 演示） | REJECT（无需为打字机引包） |
| `typewritertext` | 3.0.9 / 2024-09-14 | MIT | 近 2 年未发版 | REJECT（同上） |

### 2.3 打字机效果的正确做法（不引包）

- 已有 `streaming` 状态与增量文本，**自建 30~50 行**即可：`AnimationController`/`TweenAnimationBuilder<double>` 驱动「已显示字数」，或用 `AnimatedSwitcher` + 渐变上屏。
- 【推断】**产品级注意**：打字机揭示速度应与「整句合成完成 → 开始播放」的时序对齐（见 `AGENTS.md` 语音约定）。文本早于语音太多会破坏「一句一单元」的观感；建议「整句到达即整句上屏 + 轻量渐入」，而不是逐字符抢跑。
- 【推断】副作用提醒：每帧改文本会触发 `Text` 重排（layout），应在 `RepaintBoundary` 内并限制在 ~30fps 以内。

**判定：方向 1 全部 REJECT，自建渐入/打字机（0 依赖）。**

---

## 3. 方向 2：音频可视化（波形 / 频谱 / 音量电平）

### 3.1 不引包也能拿到 `AnalyserNode`（【已验证】）

- `package:web` 1.1.1（BSD-3-Clause）**已声明** `AnalyserNode`（`fftSize` / `frequencyBinCount` / `smoothingTimeConstant` / `minDecibels` / `maxDecibels`；`getByteTimeDomainData(JSUint8Array)` / `getByteFrequencyData(JSUint8Array)` / `getFloatTimeDomainData(JSFloat32Array)`）：[AnalyserNode](https://pub.dev/documentation/web/latest/web/AnalyserNode-extension-type.html)、[BaseAudioContext.createAnalyser](https://pub.dev/documentation/web/latest/web/BaseAudioContext/createAnalyser.html)、[AudioNode.connect](https://pub.dev/documentation/web/latest/web/AudioNode/connect.html)。
- `JSUint8Array.withLength()` / `.toDart → Uint8List`： [dart:js_interop](https://api.dart.dev/stable/3.13.1/dart-js_interop/JSUint8Array-extension-type.html)。
- 项目落点现成：`shell/flutter/lib/audio/audio_player.dart` 已在用 `web.AudioContext` / `GainNode` / `AudioBufferSourceNode`，只需在 `_ensureContext` 建 `context.createAnalyser()`，把 `node.connect(_gain)` 改为 `node.connect(analyser)` + `analyser.connect(_gain)`。改动约 40~60 行绑定 + 60~100 行 `CustomPainter`。
- 【已验证】`AnalyserNode` 即使输出不连接也能工作（[MDN](https://developer.mozilla.org/en-US/docs/Web/API/AnalyserNode)）；`MediaElementAudioSourceNode`（`<audio>` 元素）受 CORS 限制，跨源资源规范强制输出静音（[Web Audio 规范](https://webaudio.github.io/web-audio-api/#MediaElementAudioSourceOptions-security)）——本项目走 `AudioBufferSourceNode`（PCM 内存缓冲）+ 同源，无此问题。
- 【推断，与治理红线一致】analyser **放在 `GainNode` 之前**，静音（gain=0）时仍能读到电平 → 可用于「正在发声」状态机，而**不**影响「静音不关口型」的不变量。若要显示「用户实际听到的音量」，应在 `GainNode` 之后另插一个 analyser 或把电平乘以当前 gain（两条语义不要混用一个数据源）。

### 3.2 现成包核实（许可 / 维护 / Web / 是否适配「实时播放流」）

| 包 | 版本 / 最后发布 | 许可 | Web | 适配实时播放流 | 结论 |
|---|---|---|---|---|---|
| `audio_waveforms` | 2.0.2 / 2026-01-09 | MIT | ❌ 平台仅 `android,ios` | ❌ 录音/文件波形 | REJECT（Web 不可用） |
| `just_waveform` | 0.0.7 / 2025-03-30 | MIT | ❌ 平台 `android,ios,macos` | ❌ 走文件路径 | REJECT（Web 不可用） |
| `sound_stream` | 0.4.2 / 2025-09-27 | **GPL-3.0** | ❌ 平台仅 `android,ios` | ❌ | 🚩 REJECT（Web 不可用；许可亦无需碰） |
| `flutter_audio_waveforms` | 1.2.1+8 / **2023-02-12** | MIT | ✅ 标记支持 | ❌ 纯 UI，需外部预生成 samples | REJECT（停更 + 不适用） |
| `flutter_soloud` | 5.0.2 / 2026-09-04 | MIT（引擎 ZLib/LibPNG） | ✅ 但需 `index.html` 注入初始化脚本；AudioWorklet 需 COOP/COEP | 可用，但要替换整套播放引擎 | REJECT（代价远大于收益，见下） |
| `audio_flux` | 2.0.0 / 2026-09-05 | MIT | ✅ | 可视化现成但强依赖 soloud + recorder + shader_buffers | REJECT（依赖链过重） |
| `record`（麦克风） | 7.1.1 / 2026-06-29 | BSD-3-Clause | ✅（amplitude/dBFS） | 仅麦克风电平 | HOLD（自建 `getUserMedia` 更省） |
| `flutter_recorder` | 2.0.3 / 2026-09-04 | Apache-2.0 | ✅ wasm | 麦克风 wave/FFT/volume；依赖 ffi/hooks/native_toolchain_c | REJECT（引入工具链级依赖） |

【已验证】`flutter_soloud` 会引入 `native_toolchain_c`/`ffi`/`hooks`/`code_assets` 一类构建期依赖并要求改 `index.html`（[web 注记](https://docs.page/alnitak/flutter_soloud_docs/get_started/web_notes)、[许可](https://docs.page/alnitak/flutter_soloud_docs/get_started/license)）；收益仅是现成 FFT，对一块 60fps 电平表不必要。

### 3.3 判定

**REJECT 全部波形/频谱包；自建 `AnalyserNode` + `CustomPainter`（0 新依赖、0 新许可）。**
【推断】实现约束：只在「说话中 / 聆听中」状态重绘；`RepaintBoundary` 包裹；绘制层控制在数十个 bar；不要用 `BackdropFilter` 做发光底（见 §7）。

---

## 4. 方向 3：图标集

### 4.1 关键机制：Web 上图标字体是 tree-shake 的（本机实测，【已验证】）

在 `/tmp/probe`（Flutter 3.47.3）实测 `flutter build web --release`：

| 场景 | 构建日志 / 产物 |
|---|---|
| 基线（1 个 const `Icons.send`） | `MaterialIcons-Regular.otf` 由 **1,645,184 → 7,800 字节**（-99.5%）；`CupertinoIcons.ttf` 257,628 → 1,472 字节；`build/web/assets/fonts/` 只剩这两个文件 |
| proj + `material_symbols_icons`，仅用 1 个 const `Symbols.send` | `MaterialSymbolsOutlined.ttf` **10,643,396 → 3,504 字节**；`Rounded` **15,073,408 → 2,620 字节**；`Sharp` **8,841,308 → 2,392 字节**；`assets/` 合计 1.4 MB、字体合计 40 KB |
| **非 const `IconData`**（`final IconData x = IconData(...)`） | 🚩 **构建硬失败**：`This application cannot tree shake icons fonts. It has non-constant instances of IconData at the following locations… Target web_release_bundle failed`（exit 1） |

配套文档/issue 证据：
- 官方 issue **[#154986 [web] Icon tree shaking doesn't work well on web](https://github.com/flutter/flutter/issues/154986)**（open，P2，3.24/3.26 复现）正文即上述报错；**web 构建对非 const `IconData` 是硬失败而非"体积变大"**。
- `material_symbols_icons` README 自述：动态查表 API `Symbols.get(name)` / `symbols_map.dart` **必须用 `--no-tree-shake-icons`**（[README](https://github.com/timmaffett/material_symbols_icons) 原文：「When using the get() method *tree-shaking must be turned off* using `--no-tree-shake-icons`」）——在本项目等于**放弃字体裁剪 + 触碰 Web 构建红线**。
- 内置图标其实很多：`~/flutter/packages/flutter/lib/src/material/icons.dart` 计 **8,825 个 `IconData` 常量**，其中 `_outlined` **2,194**、`_rounded` **2,200**、`_sharp` **2,200**（本机 grep 计数）→ 「统一线宽/描边风格」用 `Icons.*_outlined` 即可满足大部分需求。

### 4.2 候选核实

| 候选 | 版本 / 最后发布 | 许可 | 维护状态 | 结论 |
|---|---|---|---|---|
| **Material 3 内建 `Icons`** | 随 SDK（3.47.3） | BSD-3-Clause（Flutter SDK） | 官方 | ✅ **ADOPT（默认方案，0 依赖）** |
| `material_symbols_icons` | 4.2960.0 / 2026-07-23 | **Apache-2.0**（GitHub API：repo `Apache-2.0`；字体来自 Google `material-design-icons` 亦 Apache-2.0） | 仓库最后 push 2026-07-23，67★，单一维护者；pub 包归档 **34.5 MB**（3 套可变字体 10.2/14.4/14.4 MB） | **HOLD（条件性 adopt）**：需要 fill/weight/光学尺寸轴或内置缺失图标时可用；**只能用 const `Symbols.x`**；代价是 pub 缓存 34 MB（不进产物） |
| `lucide_icons` | 0.257.0 / **2023-06-29** | pub 未给 license 标签（图标本体 ISC，见下） | 🚩 近 3 年未发版 | 🚩 REJECT（停更） |
| `lucide_icons_flutter` | 3.1.19 / 2026-09-07 | MIT（pub score） | 活跃，第三方 | HOLD（若坚持 Lucide 风格，选它；单一第三方维护者风险） |
| `flutter_lucide` | 1.43.0 / 2026-09-08 | MIT（[repo](https://github.com/ravikovind/flutter_lucide)） | 活跃，25★ | REJECT（与上者重复，生态碎片化） |
| `lucide_flutter` | 1.42.0 / 2026-09-07 | MIT | 活跃 | REJECT（第三个重复包装） |
| `phosphor_flutter` | 2.1.0 / **2024-05-10** | MIT（[repo](https://github.com/phosphor-icons/flutter)，151★，最后 push 2026-01-06） | 近 1 年半未发版 | REJECT（与内置/Material Symbols 重复） |
| `icons_plus` | 5.0.0 / 2023-12-15 | pub 未给 license 标签 | 🚩 停更 | 🚩 REJECT（许可未核实 + 停更） |
| `font_awesome_flutter` | 11.0.0 / 2026-03-09 | 包 MIT（note：**免费图标 CC BY 4.0、字体 SIL OFL 1.1**，需按上游规则署名/随附许可 —— 未逐项核实） | 活跃 | REJECT（风格不匹配 + 许可面更复杂） |
| `line_icons` | 2.0.3 / 2023-07-05 | **GPL-3.0**（pub score） | 停更 | REJECT（停更；GPL-3.0 虽可并入 AGPL，但无必要） |
| `hugeicons` / `solar_icons` / `remixicon` / `bootstrap_icons` / `eva_icons_flutter` | 2026 年内有发布 | MIT / BSD-3 / MIT / MIT / MIT | 活跃度不一 | REJECT（重复建设；仅作风格备选） |
| `flutter_svg` (+`vector_graphics`) | 2.3.0 / 2026-05-08 | MIT（flutter favorite） | 官方生态活跃，web+wasm | HOLD（**仅当**必须像素级控制描边宽度且愿意自管 SVG 资产；Lucide 图标本体 ISC，可安全内置少量 SVG） |

【已验证】Lucide 图标本体许可 = **ISC**（`lucide-icons/lucide` 的 LICENSE 原文首行「ISC License / Copyright (c) 2026 Lucide Icons and Contributors」；GitHub API 因多版权持有人报 `NOASSERTION`，以原文为准）。

### 4.3 判定

**默认 ADOPT 内建 `Icons`（含 `_outlined`/`_rounded` 变体，Apache/BSD 许可、0 依赖、实测裁剪到 ~8 KB）。**
**HOLD `material_symbols_icons`**：只有出现「内置没有的图标」或「需要 weight/fill 轴」时才引入，且**硬性要求**：全部使用 const `Symbols.x`，禁止 `Symbols.get()` / `symbols_map.dart`（否则 Web release 构建失败或必须关掉字体裁剪）。
**REJECT 所有 Lucide/Phosphor 等第三方包装**（4 个 Lucide 包装并存 = 生态碎片化；真实描边需求可用内置 outlined 覆盖）。

---

## 5. 方向 4：中文字体（许可 + 体积 + 明确策略）

### 5.1 这是本项目最容易被忽略的「隐形 CDN 依赖」

【已验证】Flutter Web（CanvasKit）在渲染**未被内置字体覆盖的字形**（汉字、emoji 等）时，会**自动从 `fonts.gstatic.com` 下载 Noto 兜底字体**。证据链：

- 引擎源码 `lib/web_ui/lib/src/engine/canvaskit/font_fallbacks.dart` 内含 `_notoSansSC` / `_notoSansTC` / `_notoSansHK` / `_notoSansJP` / `_notoSansKR` / `_notoSymbols` 与 `FallbackFontDownloadQueue`（[engine 源码](https://github.com/flutter-team-archive/engine/blob/2393e69c88f56765763cb65de98a7e1846d112c9/lib/web_ui/lib/src/engine/canvaskit/font_fallbacks.dart)）。
- issue **[#163823 flutter web: use the device's default font instead of loading the large font file from fonts.gstatic.com](https://github.com/flutter/flutter/issues/163823)**（**open**，P2，2025-02-21 建，2025-10-07 仍有更新）正文：使用 `flutter build web` 时「the web app will load a **6.3MB notosanssc font file** (a Chinese font) from fonts.gstatic.com」。
- 该 issue 评论（2025-08-28）：**即使把所有默认字体与 fallback 都设好，只要出现未覆盖字符，仍会去 gstatic 拉 notosanssc**；且「每次切路由/重建 widget 都会重新请求」「请求失败会无限重试」。
- issue **[#184449 [web] Font fallback download failure causes infinite loop](https://github.com/flutter/flutter/issues/184449)**（P1，2026-04-01 建，2026-05-12 **closed/completed**）——失败无限循环已被修复，但**下载行为本身仍在**。
- 社区可行 workaround（issue 评论 2025-10-07）：在自定义 loader 里拦截 `url.includes('fonts.gstatic.com')` 并改写为本地字体文件。
- 官方长期方案 **[#172561 [web] ☂️ WebParagraph](https://github.com/flutter/flutter/issues/172561)**（open，P1）：目标是「**Eliminate fallback fonts**… They were needed because we didn't have access to system/browser fonts」。【已验证】本机 3.47.3 构建产物里确有 `build/web/canvaskit/webparagraph/canvaskit.wasm`，`flutter_bootstrap.js` 内亦有 `preferWebParagraph` + `window.TextCluster` 判定；但【已验证】`flutter_tools` 源码中**搜不到** `preferWebParagraph`（`grep` 无命中）→ 当前无官方开关，属未启用路径，**不作为方案**。

**结论**：在 Flutter Web 上「依赖系统字体回落」**不成立** —— CanvasKit 拿不到设备系统字体，所谓回落是「去 Google CDN 下载 Noto」。对本项目（本地优先、`--no-web-resources-cdn`、断网即白屏不可接受）**必须自托管中文字形**。

### 5.2 体积实测（本机实测 + 上游 release 数据）

| 字体 | 版本 | 实测体积 | 来源 |
|---|---|---|---|
| Noto Sans SC（Google Fonts，woff2，**单字重 400**，按 `unicode-range` 分 101 片） | v40 | **合计 2,354 KB / 101 片**（单片 1.4 KB ~ 40 KB）；本机逐片下载求和 | 实测 2026-09-10，`fonts.googleapis.com/css2?family=Noto+Sans+SC:wght@400` |
| ↳ 同上，`wght@400;700` 两字重 | v40 | **合计 4.31 MiB / 101 片** | 同上（`css2?family=Noto+Sans+SC:wght@400;700`） |
| ↳ 同上，可变字体 TTF（google/fonts 源文件） | — | **16.9 MiB**（`NotoSansSC[wght].ttf`） | google/fonts `ofl/notosanssc` |
| 思源黑体 Source Han Sans | 2.005R / 2025-06-18 | 区域包 `09_SourceHanSansSC.zip` **90.77 MB**；`05_SourceHanSansSubsetOTF.zip` **165.07 MB**；单字重 SC Regular OTF **15.8 MiB**、`SubsetOTF/CN` Regular **8.0 MiB**（7 字重 ≈ 56 MB） | [releases](https://github.com/adobe-fonts/source-han-sans/releases) |
| 霞鹜文楷 LXGW WenKai（**单字重 TTF**） | v1.522 / 2026-03-17 | Regular **24.39 MB**、Medium 24.20 MB、Light 26.96 MB；Mono 系列 24.27~26.98 MB；**Lite Regular 13.2 MiB** | [releases](https://github.com/lxgw/LxgwWenKai/releases) |
| Maple Mono（等宽，含 CN） | v7.9 | `MapleMono-CN.zip` 140.8 MB（多字重打包，单文件未量） | [repo](https://github.com/subframe7536/maple-font) |
| Flutter Web 触发 CJK 兜底时下载量 | — | **6.3 MB**（notosanssc，用户实测报告） | [#163823](https://github.com/flutter/flutter/issues/163823) |
| 【已验证】Flutter Web 是否支持 woff2 资产字体 | — | 可以：issue **[#128485 [web] Canvaskit doesn't support WOFF2 fonts](https://github.com/flutter/flutter/issues/128485)** 于 2024-10-23 以「Fixed in https://github.com/flutter/engine/pull/55908」关闭（`r: fixed`）；**[#109108 Support woff2 font](https://github.com/flutter/flutter/issues/109108) 仍 open（P3）是针对非 Web 平台** | 同上 |

> 【纪律说明】`woff2` 在 Flutter Web 的**运行时**可用性我只核到「issue 关闭原因 + 引擎 PR 链接」，**未做浏览器实测**，标记为「部分核实」。若采用 woff2 自托管方案，**必须在目标浏览器（Chrome/Firefox/Edge）实测首屏中文渲染**，并保留 TTF 兜底预案。

### 5.3 候选字体许可核实

| 字体 | 许可（可核实来源） | 关键限制 | 与 AGPL-3.0-only 项目分发 |
|---|---|---|---|
| **Noto Sans SC / Noto Sans CJK** | **SIL OFL-1.1**；Google Fonts 分发版的 OFL.txt 首行即声明 **RFN `'Source'`**（[google/fonts `ofl/notosanssc/OFL.txt`](https://github.com/google/fonts/blob/main/ofl/notosanssc/OFL.txt) 原文「Copyright 2014-2021 Adobe…, with Reserved Font Name 'Source'」；`METADATA.pb` 的 `copyright` 字段同文，且 `post_script_name` 为 `NotoSansSC-Thin`） | **子集化 = Modified Version → 不得沿用 `Noto Sans SC` / `NotoSansSC` 名（须改内部 name/PS name）**；不得单独售卖字体 | ✅ 聚合分发可用；随附 OFL 文本 + 改内名 |
| **思源黑体 Source Han Sans** | **SIL OFL-1.1**，**声明 RFN 'Source'**（[`release/LICENSE.txt`](https://github.com/adobe-fonts/source-han-sans/blob/release/LICENSE.txt) 原文「with Reserved Font Name 'Source'」） | 子集化后不得继续以 Source 名分发（OFL FAQ 2.5） | ✅ 可用（未修改则保留原名；子集化需改名） |
| **霞鹜文楷 LXGW WenKai（含 Lite / Screen 变体）** | **SIL OFL-1.1** + RFN（'霞鹜','霞鶩','落霞孤鹜','落霞孤鶩','LXGW'）+ **附加许可**（[`OFL.txt`](https://github.com/lxgw/LxgwWenKai/blob/main/OFL.txt) 原文：RFN「may continue to be used in Modified Versions… or in Modified Versions **subsetted or converted to other formats (e.g., WOFF/WOFF2) solely for web font delivery**，provided such Modified Versions are not made available as installable desktop fonts（如 Google Fonts 等平台）」；README 授权段亦声明允许嵌入 APP） | 不得以可安装桌面字体形式公开发布子集；不得单独售卖字体 | ✅ **明确允许子集化/woff2 转换并保留字体名**（改名成本最低的一种） |
| **得意黑 Smiley Sans** | **SIL OFL-1.1**（GitHub API `OFL-1.1`，[repo](https://github.com/atelier-anchor/smiley-sans)，14,815★），另有 RFN（`Smiley`/`得意黑`，见其 LICENSE） | 展示型标题字体，不适合正文；子集化需改名 | ✅ 可用（建议仅标题） |
| **Maple Mono（含 CN）** | **SIL OFL-1.1**，OFL.txt **无 RFN 声明**（[`OFL.txt`](https://github.com/subframe7536/maple-font/blob/variable/OFL.txt)） | 无 RFN → 子集化改名义务最轻 | ✅ 可用（等宽/日志场景；多字重包 140.8 MB，需自行裁剪） |
| **MiSans（小米）** | 🚩 **自定义条款**（[官方 FAQ](https://hyperos.mi.com/font/faq) 实读：「全球免费商用…**可以**用作嵌入式字体。但您应在软件中**特别注明**使用了 MiSans 字体」「您可以自由调节字体的粗细、间距等。但您**不得单独**将 MiSans 字体或其组件进行外观上的更改」；另有 MiSans 知识产权许可协议 PDF） | 强制署名 + 禁止外观更改 + 禁止单独再许可/分发/售卖；**子集化是否被允许未核实** | 🚩 **不建议**（自定义条款、非 OSI 认可、子集化合规性不明；需法务判断） |
| **HarmonyOS Sans** | 🚩 **自定义 HarmonyOS Sans Fonts License Agreement（非 OFL）**；本轮仅取到**二手镜像**的许可全文 | 允许嵌入/捆绑/再分发**未修改**副本；**明文禁止任何修改**；须在软件中显著声明；禁止单独分发售卖 → **子集化不可行**（[镜像证物](https://github.com/zhiyuan1i/fonts-harmonyos-sans-cn/blob/main/source/usr/share/doc/fonts-harmonyos-sans-sc/copyright)，**非官方一手来源，标记未完全核实**） | 🚩 **REJECT**（禁止修改 ⇒ 无法子集化；只允许整包嵌入 + 署名） |
| **阿里巴巴普惠体 / 站酷系列** | **许可未核实**（官方许可原文本轮未取得；官网 meta 仅称「永久免费正版商用」） | — | 🚩 不予评估/不使用（核不到就不引入） |

### 5.4 明确建议（自托管 vs 系统字体回落）

1. **明确的建议 = 自托管 OFL 子集字体（方案 A）**；**"系统字体回落"在 Flutter Web 上不可用，不要选**（§5.1）。
2. **推荐组合**：
   - 正文/UI（**推荐**）：**霞鹜文楷 Lite 或 LXGW WenKai（OFL-1.1）子集化 woff2** —— 其附加许可**明文允许子集化/woff2 转换并保留字体名**，改名成本最低；
   - 或者：**Noto Sans SC（OFL-1.1）子集化 woff2** —— 但 **RFN `'Source'` 已声明**（Google Fonts OFL.txt），**子集化后必须改内部 name/PS name**（例如 `Live2DAI Sans SC`），否则违反 OFL 条件 3；
   - 标题点缀：得意黑 Smiley Sans（OFL-1.1，RFN `Smiley`/`得意黑`，子集化需改名），仅子集化标题用字；
   - 等宽（代码/日志）：Flutter 内置等宽即可；若需中文等宽，**Maple Mono（OFL-1.1，无 RFN → 改名义务最轻）**优于 MiSans/鸿蒙。
3. **子集化范围**：UI 文案（有限字符集）+ 常用汉字表（GB2312 / 3500 常用字）+ 标点，**不含**生僻字。这样「首屏中文字体」目标量级在**数百 KB**（【推断】，必须实测；对照基准：Noto Sans SC 单字重全量 woff2 = 2,354 KB / 400+700 两字重 = 4.31 MiB、可变 TTF = 16.9 MiB、文楷全量 TTF = 24.4 MiB、文楷 Lite = 13.2 MiB）。
4. **合规动作**：随附 `OFL.txt`（OFL 条件 2）；在 `CREDITS.md`/`NOTICE` 记录字体来源与版本；**子集化后按 RFN 规则处理字体名**（Noto/思源：RFN `Source` → 改名；得意黑：改名；文楷：附加许可允许保留名；Maple Mono：无 RFN）。
5. **字体加载行为（决定了"首屏代价"）**：【部分核实】自托管字体会打进 asset bundle 并写入 `assets/FontManifest.json`（本机构建产物实读），启动路径会拉取该 manifest 并装载其中字体（本机在 `build/web/main.dart.js` 中定位到 `FontManifest.json` 的加载调用及其「Font manifest does not exist at …」报错分支）；官方 issue **[#108660](https://github.com/flutter/flutter/issues/108660)**（open，P3，2022 年至今）正是要求「把启动时加载全部字体改为按需加载」。→ **结论**：全量自托管 = 首屏全量下载，必须靠「子集化 + 只声明少量字体族/字重」控制。
6. **不要靠 `fontFamilyFallback` 解决中文**：【推断，基于 §5.1 机制】CanvasKit 取不到设备字体，`fontFamilyFallback: ['Microsoft YaHei', 'PingFang SC', …]` 这些名字在 Web 上**不会命中**，最终仍走引擎的 Noto/gstatic 兜底。保留该字段无害（为将来 WebParagraph 生效做准备），但**不能当成方案**。
7. **必须配套的兜底**：即使用了自托管字体，也要考虑**未覆盖字符仍会触发 gstatic 兜底下载**（issue #163823 评论明确说「未加载字符出现时 Noto 仍会被下载」）。因此建议二选一并写入文档：
   - (a) 在 `web/index.html` / 自定义 loader 中**拦截 `fonts.gstatic.com` 请求并改写为本地资源**（社区 workaround，【已验证 issue 评论有做法，未在本项目实测】）；
   - (b) 接受「生僻字/emoji 触发一次 gstatic 请求」，并在 `scripts/ignite.sh` 的 CDN 探测中显式标注这一例外（与 `--no-web-resources-cdn` 的严格程度对齐前需用户裁决）。
8. **REJECT `google_fonts`**：包 README 原文「**HTTP fetching at runtime**，ideal for development…」+「Font file caching, on device file system」（[README](https://github.com/flutter/packages/tree/main/packages/google_fonts)），与「本地优先 / 不依赖 Google CDN」冲突；且它用的是单文件完整字体，**没有 Google Fonts 的 `unicode-range` 分片能力**，中文场景等于把 2.3 MB+ 一次拉下来。
9. 【推断】Google Fonts 式的 `unicode-range` 分片**在 Flutter asset 字体上不成立**（asset 字体没有 CSS `unicode-range` 语义）；若要分片，只能走「自定义 CSS/`FontLoader` 手动按需加载」，复杂度显著上升 → 一期建议「整包子集 + 只声明 1~2 个字重」，不要一上来做分片。

**资产体积预算（【推断】，需实测确认）**：中文字体子集（正文 Regular + 标题/加粗 Medium）合计目标 **≤ 600 KB**；若实测超 1 MB，退回到「仅界面文案子集 + 常用 1000 字」。

---

## 6. 方向 5：动效（`flutter_animate` / `animations` / `rive`）

| 候选 | 版本 / 最后发布 | 许可 | 维护状态 | 关键发现 | 结论 |
|---|---|---|---|---|---|
| `flutter_animate` | 4.5.2 / **2024-11-25** | BSD-3-Clause（[repo](https://github.com/gskinner/flutter_animate)） | 约 22 个月未发版（1,110★） | 依赖 `flutter_shaders`（0.1.3 / 2024-09-26，BSD-3-Clause）；提供链式 DSL | REJECT（可用内建替代；依赖已停更） |
| `animations`（官方） | **2.2.0 / 2026-04-23**（依赖仅 flutter） | BSD-3-Clause（pub score） | 官方，web+wasm | 🚩 **3.0.0（2026-08-19）新增依赖 `material_ui ^1.0.0`** —— `material_ui` 是官方新拆出的**平行 Material 实现**（pub 1.2.0 / 2026-09-08），而本项目用 `package:flutter/material.dart`（SDK 自带 `src/material/*`，本机核实）→ **两个 Material 树并存**，`Theme`/`InheritedWidget` 身份不同，3.0.0 的 `OpenContainer` 等组件在旧 Material 主题下行为不可预期 | HOLD（如确需 `ContainerTransform`/`SharedAxis`，**必须锁 `animations: ^2.2.0`**，禁止 3.x） |
| `rive` | 0.14.11 / 2026-08-03 | 运行时 MIT（[repo](https://github.com/rive-app/rive-flutter)） | 活跃 | 依赖 `rive_native`；需要 Rive 编辑器（专有工具）产出 `.riv` 资产 + 独立设计流程；运行时体积与许可条款（编辑器/署名）**未核实** | REJECT（团队与资产管线不匹配） |
| `lottie` | 3.5.1 / 2026-07-08 | MIT | 活跃，web+wasm | 需引入 Lottie JSON 资产 + 播放器 | REJECT（同 Rive，收益不抵成本） |

**判定：不引入动效库。** 用内置隐式动画（`AnimatedContainer` / `AnimatedOpacity` / `AnimatedSwitcher` / `TweenAnimationBuilder`）+ 少量 `AnimationController`（面板开合、气泡入场，约 50~100 行/效果）即可。理由：本项目动效需求是「面板过渡 + 消息渐入 + 说话状态脉冲」，全部是一次性、局部、不需编排 DSL 的场景；引入 DSL 反而增加治理面（`flutter_shaders` 停更、`material_ui` 平行树）。

---

## 7. 方向 6：「玻璃拟态」/ 模糊背景（`BackdropFilter` 的真实代价）

### 7.1 官方原文与证据（【已验证】）

- 官方 API 文档：`BackdropFilter`「**This effect is relatively expensive, especially if the filter is non-local, such as a blur.**」，且「The filter will be applied to all the area within its parent or ancestor widget's clip. If there's no clip, the filter will be applied to the full screen.」[BackdropFilter](https://api.flutter.dev/flutter/widgets/BackdropFilter-class.html)
- 官方同页指引：只想模糊一个 widget 时应改用 `ImageFiltered`，「both easier to use and less expensive」，对 blur 这类复杂滤镜「the performance will be improved dramatically」。
- 性能最佳实践（saveLayer 语义）：「Calling `saveLayer()` allocates an offscreen buffer and drawing content into the offscreen buffer might trigger a render target switch… On mobile GPUs this is particularly disruptive to rendering throughput.」；并建议「Instead of wrapping simple shapes or text in an Opacity widget, it's usually faster to just draw them with a semitransparent color.」[docs.flutter.dev/perf/best-practices](https://docs.flutter.dev/perf/best-practices)
- 渲染器现状：「Flutter on the web currently uses Skia for rendering.」→ Web 端**没有 Impeller**，证据只能取 Skia 量级（[impeller](https://docs.flutter.dev/perf/impeller)）。
- 相关 issue：**[#189260 BackdropFilter on Firefox Linux has major performance degredation](https://github.com/flutter/flutter/issues/189260)**（open，platform-web，P3）；**[#89889 BackdropFilter disrupted by non-overlapping empty platform view in Flutter Web](https://github.com/flutter/flutter/issues/89889)**（open，`e: web_canvaskit`）。
- **对本项目决定性的一条**：**[#184996](https://github.com/flutter/flutter/issues/184996)** 报告「PlatformView-based content (WebView, native video views) **passes through completely un-blurred**… no workaround」，官方以 duplicate 关闭。本项目的 Live2D 舞台正是 `HtmlElementView` + `<iframe>`（`shell/flutter/lib/live2d/live2d_host_web.dart`），官方文档示例同样是 `HtmlElementView.fromTagName(tagName: 'iframe')`（[web-content-in-flutter](https://docs.flutter.dev/platform-integration/web/web-content-in-flutter)）。
- 【推断】因此「在 Live2D 舞台之上盖一块磨砂玻璃面板」：**iframe 区域根本不会被模糊**，面板只会透出 Flutter 画布自己的底色 → 付了离屏+卷积代价却拿不到观感。
- 量级参考（**原生 Skia/Impeller，非 Web，Web 端无实测数据 = 未核实**）：桌面独显 36 块 σ=15 面板 Skia raster p50 **4.06 ms**（[#191207](https://github.com/flutter/flutter/issues/191207)）；Adreno 610 每 4 张卡 1 个 BackdropFilter，Skia p50 **12.9 ms**、**34% 帧超 16.7 ms**（[#192147](https://github.com/flutter/flutter/issues/192147)）。

### 7.2 替代方案与建议

| 方案 | 每帧代价 | 观感 / 限制 |
|---|---|---|
| `BackdropFilter` + blur | 高（离屏快照 + 卷积，∝ 面积 × σ × 面板数） | 真模糊，但**对 iframe 无效** |
| 同 + `ClipRect` + `BackdropGroup.grouped` | 中（共享 backdropKey，只算一次；重叠面板不可共用） | 同上 |
| 同 + `ImageFilterConfig.blur(bounded: true)` | 更可控（只采样本矩形、避免相邻串色） | 同上（[API](https://api.flutter.dev/flutter/rendering/ImageFilterConfig/ImageFilterConfig.blur.html)） |
| **半透明纯色 + 1px 高光 border + 阴影（`Color.withValues(alpha:)`）** | **近零** | 官方认可的最省替代；跨 canvaskit/skwasm 一致 |
| 静态预渲染模糊图 / 一次性 `toImage` 快照 | 近零 | 仅静态背景成立；背景是每帧变化的 Live2D 时会「糊错」 |

**判定：主场景（舞台之上）不使用 `BackdropFilter`。** 面板统一用「`Color.withValues(alpha: 0.10~0.16)` 半透明纯色 + `Colors.white.withValues(alpha: 0.08)` 1px 高光 border + 大范围低透明度阴影」。
注意：`Color.withOpacity` 已弃用（`@Deprecated('Use .withValues() to avoid precision loss.')`，[withOpacity](https://api.flutter.dev/flutter/dart-ui/Color/withOpacity.html) / [withValues](https://api.flutter.dev/flutter/dart-ui/Color/withValues.html)）。
若某处必须保留磨砂观感：限定在 **Flutter 自绘的静态背景区域**，并强制 `ClipRect` + `BackdropGroup` + `bounded: true`、σ ≤ 10，且禁止出现在滚动/动画列表中。

---

## 8. 方向 7：可复用的开源 UI 壳 / 组件库

### 8.1 候选核实

| 候选 | 类型 | 许可（来源） | 维护状态 | 结论 |
|---|---|---|---|---|
| [`flutter_chat_ui`](https://pub.dev/packages/flutter_chat_ui) | Flutter 聊天 UI SDK | Apache-2.0（pub score；[LICENSE](https://github.com/flyerhq/flutter_chat_ui/blob/main/LICENSE)） | 2.11.1 / 2025-12-11，web+wasm，2,335★ | **REJECT 作为依赖**（见下）/ **ADOPT 作为结构参考** |
| [`flutter_ai_toolkit`](https://pub.dev/packages/flutter_ai_toolkit) | 官方 `flutter/ai` 聊天脚手架 | BSD-3-Clause（[LICENSE](https://github.com/flutter/ai/blob/main/LICENSE)） | 1.0.0 / 2025-12-15；平台标记**无 windows/linux**（只有 android/ios/macos/web） | REJECT（默认依赖 `flutter_markdown_plus` + Firebase AI provider，与「禁 Markdown」「协议在 Rust 核心层」双重冲突） |
| [`mylxsw/aidea`](https://github.com/mylxsw/aidea) | Flutter 全平台 AI 聊天 App | MIT（[LICENSE](https://github.com/mylxsw/aidea/blob/main/LICENSE)） | 6,931★，最后 push 2026-03-04 | 仅设计参考（会话列表/设置页结构） |
| [`hooshyar/flutter_gen_ai_chat_ui`](https://github.com/hooshyar/flutter_gen_ai_chat_ui) | 轻量聊天气泡 | MIT | 85★，2026-09-03 活跃 | 仅参考（气泡/输入框） |
| [`flutter/samples`](https://github.com/flutter/samples)（`desktop_photo_search`、`material_3_demo`、`compass_app`） | 官方示例 | BSD-3-Clause（[LICENSE](https://github.com/flutter/samples/blob/main/LICENSE)，混合 OFL 字体/Apache-2.0，故 GitHub API 报 NOASSERTION） | 19,258★，push 2026-09-03，未 archive | ✅ **可复制代码**（需保留版权/许可声明）→ ADOPT 作为桌面布局参考 |
| [`flutter/gallery`](https://github.com/flutter-team-archive/gallery) | 官方示例 | BSD-3-Clause | 🚩 **已 archive**（迁至 `flutter-team-archive`），最后 push 2026-06-08 | 仅参考（Flutter 版本陈旧） |
| [`moeru-ai/airi`](https://github.com/moeru-ai/airi) | 虚拟形象 AI 伴侣 | MIT | 49,010★，活跃 | 🚩 **非 Flutter（TS/Vue/Tauri）** → 仅设计参考（多模态人设/记忆/打断流程） |
| [Open-LLM-VTuber](https://github.com/Open-LLM-VTuber/Open-LLM-VTuber) | AI VTuber | 代码 MIT（GitHub API 因附加条款报 NOASSERTION） | 13,692★ | 🚩 **非 Flutter（Python+Web）**；其 [LICENSE-Live2D.md](https://github.com/Open-LLM-VTuber/Open-LLM-VTuber/blob/main/LICENSE-Live2D.md) 将示例模型另置于 Live2D Free Material License（禁再分发）→ **模型资源绝对不可搬运**（与本项目「不捆绑模型」一致） |
| `lgnorant-lu/Ming-Status-mvp`（**是 Flutter 桌宠**） | Flutter 桌宠 | 🚩 **无 LICENSE = 保留全部权利** | 2★，2025-06-28 后停更 | 🚩 **REJECT（无许可不可引入，代码/素材均不可复制）** |
| 其余桌宠（duzexu/desktop-pet JS/GPL-3.0；TonyNa-code/desktop-pet Electron/MIT；NyaDeskPetAPP Kotlin/MIT） | — | — | — | 🚩 **非 Flutter**，不适用于 `/shell/flutter/` |
| [`stream_chat_flutter`](https://pub.dev/packages/stream_chat_flutter)（GetStream） | 聊天 SDK | 🚩 **许可未核实（pub score 标签 `license:unknown`）** | 10.4.0 / 2026-09-03 | 🚩 REJECT（许可不明 + 依赖后端服务） |
| 桌面窗口类：`window_manager`（MIT，仅 windows/linux/macos）、`flutter_acrylic`（MIT，仅桌面） | — | — | — | **N/A**：本项目 Flutter 只产出 Web 产物，窗口/透明/置顶由 Rust 桌面层负责，Flutter 侧不需要这些包 |

### 8.2 为什么 `flutter_chat_ui` 不引入（只借鉴）

【已验证】`flutter_chat_ui 2.11.1` 的依赖链：`provider`、`scrollview_observer`、`diffutil_dart`、`cross_cache`、`flutter_chat_core`（后者又依赖 `dio`、`freezed_annotation`、`intl`、`json_annotation`、`path_provider`）。
结合项目治理（前端只做展示与调度、核心协议在 Rust、实现保持最小、门禁 `flutter analyze`+`flutter test`）：
- 引入即**一次性增加 6+ 直接/间接依赖**，且 `provider` 会把状态管理范式带进来，与现有 `ChangeNotifier`（`ChatController`）风格重复；
- `path_provider` / `dio` 属于前端不需要的能力（历史消息、网络客户端都在 Rust 侧）；
- 收益（气泡+输入框+列表）在现有 `main.dart`（479 行）里已有雏形，自建成本约 200~300 行。

**判定：REJECT 作为依赖；ADOPT「结构借鉴」**（消息列表虚拟化/滚动锚定/输入区附件布局的交互写法）。同理，`flutter_ai_toolkit` 的 `LlmProvider` 抽象对本项目**没有价值**——LLM 协议与流式调度属于 Rust 核心层，Flutter 侧只消费 `/ws/state` 与 `/api/v1/chat`。

### 8.3 总判断

**「有可复用件，但没有值得整壳引入的依赖」**：结构参考 `flutter/samples`（BSD-3-Clause，可直接抄代码并保留声明）与 `flutter_chat_ui`（Apache-2.0，借鉴交互）；**Live2D 舞台层与桌宠形象层没有 Flutter 可复用件，必须自建**（AIRI / Open-LLM-VTuber 是 TS/Python 项目，仅作设计参考，且其模型/素材不可搬运）。

---

## 9. 方向 8：其他基础控件 —— Material 3 内建够不够

| 需求 | 内建是否够 | 结论 / 若不够的候选 |
|---|---|---|
| 滑杆（音量/口型参数） | ✅ `Slider`（M3 已更新外观，`SliderTheme`、`secondaryTrackValue` 可做「已缓冲/目标值」标记）、`RangeSlider` | **ADOPT 内建**；无需包（[M3 Slider breaking change](https://docs.flutter.dev/release/breaking-changes/updated-material-3-slider)） |
| 旋钮（Knob） | ❌ 无内建 | 【推断】自建 `CustomPainter` + `GestureDetector`（约 80~120 行）；pub 上 `super_slider` **不存在**（API 404 实测） |
| Toast / 轻提示 | ✅ `ScaffoldMessenger` + `SnackBar`（可自定义样式/时长） | **ADOPT 内建**；`toastification`（3.2.0 / 2026-04-19，BSD-3，web）/ `another_flushbar`（2.2.4，MIT，web）/ `fluttertoast`（10.0.0，MIT，平台仅 android/ios/web，无桌面）→ 均 REJECT（重复建设、`fluttertoast` 平台不含桌面） |
| 可折叠面板 | ✅ `ExpansionTile` / `ExpansionPanelList` / `AnimatedSize` | **ADOPT 内建**；`expansion_tile_group`（2.3.0，BSD-3，web）仅在需要手风琴互斥时可选 → HOLD（低优先） |
| 对话框 / 确认框 / 底部抽屉 | ✅ `showDialog` / `AlertDialog` / `showModalBottomSheet` / `showAdaptiveDialog` | **ADOPT 内建** |
| 颜色选择器（外观/主题色） | ❌ 无内建 | 【推断】优先**预设色板 + `SegmentedButton`**（0 依赖）；确需自由取色时：`flex_color_picker` 4.0.0 / 2026-08-30 / **BSD-3-Clause** / web+wasm / pub 160/160（[pub](https://pub.dev/packages/flex_color_picker)）→ **HOLD**；`flutter_colorpicker` 1.1.0 / 2024-05-19 / MIT → 次选 |
| 下拉 / 分段 / 标签页 / 提示气泡 | ✅ `DropdownMenu`、`SegmentedButton`、`TabBar`、`Tooltip`、`MenuAnchor` | **ADOPT 内建** |
| 状态管理 | ✅ 现有 `ChangeNotifier` + `ValueListenableBuilder` 已足够 | **REJECT** `provider`/`riverpod`/`bloc`（治理：依赖最小化；`flutter_chat_ui` 会顺带引入 provider，是拒绝它的理由之一） |
| SVG / 矢量资源 | ❌ 无内建 | `flutter_svg` 2.3.0 / 2026-05-08 / MIT / web+wasm（flutter favorite）→ **HOLD**（仅当自管 SVG 图标资产时） |
| 玻璃卡片 | — | 见 §7：**内建 `Container` + 半透明色即可**；`glass_kit` 4.0.2（MIT，web）→ REJECT（用 `BackdropFilter` 封装，继承同样代价）；`liquid_glass_renderer` 0.2.0-dev.4（MIT，平台**不含 web**）→ REJECT |

---

## 10. 标记汇总（红/黄）

🚩 **拒绝（许可或状态问题）**
1. `flutter_markdown` —— 已 discontinued（`replacedBy=flutter_markdown_plus`），官方未接手。
2. `flutter_highlight` / `highlight` —— `sdk <3.0.0`，与当前 Dart 3.13 不可解析。
3. `google_fonts` —— 运行时 HTTP 拉取 `fonts.gstatic.com`，违反本地优先 / `--no-web-resources-cdn` 治理。
4. `line_icons` —— GPL-3.0（可并入 AGPL 但无必要）+ 2023 年停更。
5. `sound_stream` —— GPL-3.0 + Web 不可用。
6. `stream_chat_flutter` —— **许可未核实**（pub `license:unknown`）。
7. `Ming-Status-mvp`（Flutter 桌宠）—— **无 LICENSE（保留全部权利）**，代码与素材均不可复制。
8. `MiSans` —— 自定义条款（免费商用且**明确允许嵌入式**，但强制署名 + 禁止更改字体外观 + 禁止单独再分发/售卖；子集化合规性不明）→ 不建议。
9. `HarmonyOS Sans` —— 自定义协议**明文禁止任何修改** ⇒ 无法子集化（只允许整包嵌入 + 显著署名），且本轮仅取到二手镜像证物 → 不使用。
10. `animations 3.0.0` —— 引入 `material_ui` 平行 Material 实现，与本项目 `package:flutter/material.dart` 主题体系并存风险（要用请锁 `^2.2.0`）。
11. `material_symbols_icons` 的动态查表用法（`Symbols.get()` / `symbols_map.dart`）—— 需 `--no-tree-shake-icons`，与 Web 构建红线冲突。
12. 任何**非 const `IconData`** —— 本机实测 Web release 构建**硬失败**（见 §4.1）。
13. `window_manager` / `flutter_acrylic` —— 平台不含 web，对本项目 N/A。

⚠️ **需人工确认 / 未核实**
- `woff2` 资产字体在 Flutter Web 的运行时表现（仅核到 issue #128485 关闭与引擎 PR 链接；**必须浏览器实测**）。
- HarmonyOS Sans 官方一手许可原文（现有证物为第三方 deb 包内 copyright 镜像）/ 阿里巴巴普惠体 / 站酷系列官方许可原文。
- `font_awesome_flutter` 图标 CC BY 4.0 与字体 OFL 的逐项署名要求。
- `rive` 编辑器与 `.riv` 资产的许可条款。
- Web 端 `BackdropFilter` 的实测帧耗时（仅有原生 Skia/Impeller 数据）。
- 自托管字体子集后的**实际字节数**（本报告只给了上游全量基准，未做子集化实测）。
- 已核实（原列为待确认，现结论）：Noto Sans SC 的 OFL.txt **确实声明 RFN `'Source'`**（Google Fonts 分发版）→ 子集化须改内部字体名。

---

## 11. adopt / reject 决策表（最终）

| # | 方向 | 候选 | 许可 | 维护状态 | 决策 | 一句话理由 |
|---|---|---|---|---|---|---|
| 1 | 消息渲染 | `flutter_markdown` | BSD-3（pub） | 🚩 discontinued | **REJECT** | 官方未接手、由第三方接续；产品禁用 Markdown |
| 2 | 消息渲染 | `flutter_markdown_plus` / `markdown_widget` / `gpt_markdown` | BSD-3 / MIT / BSD-3 | 活跃 | **REJECT** | 无解析需求；口语短句 + 禁用 Markdown |
| 3 | 消息渲染 | `flutter_highlight` / `highlight` | 未核实 | 🚩 Dart 3 不兼容 | **REJECT** | `sdk <3.0.0` 无法解析 |
| 4 | 消息渲染 | 打字机/动文本包（`animated_text_kit`、`typewritertext`） | MIT | 一般 | **REJECT** | 自建 30~50 行渐入即可；且需与「一句一单元」时序对齐 |
| 5 | 音频可视化 | `audio_waveforms` / `just_waveform` / `sound_stream` | MIT / MIT / GPL-3.0 | — | **REJECT** | 平台不含 web |
| 6 | 音频可视化 | `flutter_soloud` / `audio_flux` / `flutter_recorder` | MIT / MIT / Apache-2.0 | 活跃 | **REJECT** | 需换播放引擎 + 构建期原生工具链依赖 |
| 7 | 音频可视化 | **自建 `AnalyserNode` + `CustomPainter`** | 项目自有（`web` 为 BSD-3） | `package:web` 已含 `AnalyserNode` | ✅ **ADOPT** | 0 新依赖、0 新许可、与现有 WebAudio 播放器同图 |
| 8 | 图标 | **内建 Material 3 `Icons`（含 `_outlined`/`_rounded`）** | BSD-3（SDK） | 官方 | ✅ **ADOPT** | 8,825 个常量；实测裁剪到 ~8 KB、0 依赖 |
| 9 | 图标 | `material_symbols_icons`（仅 const `Symbols.x`） | **Apache-2.0** | 活跃（单一维护者） | ⏸ **HOLD（条件性 adopt）** | 需要 fill/weight 轴或内置缺失图标时；代价仅 pub 缓存 34 MB，产物实测裁剪到 ~3.5 KB |
| 10 | 图标 | `lucide_icons` / `flutter_lucide` / `lucide_flutter` / `lucide_icons_flutter` / `phosphor_flutter` / `icons_plus` / `line_icons` / `font_awesome_flutter` / `hugeicons` / `solar_icons` / `remixicon` / `bootstrap_icons` / `eva_icons_flutter` | MIT/ISC/BSD-3 为主；`line_icons` GPL-3.0；`icons_plus` 未核实 | 停更/重复/碎片化 | 🚩 **REJECT** | 内建 outlined 已够；4 个 Lucide 包装并存 = 生态碎片化，无稳定选择 |
| 11 | 图标 | `flutter_svg`（+`vector_graphics`） | MIT | 官方生态活跃 | ⏸ **HOLD** | 仅在必须像素级控制描边且愿意自管 SVG 资产时 |
| 12 | 字体 | **自托管 OFL 子集（首选 LXGW WenKai Lite；次选 Noto Sans SC·须改内名）** | **OFL-1.1**（LXGW 另含允许子集化/woff2 的附加许可；Noto/思源 RFN = `'Source'`） | 上游活跃 | ✅ **ADOPT** | 唯一能同时满足「许可干净」+「本地优先、不碰 gstatic」的方案 |
| 13 | 字体 | `google_fonts` | BSD-3（flutter/packages） | 活跃 | 🚩 **REJECT** | 运行时 HTTP 拉 gstatic，违反本地优先；无 `unicode-range` 分片能力 |
| 14 | 字体 | 全量 TTF 自托管（LXGW 24.39 MB/字重、Source Han SC 90.77 MB/包） | OFL-1.1 | 活跃 | 🚩 **REJECT** | 首屏体积不可接受 |
| 15 | 字体 | 「依赖系统字体回落」 | — | 🚩 **Flutter Web 不成立** | 🚩 **REJECT** | CanvasKit 取不到设备字体；所谓回落 = 去 gstatic 下载 Noto（6.3 MB） |
| 16 | 字体 | MiSans / HarmonyOS Sans / 阿里巴巴普惠体 / 站酷 | 自定义条款 / **未核实** | — | 🚩 **REJECT** | 自定义条款（强制署名、禁改外观）或许可未核实 |
| 17 | 动效 | `flutter_animate` | BSD-3 | 2024-11 后未发版 | **REJECT** | 内建隐式动画足够；依赖 `flutter_shaders` 亦停更 |
| 18 | 动效 | `animations` 官方 | BSD-3 | 官方活跃 | ⏸ **HOLD（必须锁 ^2.2.0）** | 3.0.0 引入 `material_ui` 平行 Material 树，与本项目主题体系冲突 |
| 19 | 动效 | `rive` / `lottie` | MIT（`rive` 条款未全面核实） | 活跃 | **REJECT** | 需专有编辑工具/额外资产管线与运行时，收益不抵成本 |
| 20 | 玻璃拟态 | `BackdropFilter`（舞台之上） | 内建 | — | 🚩 **REJECT** | iframe platform view 模糊不到（#184996 / #89889），代价照付 |
| 21 | 玻璃拟态 | **半透明纯色 + 1px 高光 border + 阴影** | 内建 | — | ✅ **ADOPT** | 官方认可的最省替代（`Color.withValues(alpha:)`） |
| 22 | 玻璃拟态 | `glass_kit` / `liquid_glass_renderer` | MIT / MIT | — | **REJECT** | 前者封装 `BackdropFilter`；后者平台不含 web |
| 23 | UI 壳 | `flutter_chat_ui` | Apache-2.0 | 活跃 | 🚩 **REJECT 作为依赖 / ADOPT 结构借鉴** | 拖入 provider/dio/path_provider 等 6+ 依赖，且协议在 Rust 核心层 |
| 24 | UI 壳 | `flutter_ai_toolkit` | BSD-3 | 活跃 | **REJECT** | 默认绑 `flutter_markdown_plus` + Firebase AI；平台标记缺 windows/linux |
| 25 | UI 壳 | `flutter/samples`（`desktop_photo_search` 等） | BSD-3-Clause | 官方，活跃 | ✅ **ADOPT（仅抄结构与少量代码，保留声明）** | 桌面三栏/面板布局可直接借鉴 |
| 26 | UI 壳 | `aidea` / `flutter_gen_ai_chat_ui` | MIT | 活跃度不一 | ⏸ **参考** | 仅交互参考，不引入 |
| 27 | UI 壳 | `stream_chat_flutter` | 🚩 未核实（`license:unknown`） | 活跃 | 🚩 **REJECT** | 许可不明 |
| 28 | UI 壳 | Flutter 桌宠仓库（含 `Ming-Status-mvp`） | 🚩 无 LICENSE | 停更 | 🚩 **REJECT** | 无许可 = 不可复制；且质量/维护不足 |
| 29 | UI 壳 | AIRI / Open-LLM-VTuber | MIT（模型另有限制） | 活跃 | ⏸ **仅设计参考** | 非 Flutter；模型/素材不可搬运 |
| 30 | 其他控件 | `Slider`/`SnackBar`/`ExpansionTile`/`AlertDialog`/`DropdownMenu`/`SegmentedButton`/`TabBar` | 内建 | — | ✅ **ADOPT** | M3 内建覆盖，0 依赖 |
| 31 | 其他控件 | 旋钮（Knob） | — | — | **自建** | 无内建、无合适包（`super_slider` 不存在） |
| 32 | 其他控件 | `flex_color_picker`（自由取色） | BSD-3-Clause | 活跃，web+wasm | ⏸ **HOLD** | 仅在需要自由取色时；优先预设色板 |
| 33 | 其他控件 | `flutter_colorpicker` | MIT | 2024-05 后未发版 | ⏸ **次要 HOLD** | 备选 |
| 34 | 其他控件 | `toastification` / `another_flushbar` / `fluttertoast` | BSD-3 / MIT / MIT | 活跃度不一 | **REJECT** | `SnackBar` 已够；`fluttertoast` 平台不含桌面 |
| 35 | 其他控件 | `expansion_tile_group` | BSD-3 | 活跃 | ⏸ **HOLD（低）** | 仅手风琴互斥场景 |
| 36 | 其他控件 | `provider` / `riverpod` / `bloc` | — | — | **REJECT** | 现有 `ChangeNotifier` 足够，依赖最小化 |
| 37 | 其他控件 | `window_manager` / `flutter_acrylic` | MIT | — | **N/A → REJECT** | 平台不含 web；窗口能力属 Rust 桌面层 |

---

## 12. 建议引入的依赖短清单（可直接执行）

**新增运行时依赖：无。**
`shell/flutter/pubspec.yaml` 维持：

```yaml
dependencies:
  flutter: {sdk: flutter}
  http: ^1.6.0
  web: ^1.1.1
dev_dependencies:
  flutter_test: {sdk: flutter}
  flutter_lints: ^6.0.0
```

**要新增的不是包，而是资产与自建模块**（全部 0 依赖）：

1. `shell/flutter/assets/fonts/`：**自托管 OFL 子集 woff2**（推荐 LXGW WenKai Lite —— 附加许可允许保留字体名；或用 Noto Sans SC 但**须改内部字体名**，因其 RFN 为 `'Source'`），随附 `OFL.txt`；体积预算 ≤ 600 KB（需实测）。
2. `web/index.html` / 自定义 loader：处理 `fonts.gstatic.com` 兜底请求（拦截改写或显式裁决例外），与 `--no-web-resources-cdn` 策略对齐。
3. `lib/ui/` 自建：`LevelMeter`/`WaveformPainter`（`AnalyserNode` + `CustomPainter`）、`GlassPanel`（半透明纯色 + 高光 border）、`Knob`、打字机/渐入 `AnimatedText`。
4. 图标：只用 const 内建 `Icons.*`（含 `_outlined`）；**新增图标前先确认可用 const `IconData`**，因为非 const 会让 `flutter build web --release` 直接失败。
5. 门禁照旧：`cd shell/flutter && flutter analyze && flutter test`；构建必须带 `--base-href /app/` 与 `--no-web-resources-cdn`。

**触发条件成立时才启用的可选依赖（均为宽松许可，可并入 AGPL）**

| 触发条件 | 引入 | 许可 |
|---|---|---|
| 内建图标缺失 / 需要 fill·weight 轴 | `material_symbols_icons`（**只用 const `Symbols.x`**） | Apache-2.0 |
| 需要自由取色（而非预设色板） | `flex_color_picker` | BSD-3-Clause |
| 必须像素级控制矢量描边并自管 SVG | `flutter_svg`（图标本体 Lucide = ISC） | MIT / ISC |
| 确需 `ContainerTransform` 等官方过渡 | `animations: ^2.2.0`（**禁止 3.x**） | BSD-3-Clause |

---

## 13. 参考链接索引

**许可判定**
- FSF 许可清单（Apache-2.0 / ISC / Expat / OFL）：https://www.gnu.org/licenses/license-list.html
- FSF 许可兼容性与 GPLv3↔AGPLv3 合并规则：https://www.gnu.org/licenses/license-compatibility.html
- SIL OFL 1.1 FAQ（1.2 聚合、1.4 打包、2.5 子集化=Modified Version 需改名）：https://openfontlicense.org/ofl-faq/

**Flutter / 平台行为**
- `BackdropFilter`（"relatively expensive"、`ImageFiltered` 指引）：https://api.flutter.dev/flutter/widgets/BackdropFilter-class.html
- 性能最佳实践（saveLayer / Opacity）：https://docs.flutter.dev/perf/best-practices
- Impeller 现状（Web 仍用 Skia）：https://docs.flutter.dev/perf/impeller
- `Color.withValues` / `withOpacity` 弃用：https://api.flutter.dev/flutter/dart-ui/Color/withValues.html
- Web platform view（iframe）官方示例：https://docs.flutter.dev/platform-integration/web/web-content-in-flutter
- `imagefilter` bounded blur：https://api.flutter.dev/flutter/rendering/ImageFilterConfig/ImageFilterConfig.blur.html

**Flutter issues（本报告引用）**
- [#154986 [web] Icon tree shaking doesn't work well on web](https://github.com/flutter/flutter/issues/154986)（open）
- [#184996 BackdropFilter blurs PlatformView / 完全不生效](https://github.com/flutter/flutter/issues/184996)
- [#89889 BackdropFilter disrupted by empty platform view in Flutter Web](https://github.com/flutter/flutter/issues/89889)（open）
- [#189260 BackdropFilter on Firefox Linux performance](https://github.com/flutter/flutter/issues/189260)（open）
- [#191207](https://github.com/flutter/flutter/issues/191207) / [#192147](https://github.com/flutter/flutter/issues/192147)（BackdropFilter 原生量级）
- [#163823 CJK 兜底字体从 gstatic 拉 6.3 MB](https://github.com/flutter/flutter/issues/163823)（open）
- [#184449 Font fallback download failure causes infinite loop](https://github.com/flutter/flutter/issues/184449)（closed/fixed）
- [#172561 [web] ☂️ WebParagraph：目标消除 fallback 字体](https://github.com/flutter/flutter/issues/172561)（open）
- [#128485 CanvasKit WOFF2 支持（closed, fixed）](https://github.com/flutter/flutter/issues/128485) / [#109108 Support woff2 font（open，非 Web）](https://github.com/flutter/flutter/issues/109108)

**字体**
- Noto CJK LICENSE（OFL-1.1）：https://github.com/notofonts/noto-cjk/blob/main/Sans/LICENSE
- **Noto Sans SC OFL.txt（含 RFN `'Source'`）：https://github.com/google/fonts/blob/main/ofl/notosanssc/OFL.txt**（`METADATA.pb` 同文）
- Source Han Sans LICENSE（OFL-1.1, RFN 'Source'）：https://github.com/adobe-fonts/source-han-sans/blob/release/LICENSE.txt
- LXGW WenKai OFL.txt（RFN + 允许子集化/woff2 的附加许可）：https://github.com/lxgw/LxgwWenKai/blob/main/OFL.txt
- Smiley Sans（OFL-1.1）：https://github.com/atelier-anchor/smiley-sans
- Maple Mono OFL.txt（OFL-1.1，无 RFN）：https://github.com/subframe7536/maple-font/blob/variable/OFL.txt
- MiSans FAQ（自定义条款）：https://hyperos.mi.com/font/faq
- 字体加载行为（启动时加载 manifest 全部字体）：https://github.com/flutter/flutter/issues/108660

**包 / 仓库**
- pub.dev API 与包页：`https://pub.dev/api/packages/<pkg>`、`https://pub.dev/packages/<pkg>`
- `material_symbols_icons`：https://github.com/timmaffett/material_symbols_icons · https://pub.dev/packages/material_symbols_icons
- Lucide 图标（ISC）：https://github.com/lucide-icons/lucide
- `flutter_chat_ui`（Apache-2.0）：https://github.com/flyerhq/flutter_chat_ui
- `flutter_ai_toolkit`（BSD-3）：https://github.com/flutter/ai
- `flutter/samples`（BSD-3）：https://github.com/flutter/samples
- `material_ui`（平行 Material 实现）：https://pub.dev/packages/material_ui
- `package:web` `AnalyserNode`：https://pub.dev/documentation/web/latest/web/AnalyserNode-extension-type.html

---

## 附：本报告的可复现实验记录（本机，2026-09-10）

```bash
# 1) 一次性探针工程（位于 /tmp/probe，不污染仓库）
flutter create --platforms=web --project-name probe /tmp/probe
cd /tmp/probe && flutter build web --release          # 基线：MaterialIcons 1,645,184 → 7,800 B
flutter pub add material_symbols_icons                # + const Symbols.send 一个图标
flutter build web --release                            # 三套字体 10.6/15.1/8.8 MB → 3.5/2.6/2.4 KB
# 2) 非 const IconData → 构建硬失败（exit 1）
#    "This application cannot tree shake icons fonts… Target web_release_bundle failed"
# 3) Noto Sans SC(woff2, wght=400) 逐片下载求和：101 片 / 2,354 KB
```
