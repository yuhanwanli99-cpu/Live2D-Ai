# Flutter × Live2D 开源实现调研（2026-09）

> 目的：为本项目「Flutter 前端 + Rust 核心」定渲染方案。用户裁决：先找其他仓库的相关实现，本机实现需要优化。
> 结论先行：**Flutter Web 无法用 dart:ffi 渲染，业界统一做法 = 「隔离的 Web 渲染面 + 版本化消息桥」**；
> 差异只在「渲染面里跑什么」（Cubism Web SDK / Pixi / 自研 wasm）。本报告只做调研与选型建议。

---

## 一、调研范围与方法

- pub.dev 全量检索 `live2d`（仅 2 个相关包）；GitHub 检索 `flutter live2d`（10 个结果，逐个读源码）；
- 对关键包**下载 pub 归档解包读源码**（`live2d_flutter` 30MB，含全平台原生代码）；
- 对关键仓库读 ADR / bridge / host / renderer 源码（`ai-cartoon-chat`）。

## 二、结论摘要

| 方案 | 平台 | 渲染面 | 许可 | 适配本项目 |
|---|---|---|---|---|
| **A. live2d_flutter（hunmer）** | Web/Win/macOS/Android/iOS | Flutter **canvas 平台视图** + Cubism Web Framework（TS） | **专有**（Core 默认走 CDN） | 最快出画面；引入第二渲染器 + 专有许可 |
| **B. ai-cartoon-chat 架构** | Web/Win/Android | **iframe 隔离页** + Pixi + Cubism Web（本地打包） | 专有（Cubism） | **架构可直接照搬**，渲染面可换我们的 wasm |
| **C. 自研 Rust→wasm 渲染面 + 同款桥（推荐）** | Web（+桌面 WebView） | 现有 `l2d-wasm-demo` 的 `/render` | **MIT/Apache（Ayagami）** | 复用已验证渲染器，无专有依赖 |
| D. 原生平台视图/纹理 + Cubism Native | Android/iOS/macOS/Win（无 Web） | GLES / Metal / D3D 纹理 | 专有 | 非本次目标 |
| E. KMP/Compose 原生 | Android/iOS | OpenGL ES 2 | 专有 | 非 Flutter |

**核心事实**：Flutter Web 没有 `dart:ffi`，也**没有任何包在 Dart 侧渲染 Live2D** ——
所有 Web 方案都是「在 HTML/WebGL 画布（或 iframe）里渲染，Flutter 用消息桥控制」。

---

## 三、逐个实现分析

### A. `live2d_flutter` 1.0.3（pub.dev，2026-09-04 发布）

- 声明平台：**Android / iOS / macOS / Web / Windows**；仓库 `github.com/hunmer/flutter_live2d`（匿名访问 404，私有或改名；pub 归档可下载）。
- **Web 实现**：`ui_web.platformViewRegistry.registerViewFactory` 注册 `HTMLCanvasElement` 平台视图（`lib/src/web_view.dart`），
  再经 JS interop 调 `FlutterLive2DWeb.{createView,loadModel,startMotion,setExpression,setParameter,...}`。
- **Web 运行时**：`web_runtime/` = Cubism **Web Framework 源码**（TS，含 `cubismlipsyncupdater.ts` 口型、physics、WebGL renderer）+ 自写 host（`runtime_host.ts`），`build.mjs` 打包。
- **Core 依赖**：`lib/flutter_live2d_web.dart` 硬编码 `https://cubism.live2d.com/sdk-web/core/06/live2dcubismcore.min.js`（**CDN 取 Core**）。
- **原生**：Android/iOS/Windows/macOS 各自 vendored 一份 Cubism Native Framework + `libLive2DCubismCore.a`。
- 优点：开箱即用、API 完整（loadModel/startMotion/expression/setParameter/暂停）、透明背景、多实例。
- 代价：①**专有许可**（Cubism Core + Framework 条款）；②Web 默认依赖外网 CDN；③等于**再引入第二渲染器**，与现有 `l2d`(Ayagami+wgpu) 并行。

### B. `shallowsingaa/ai-cartoon-chat` —— 架构最值得照搬

面向 **Android / Web / Windows** 的双角色 Live2D AI 聊天应用，有 ADR 与完整测试。要点：

- **ADR-0001**：*Flutter 客户端拥有界面与客户端状态；Live2D 渲染留在受控 Web 组件里，通过**结构化消息**驱动*；
  复用同一套 Cubism Web 渲染器覆盖三端，避免三套渲染集成，同时保证「是真正的 Flutter 客户端，而不是套壳 WebView」。
- **渲染面**：`assets/live2d/{index.html,bridge.js,renderer.js,vendor/{pixi,cubism4,live2dcubismcore}}` **随客户端分发（离线，不依赖 CDN）**；ADR-0003 接受约 18.7MiB 体积。
- **协议**（`bridge.js`）：`{version:1, type, payload}` 双向；下行 `sync / action / mood / lipSync / pause / destroy`，上行 `ready / loading / error / destroyed`。
- **Dart 桥**（`live2d_bridge.dart`）：状态机 `loading→ready→error/destroyed`；**未就绪命令入队**、ready 后 flush；换角色才重载模型。
- **宿主抽象**（`live2d_host*.dart` 条件导入三实现）：
  - `_web`：`HtmlElementView.fromTagName("iframe")` + `postMessage`（**校验 `source` + `origin` 同源**）；
  - `_native`：`webview_flutter` + `webview_flutter_windows`（Windows 需 WebView2，缺失报 `MissingWebView2Runtime`）；
  - Android：`Live2dAssetServer` 把 asset 起成本地 HTTP 再喂 WebView。
- **传输接口**：`Stream<String> events` / `Future<void> send(String)` / `dispose()`。
- **Stage widget**：持有 bridge；didUpdateWidget 下发 action/mood/lipSync；生命周期暂停；错误可重试。

⚠️ **该项目的口型是假的**：`renderer.js:290` 用正弦波 `(sin(t*11)*0.5+0.5)*0.85` 直写 `ParamMouthOpenY`，**不是真实 TTS 音频驱动**。这一点**不要照抄**。

### C. 原生渲染类（桌面/移动参考，不适用于 Flutter Web）

| 仓库 | 做法 | 口型 |
|---|---|---|
| `linh18nd/flutter_live2d`（pub `flutter_live2d`） | Android/iOS + Cubism Native + GLES2，透明背景，controller 响应式 | 无 |
| `ltarcher/flutter_live2d_lipsync` | Android 插件，`updateLipSync(level 0..1)` → `ParamMouthOpenY` | 外部传入电平 |
| `SunWithCat/lumi` | Android，Cubism + GLES2，**Flutter `Texture` 接入**，60FPS | 情绪联动，口型弱 |
| `9ckobn/ai-chan-flutter`（iOS 上架） | Swift/ObjC++ **Metal 平台视图**，60FPS | **振幅 + 元音形**分析器（60 采样/秒 → mouth openness + vowel form），Dart 侧平滑，驱动 `ParamMouthOpenY`/`ParamMouthForm` |
| `gameswu/NyaDeskPet` | 桌面原生 OpenGL；移动端 KMP+Compose（非 Flutter） | 有 |
| `BOHUYESHAN-APB/N-T-AI` | Flutter + FastAPI + **WebView 渲染 Live2D** | 明确写：**TTS 必须在 Flutter 侧生成，才能对齐口型时序** |

---

## 四、对本项目的结论

### 4.1 架构（对齐 ADR-0001 与 B，渲染面换成我们自己的）

```text
Flutter Web（真正的客户端：UI / 状态 / 网络编排）
  ├── 模型舞台：HtmlElementView("iframe") → Rust→wasm 渲染面（我们已有的 /render）
  │      ↕ 版本化消息桥 {version,type,payload}（postMessage，同源校验）
  └── HTTP/WS → Rust 后端（现有 live2d-ai-desktop --web）
         · POST /api/v1/chat  → LLM SSE → 分句 TTS
         · WS: text_delta / turn_state / action_state / audio(s16le)
```

**闭环**：Flutter 发消息 → Rust LLM→TTS → WS 回音频 → Flutter 播放并算**真实 RMS**（attack/release 平滑）
→ 桥下发 mouth level → wasm 写 `ParamMouthOpenY` → 模型嘴型随语音变化。

### 4.2 为什么渲染面用我们自己的 wasm 而不是 Cubism/Pixi

1. **许可**：项目显式选择 Ayagami（MIT/Apache）替代 Live2D 专有 Cubism SDK（README:54-55）；引入 Cubism 会推翻这条路线。
2. **复用**：`l2d-wasm-demo` 已能渲染我们的 `.moc3` 资产，且已有 `action-state` / `audio-volume` / `stage-config` / `load-model` 的 postMessage 桥（`main.rs:297-324`）——
   只需把它**协议化/版本化**并补 `ready/loaded/error/fps` 上行。
3. **单一渲染真相**：桌面与 Web 共用一套 Rust 参数/动作语义（`live2d-ai-core` 仲裁），不会出现两套行为。

### 4.3 已知坑（实现时必须处理）

| 坑 | 出处 | 处理 |
|---|---|---|
| 平台视图层级 / 点击穿透 | flutter#101580、#145360、#175119 | 模型舞台用独立 iframe；交互控件不叠在 iframe 上方（或让 iframe pointer-events 可控）。**本项目的落地（2026-09-11）**：控件确实叠在了舞台上（设置面板/断线横幅/缩放角标/错误重试），于是全部点不着——最后选了后半句的**按盒子**版本：`StagePointerInterceptor`（透明 `<div>` 平台视图垫层，与 flutter/packages 的 `pointer_interceptor` 同法）。参考 flutter/flutter#190574 |
| 必须同源 | ai-cartoon-chat transport 校验 | Flutter 构建产物与 `/render` 都由 Rust 服务同源提供（loopback） |
| Flutter Web 无 dart:ffi | 语言事实 | 后端必须走 HTTP/WS（现有 `--web` 正好满足） |
| 口型假驱动 | ai-cartoon-chat renderer.js:290 | 用真实音频 RMS（后端 `RmsMeter` 参数：50ms 窗 / attack 0.03 / release 0.10） |
| Core/CDN 外网依赖 | live2d_flutter web | 我们的 wasm 全本地，无外网依赖 |
| 桌面（非 Web）需 WebView2 | ai-cartoon-chat native host | 本阶段只做 Web；桌面后续用同一桥 + `webview_flutter_windows` |

### 4.4 建议的桥协议 v1

```jsonc
// Flutter → 渲染面
{ "version":1, "type":"sync",    "payload":{ "model":"/models/bai/...", "scale":1.0, "dark":true, "tier":4096, "paused":false } }
{ "version":1, "type":"action",  "payload":{ "action":"nod", "strength":2 } }   // 6 基础动作
{ "version":1, "type":"mouth",   "payload":{ "level":0.0 } }                    // 0..1，实时
{ "version":1, "type":"stage",   "payload":{ "dark":true, "scale":1.0, "tier":4096 } }
{ "version":1, "type":"destroy", "payload":{} }
// 渲染面 → Flutter
{ "version":1, "type":"ready",    "payload":{ "protocol":true } }
{ "version":1, "type":"loaded",   "payload":{ "model":"bai" } }
{ "version":1, "type":"progress", "payload":{ "phase":"textures", "ratio":0.4 } }
{ "version":1, "type":"error",    "payload":{ "message":"..." } }
{ "version":1, "type":"fps",      "payload":{ "value":60 } }
```

---

## 五、推荐落地顺序

1. **协议化现有 wasm 渲染面**：把 `l2d-wasm-demo` 的 postMessage 提升为版本化 v1（补 `ready/loaded/error/fps` 上行，保留旧类型兼容）。
2. **Rust 服务同源托管 Flutter Web 产物**（新增静态路由，loopback-only）。
3. **Flutter Web 应用**：`Live2dTransport`（iframe + 同源校验）/ `Live2dBridge`（状态机 + 命令队列）/ `Live2dStage`（Widget）三件套按 §4.1、§4.4 实现；UI 用 Flutter。
4. **真实口型**：Flutter 侧对 WS `audio` 帧做 RMS + attack/release 平滑，下发 `mouth`。
5. **验收**：真 LLM/TTS 下，浏览器发消息 → 出声 → 嘴型同步 → 动作触发。

> 备选（最快出画面）：直接依赖 `live2d_flutter`（方案 A），代价是专有许可 + 第二渲染器 + CDN 依赖；
> 与本项目「不用闭源 Cubism SDK」的既定路线冲突，需显式豁免。

---

## 六、参考链接

- [shallowsingaa/ai-cartoon-chat](https://github.com/shallowsingaa/ai-cartoon-chat)（ADR-0001 隔离渲染面 / 消息桥 / 三端宿主）
- [hunmer/flutter_live2d on pub.dev](https://pub.dev/packages/live2d_flutter)（Web+桌面的 Cubism Web 平台视图方案）
- [linh18nd/flutter_live2d](https://github.com/linh18nd/flutter_live2d)（Android/iOS Cubism Native）
- [ltarcher/flutter_live2d_lipsync](https://github.com/ltarcher/flutter_live2d_lipsync)（电平→ParamMouthOpenY）
- [SunWithCat/lumi](https://github.com/SunWithCat/lumi)（Flutter Texture 接入 GLES2，60FPS）
- [9ckobn/ai-chan-flutter](https://github.com/9ckobn/ai-chan-flutter)（Metal 平台视图 + 振幅/元音形口型）
- [BOHUYESHAN-APB/N-T-AI](https://github.com/BOHUYESHAN-APB/N-T-AI)（Flutter+WebView+FastAPI，TTS 在客户端对齐口型）
- [gameswu/NyaDeskPetAPP](https://github.com/gameswu/NyaDeskPetAPP)（KMP/Compose 原生桌宠）
- Flutter 平台视图已知问题：[#101580](https://github.com/flutter/flutter/issues/101580)、[#145360](https://github.com/flutter/flutter/issues/145360)、[#175119](https://github.com/flutter/flutter/issues/175119)
