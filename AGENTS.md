# Live2D-Ai 项目说明（AGENTS.md）

> 本文件是项目级通用说明，供任何 CLI agent（pi / Claude Code / Hermes 等）读取。
> 状态：2026-09-10 重写——确立 **Rust 核心 + Flutter 前端** 双主导分层，
> 前端/接口层自 `rust-ratio` 门禁中**显式豁免**（原版只描述 Rust 单主线）。

## 项目背景

- 文档索引见 `docs/README.md`；核心契约与目录约定见 `docs/architecture/core-contracts.md`
  与 `docs/architecture/directory.md`。
- 定位：**通用人形皮套 AI 接入一体化平台**——AI + Live2D 人形皮套的通用接入与应用层，
  **不绑定任何单一模型**（模型由用户合法导入，`assets/models/` 不捆绑二进制），
  **不做复杂上层**（实现保持最小）。验证「文本 → LLM → TTS → 口型/表情/动作 → Live2D 渲染」闭环。
- `Live2D-Ai-pc/`（Python）已归档（tag `py-legacy`）；`Live2D-Ai-Android/` 已归档
  （远端 `android-archive` 分支，见 `ANDROID_ARCHIVE_POINTER.md`）。
- 增强能力通过 **Mod 边界**隔离：`live2d-ai-mod-system` trait 注册中心，
  4 个 Mod（external-input / pet-desktop / director / local-llm）为 workspace crate，默认不启用。
- **TTS 不是 Mod**（2026-09-11 用户裁决）：语音合成是**核心链路**
  （LLM → TTS → 口型），端点唯一权威来源是 `live2d-ai.toml` 的 `[tts]` 段。
  见 `docs/architecture/tts-is-core.md`。

## 分层与技术栈（**双主导**）

本项目的「主导」是**两层**，不是一个语言：

| 层 | 语言 / 位置 | 职责 | 门禁 |
| --- | --- | --- | --- |
| **核心层（Rust）** | Rust workspace `crates/*` | 渲染（`l2d`）、状态机（`live2d-ai-core`）、LLM/TTS 网络与配置（`live2d-ai-runtime`）、桌面壳与 Web API（`live2d-ai-desktop`）、wasm 渲染面（`l2d-wasm-demo`）、工程工具（`xtask`） | `cargo test/fmt/clippy` + `rust-ratio` ≥95% |
| **前端/接口层（Flutter Web）** | `shell/flutter/`（Dart），由 Rust 在 `/app/` 同源托管 | 用户界面：Live2D 舞台、聊天、设置、外观/口型控制 | `flutter analyze` + `flutter test`（**豁免 `rust-ratio`**） |

- **唯一实现主线 = 核心层 Rust workspace**（`crates/`）：`l2d` / `live2d-ai-core` /
  `live2d-ai-runtime` / `live2d-ai-desktop` / `l2d-wasm-demo` / `xtask` / `live2d-ai-mod-system`。
- **前端以 Flutter Web 为准**（`shell/flutter/`）。旧的**原生 JS 前端**（`/` =
  `web_api/index.html` + `app.js` + `chat.js` + `style.css`）是**遗留实现**：
  视觉与交互质量不达标、维护成本高，**只修致命缺陷、不再加新功能**，
  长期由 Flutter 完全替代。新功能一律做在 Flutter 侧。

### 前端层豁免的理由（显式记录，勿当成遗漏）

1. **Flutter Web 无法用 `dart:ffi` 渲染**，业界统一做法是「隔离的 Web 渲染面 +
   版本化消息桥」——本项目即「Flutter 壳 + 复用 Rust 的 `/render`（wasm）」，
   渲染核心仍是 Rust，Dart 只承担界面与调度。
2. `xtask` 的 `rust-ratio` 统计扩展名（`rs/wgsl/py/ts/js/c/cc/cpp/h/hpp`）**不含 `dart`**，
   因此 Dart 不计入该门禁——这是**刻意的豁免**，不是统计漏洞。
   `xtask` 报告会**单独打印 Dart 行数**以保证可见性（可见但不设 Rust 占比门槛）。
3. 前端层的质量门槛改由 Flutter 自己的工具链承担：
   `flutter analyze`（静态检查）+ `flutter test`（含音频调度等纯逻辑回归）。
4. 治理红线仍然适用：前端**不得**绕过 core 仲裁、不得直接持有密钥、
   不得引入设备端推理运行时（LLM/TTS 仍走统一 OpenAI 兼容端点）。

## 开发约定

### 核心层（Rust）

- **门禁**（提交前必须）：`cargo test --workspace --all-targets` + `cargo test --doc --workspace`
  + `cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets -- -D warnings`
  + `cargo run -p xtask -- rust-ratio`（门槛 95%）。
- **源码 ≤500 行**（豁免 ≤1000 需头注理由）；测试文件 ≤800 行。
- **密钥安全**：密钥不进 GET/日志/WS/导出；loopback-only；mutating 需 `application/json`。
- **Mod 边界**：Mod 必须通过 `live2d-ai-mod-system` 接入，不得绕过 core 仲裁。

### 前端层（Flutter）

- **门禁**（改动 `shell/flutter/**` 时必须）：
  `cd shell/flutter && flutter analyze && flutter test`。
- 前端产物**不入库**：`shell/flutter/build/`、`.dart_tool/` 已被嵌套 `.gitignore` 排除；
  `xtask` 亦按目录名排除 `build`（否则 `main.dart.js` 会打穿占比口径）。
- 改动前端后**必须重新构建**才生效：
  `flutter build web --release --base-href /app/ --no-web-resources-cdn`
  （静态文件由服务端按请求读盘，故前端改动只需重建、不必重启后端）。
  **`--no-web-resources-cdn` 不是可选项**：缺省构建把 CanvasKit 指向
  `https://www.gstatic.com/flutter-canvaskit/<engineRevision>/`，
  而 `build/web/canvaskit/` 那份本地副本不被引用——**断网即白屏**。
  本项目是本地优先的桌宠，不允许依赖 Google CDN（`scripts/ignite.sh` 会探测并告警）。
- **中文字体必须自托管**（`assets/fonts/`，Noto Sans SC 子集，OFL-1.1）。
  Flutter Web 的 CanvasKit **取不到设备字体**，`fontFamilyFallback` 也不会命中系统字体——
  缺字时引擎只会去 `fonts.gstatic.com` 下载，**断网即豆腐块**（有网时看不出来）。
  不要删掉 `ThemeData.fontFamily`，也不要改用「系统字体回落」；回归在
  `test/theme_test.dart`（含 CJK 本地化后字体不被覆盖回 Roboto 的守卫）。
- **协议约定**：前端与渲染面之间只走**版本化消息**（`{version,type,payload}`，见
  `l2d-wasm-demo` 的 v1 协议）；新增字段必须向后兼容（缺省即默认值）。
- **不得**在 Flutter 侧复制核心逻辑（状态机、动作仲裁、LLM/TTS 协议）——
  那些属于 Rust 核心层，前端只做展示与调度。

### 语音输出约定（2026-09-10 用户裁决）

- **一句一单元**：一个句子必须**完整合成后连续播放**，不做句中切分、不做听感上的
  「断断续续」。**延迟可接受，断句不可接受。**
- 因此：分句只按真实句读边界（`。！？…` 等）切分；不允许按字符位置硬切。
- 播放侧允许为「整句完整性」引入预缓冲；宁可晚开口，不可中途卡顿或重复。
- **默认出声**（2026-09-10 用户裁决，v0.4.3 起）：出厂即有声——**「不出声」必须显式
  要求**，不能是默认状态（否则用户会以为 TTS 坏了）。要安静有两条路：用户按 UI 静音，
  或服务端 `LIVE2D_AI_MUTE_AUDIO=1`。**跑测试 / 无人值守一律用后者**：它在音频源头
  生效（下发零 PCM + 真实 `volume`），绕不过任何客户端缓存或第三方客户端。
- **主音量是客户端输出设置**（`GainNode` 增益），与口型**正交**：改音量、按静音都
  不得影响口型驱动。静音用「增益置 0」而不是断开音频图——音频图必须保持活着，
  口型的到期释放才不会漂移。

## 变更历史

- **2026-09-11（v0.5.0）**：用户裁定「**只核心链路做丰满即可**」+「前端 ui 很 ai 化同质」。
  据此**先删后改**：动作手动触发移出成品（只留分支 `archive/action-trigger-p5`
  与 `docs/design/web-action-trigger-archive.md`）；清掉没有功能的占位 UI；
  **本地 TTS 从 Mod 提升为核心链路**（`docs/architecture/tts-is-core.md`，
  Mod 5 → 4）；新增**黑/白/蓝/灰四套配色**（主题住本地偏好，点一下立刻生效）；
  舞台改**纯色底**（协议新增 `sync.stageColor`，由渲染面写——iframe 内 canvas
  会盖住父页画的底色）+ 可选用户展台图；新增**组件外观统一层**
  `buildAppComponents`（elevation 全 0、圆角/字级统一）与「**发送 = 圆形上箭头**」
  「**尽量少用图片，用文字做按钮**」两条形状约定（`test/visual_language_test.dart`）。
- **2026-09-10（v0.4.4 / v0.4.5）**：确立前端**离线可用**的两条硬约束——
  构建必须带 `--no-web-resources-cdn`（否则 CanvasKit 走 Google CDN，断网白屏）；
  中文字体必须自托管（否则断网豆腐块）。两者都是「有网时看不出来」的缺陷。
- **2026-09-10（v0.4.3）**：语音输出约定新增「**默认出声**」与「主音量/静音与口型正交」；
  静音改为默认关闭的服务端开关 `LIVE2D_AI_MUTE_AUDIO=1`（跑测试/无人值守用）。
- **2026-09-10**：确立 **Rust 核心 + Flutter 前端** 双主导分层；前端/接口层
  自 `rust-ratio` 显式豁免并改由 Flutter 工具链门禁；原生 JS 前端降为遗留；
  新增「语音输出约定（一句一单元，不断句）」。
- **2026-08-31**：按 Rust 主线重写（移除 Android/Python 双端描述）。
- **2026-08-09**：重写——移除 pi 专属路径引用，保留项目级通用信息。
