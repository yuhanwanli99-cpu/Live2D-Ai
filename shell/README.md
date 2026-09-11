# shell/flutter —— Live2D-Ai 的 Flutter Web 前端外壳

> 状态：最小可用工程。只依赖 `flutter` + `http` + `web`（dev 依赖 `flutter_lints`），
> 不引入任何 Live2D / webview / 状态管理框架。
> 渲染面走「Flutter 内嵌 `/render` iframe」+ 协议 v1 postMessage。

## 1. 目录结构

```
shell/flutter/
├── pubspec.yaml                 # 依赖：http ^1.6.0 / web ^1.1.1
├── lib/
│   ├── main.dart                # MaterialApp（深色）+ 主页面（左舞台 / 右聊天）
│   ├── api/
│   │   ├── api_client.dart      # POST /api/v1/chat、/chat/stop、GET /app/status
│   │   └── ws_client.dart       # /ws/state 连接 + 事件解析 + 指数退避重连
│   ├── audio/
│   │   └── audio_player.dart    # WebAudio 播放 + s16le RMS 口型包络
│   ├── chat/
│   │   └── chat_controller.dart # 消息列表 / 流式拼接 / turn 状态 / 错误提示
│   └── live2d/
│       ├── live2d_transport.dart   # 传输接口 Stream<String> / send / dispose
│       ├── live2d_bridge.dart      # 协议 v1 状态机 + ready 前入队
│       ├── live2d_host_web.dart    # HtmlElementView iframe + postMessage 校验
│       ├── live2d_host_stub.dart   # 非 web 平台占位
│       ├── live2d_stage.dart       # StatefulWidget，对外 setMouth / sync / retry
│       ├── stage_pointer_interceptor.dart       # 压在舞台上的控件必须垫的指针垫层
│       ├── stage_pointer_interceptor_web.dart   # web：透明 <div> 平台视图
│       └── stage_pointer_interceptor_stub.dart  # 非 web：透明返回 child
├── test/live2d_bridge_test.dart # 协议 v1 状态机单元测试（VM 可跑）
└── web/                         # index.html / manifest.json（base href 由构建注入）
```

`build/` 已在 `shell/flutter/.gitignore` 中（`/build/`），构建产物不入库。

> **压在舞台上的控件必须套 `StagePointerInterceptor`**（2026-09-11）：舞台是 `<iframe>`
> 平台视图，DOM 上排在 Flutter 画布之上，而画布是 `pointer-events: none`——指针落在
> 舞台区域会进 iframe 自己的文档，父页收不到。漏套不会报错，只会
> **「看得见、点不着、也滑不动」**（设置面板、断线横幅、缩放角标、错误重试全中过）。
> 原理与边界见该文件头注；`test/stage_pointer_interceptor_test.dart` 钉住已知接线点。

## 2. 构建

Flutter SDK 在本机位于 `~/flutter`（默认不在 PATH）：

```bash
export PATH="$HOME/flutter/bin:$PATH"

cd shell/flutter
flutter pub get
flutter analyze                 # 要求：无 error、无 warning
flutter test                    # 协议 v1 状态机单测
flutter build web --release --base-href /app/ --no-web-resources-cdn
```

产物：`shell/flutter/build/web/`（`index.html` / `main.dart.js` / `flutter_bootstrap.js` /
`assets/` / `canvaskit/` …）。

**必须带 `--base-href /app/`**：产物由 Rust 挂在 `/app/` 下，不带 base href 时
`flutter_bootstrap.js`/`main.dart.js` 会按根路径解析（`/flutter_bootstrap.js`）而 404。

**必须带 `--no-web-resources-cdn`**：不加时 Flutter 会把 CanvasKit 指向
`https://www.gstatic.com/flutter-canvaskit/<engineRevision>/`，而产物里那份
`canvaskit/`（约 37 MB）**根本不被引用**——结果是**断网白屏**。
本项目是本地优先的桌宠，不允许依赖 Google CDN。（`scripts/ignite.sh` 启动时会探测
产物是否仍引用该 CDN 并告警。）

## 3. 如何被 Rust 以 `/app/` 托管

Rust 侧已有模块 `crates/live2d-ai-desktop/src/web_api/flutter_app.rs`：

- 接管 `GET /app` 与 `GET /app/*`（不吞 `/apple`）；
- 静态服务构建目录 `shell/flutter/build/web/`；也可用环境变量
  `LIVE2D_AI_FLUTTER_WEB_DIR` 覆盖；
- 仅 GET，其它方法返回 `405 method_not_allowed`；
- 产物缺失时返回 `503 flutter_build_missing`，并提示先执行上面的构建命令（不留白屏）；
- 无扩展名的路径（客户端路由）回落 `index.html`；
- 路径经规范化 + canonicalize 校验，拒绝 `..` 越界；
- 响应带 `Cache-Control: no-cache`。

启动后端（仅 loopback）：

```bash
cargo run -p live2d-ai-desktop -- --web --http-port 18080
# 浏览器打开 http://127.0.0.1:18080/app/
```

**为什么必须同源**：Flutter 用 `HtmlElementView` 内嵌 `/render` iframe，并以
`postMessage(json, location.origin)` 通信、按 `event.origin` 过滤。只有 Flutter 产物与
`/render` 由同一个 loopback 服务提供，同源校验才成立。

## 4. `--dart-define` 用法

| 变量 | 缺省 | 说明 |
|---|---|---|
| `API_BASE` | 空（同源） | HTTP 根。空时取页面 `Uri.base.origin`；可覆盖为 `http://127.0.0.1:18080` |
| `WS_URL` | 由页面推导 | WebSocket 地址。缺省 `ws(s)://<host>:<port>/ws/state` |

```bash
# 同源相对路径（推荐，由 /app/ 托管时）
flutter build web --release --base-href /app/ --no-web-resources-cdn

# 指向固定后端（例如 Flutter 单独 flutter run 时）
flutter build web --release --base-href /app/ --no-web-resources-cdn \
  --dart-define=API_BASE=http://127.0.0.1:18080 \
  --dart-define=WS_URL=ws://127.0.0.1:18080/ws/state
```

## 5. 与后端的真实契约（读源码确认）

### HTTP

- `POST /api/v1/chat`，`Content-Type: application/json`，请求体 `{"text":"..."}`
  （`chat_routes.rs`）。
  - `200` → `{"accepted":true,"epoch":N,"pending_cleared":false}`
  - `400 invalid_payload` / `405 method_not_allowed` / `429 busy` / `503 no_supervisor`
- `POST /api/v1/chat/stop` → `200 {"accepted":true}`（幂等）。
- `GET /api/v1/app/status` → 运行状态摘要（`dto.rs::AppStatus`）。
- `GET /api/v1/app/capabilities` → 能力与版本（`{version, actions, runtime_ws, ...}`）。
- `GET /api/v1/settings` → `{llm, tts, persona, dev_mode}`（**密钥只回 `has_api_key` 布尔**）。
- **`PATCH /api/v1/settings`** → 部分更新设置。
  ⚠️ 是 **`PATCH` 不是 `PUT`**：`mod.rs` 的 `match_route` 只认 `Method::Patch`，
  用 `PUT` 会 **404**（实测 `PUT→404` / `PATCH→200`）。
- `POST /api/v1/settings/test/llm`、`POST /api/v1/settings/test/tts` → 连通性自检。
- `GET /api/v1/models`（含导入）、`GET /api/v1/commands`、`GET /api/v1/logs`、`GET /api/v1/mods`。
- 错误体统一为 `{"error":{"code","message","details"}}`。

### WebSocket `/ws/state`

服务端每连接包装成 `{"type":...,"seq":N,"ts":"...","data":{...}}`：

| type | data |
|---|---|
| `subscribe_ack` | `{topics:[...]}`（连接建立后首帧） |
| `heartbeat` | `{}`（10s 周期保活） |
| `turn_state` | `{epoch, status:"completed"\|"failed"}` |
| `runtime_status` | `{event:"voice_started"\|"voice_ended"\|"new_epoch"\|"shutdown_ready", epoch?}` |
| `text_delta` | `{epoch, ts_ms?, text}`（流式正文）或 `{epoch, completed}`（完成锚点） |
| `action_state` | `{epoch, kind, action?}`（本轮前端只解析、不实现表演） |
| `audio` | `{epoch, audio:<base64 s16le>, sample_rate, slice_ms:20, volume, start, end, muted}`（`muted=true` 时 `audio` 全零但 `volume` 仍为真实值） |
| `error` | `{code, message}` |

`ws.rs` / `ws/connection.rs` 明确：**WebSocket 是单向事件流**，服务端不读客户端帧
（客户端发 `subscribe`/`ping` 会被底层静默丢弃）。因此本前端连接后只监听，
不发 subscribe，命令一律走 HTTP。收到 `shutdown_ready` 后不再重连；其余断开按
1s→2s→4s→8s→16s→30s 指数退避重连。

## 6. 渲染面协议 v1（Flutter ↔ `/render` iframe）

每条消息都是 JSON 字符串 `{"version":1,"type":...,"payload":{...}}`。

下行（Flutter → iframe），`iframe.contentWindow.postMessage(jsonString, location.origin)`：

```json
{"version":1,"type":"sync","payload":{"model":"/models/bai/runtime/bai.model3.json","scale":1.0,"dark":true,"tier":4096,"paused":false}}
{"version":1,"type":"mouth","payload":{"level":0.0}}
{"version":1,"type":"stage","payload":{"dark":true,"scale":1.0,"tier":4096}}
{"version":1,"type":"destroy","payload":{}}
```

`sync` 字段均可选；`mouth.level ∈ [0,1]`，实时、节流 ≤30Hz。

上行（iframe → Flutter）：`ready` / `loaded` / `progress` / `error` / `fps`。
另有历史遗留的 `{"type":"stage-ack",...}`（无 version）。

实现约定（`live2d_bridge.dart`）：

- 状态机 `loading → ready → error → destroyed`；`ready` 前命令入队，收到 `ready` 后按序 flush；
- **容忍无 version 的消息**，忽略未知 `type`，坏帧/非 JSON 一律丢弃，**绝不抛异常**；
- 接收侧严格校验 `event.source == iframe.contentWindow` 且 `event.origin == location.origin`；
- 12s 内未收到 `ready` → 进入 error 并显示可见错误 + 「重试」按钮（不静默白屏）。

## 7. 音频与口型

`/ws/state` 的 `audio` 帧是 base64 编码的 s16le 单声道 PCM（默认 24000 Hz、20ms/片）：

- 播放：`AudioContext` → `AudioBuffer` → `AudioBufferSourceNode`，按 `nextStartTime` 链式
  衔接；epoch 切换或 stop 时打断；
- 口型：对同一份 PCM 直接算 RMS（s16le → [-1,1] → RMS），窗口 50ms，attack 0.03s /
  release 0.10s，按帧推进，输出 `level ∈ [0,1]` 给舞台 `setMouth`；
- autoplay：首次用户手势（页面任意指针按下）时 `AudioContext.resume()`。

## 8. 已知限制 / 存疑点

1. ~~**渲染端 v1 兼容性**：…两者不一致时口型不会生效~~ —— **该说明已过期，2026-09-10 更正**。
   渲染端现已**优先读 `payload.level`**、仅在缺失时回退 `payload.value`
   （`crates/l2d-wasm-demo/src/main.rs`：`payload.get("level").or_else(|| payload.get("value"))`），
   所以本前端按 v1 规格发 `level` **是正确的**，口型正常生效，无需改发 `value`。
   原文把「读 `value`」当成了唯一口径，会误导实现者去改协议。
2. WS 客户端按真实协议**不发** `subscribe`/`ping`（服务端不读客户端帧）。
3. 动作**仍是只解析、不表演**，但解析已完整：`ActionStateEvent` 现在带
   `action` / `strength` / `source`（用户 / LLM / 规则）。
   界面**已经**把 `source` 表达出来（`lib/ui/action_source_badge.dart` +
   `lib/ui/action_history_view.dart`，在 `actions_section.dart` 里接线）——
   这条说明 2026-09-11 更正：过去写的是「界面尚未表达」，与代码不符。
   这是设计规格 `docs/design/web-ui-spec-v3.md` §8.3 / §10-P2 的内容。
4. 构建时 Flutter 会打印 `Expected to find fonts for (... packages/cupertino_icons/CupertinoIcons)`
   的警告：这是 Flutter 在存在文本输入控件时的既有提示；本工程按约束未引入
   `cupertino_icons` 依赖，不影响构建与运行。
