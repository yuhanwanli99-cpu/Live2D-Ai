# 点火计划：核心链路闭环 + 本地双 Mod（2026-09-10）

> 状态：**执行中**。许可 = **`AGPL-3.0-only`（W1 取消）**；**TTS 选型待定**（相关配置已移除，等用户选定后再接回）。
> 唯一目标：**Flutter 前端 + Rust 后端**跑通「文本 → LLM → TTS → 音频 → 真实口型 → Live2D 渲染」，交你点火实测。
> 唯一豁免：**Mod**（两个本地 Mod 接入）。
> **动作不做**；**本地模型本体不碰** —— 只要核心链路说 OpenAI 标准接口，任何本地实现都能插进来。

---

## 1. 点火成功的定义（你可逐条自测）

| # | 现象 | 判定 |
|---|---|---|
| 1 | 打开前端页面，Live2D 模型正确渲染（非黑屏/白屏/空白） | 模型可见 |
| 2 | 输入文本发送 → LLM 回复**流式**逐字显示 | 文本流 |
| 3 | ~~TTS 语音~~ **本轮不验收**（TTS 选型待定，配置已移除） | 待你选定 |
| 4 | ~~口型随语音开合~~ **本轮不验收**（依赖 TTS） | 待你选定 |
| 5 | LLM/TTS 全部走 **OpenAI 标准接口**（`/chat/completions` + `/audio/speech`），由你提供的本地实现承担 | 接口闭环 |

---

## 2. 本轮范围

### 做（只有两块）

**A. 核心链路**
- 许可：**保持 `AGPL-3.0-only` 现状**（最终裁决）→ **W1 取消，零改动**。
- Flutter 前端（Web）+ Rust 后端（现有 `--web`）闭环。
- 渲染面协议 v1：`/render`（Rust→wasm）与 Flutter 之间的版本化消息桥。
- 真实口型：TTS PCM → RMS（attack/release）→ `ParamMouthOpenY`。

**B. Mod（唯一豁免）**
- `local-llm`：**探活 + 写 settings + 热重载**，把核心链路指向你提供的任意 OpenAI 兼容 LLM 端点。
- `local-tts`：**本轮不接入**（用户裁决 TTS 选型待定，相关配置已从仓库移除）。
- **不内置推理、不绑定模型、不下载任何模型**。

### 不做（标记待做，本轮不碰）
- **动作 / 表演**：6 基础动作触发、director 编序、动作快捷按钮。
- **本地模型本体**：不下载权重、不跑推理、不调参。
- STT / 语音输入、pet-desktop 窗口、external-input 产品化。
- 设置面板精修、多模型管理、背景图、纹理档位 UI、日志面板、可观测性。
- 桌面（Windows/Linux）与 Android/iOS 端。
- 核心状态机重构、benchmark 扩容、外部验收项（S3/S4）。

---

## 3. 现状可复用点（不重复造轮子）

| 能力 | 现状 | 证据 |
|---|---|---|
| OpenAI 双协议客户端 | LLM `/chat/completions`(SSE) + TTS `/audio/speech`(pcm s16le)，`base_url` 一改即换端 | `runtime/src/lib.rs:105`、`llm.rs`、`tts.rs` |
| Rust→wasm 渲染面 | 已有 `/render` 页面 + `/models/*` 资产 + postMessage | `web_api/wasm_assets.rs`、`l2d-wasm-demo/src/main.rs:297-324` |
| Web 控制面 | 已有 chat / settings / mods 路由 + WS 事件（text_delta / turn_state / audio） | `web_api/{chat_routes,mods_routes,ws}` |
| 双 Mod 真回路 | spawn → 探活 → `__apply_settings` → 原子写盘 → `supervisor.reload()` | `mod_registry.rs:233-260`、`cli_entry.rs:177-236` |
| 口型算法 | `RmsMeter`（50ms 窗 / attack 0.03 / release 0.10），只取实际输出样本 | `runtime` audio、`audio/ring.rs:382` |
| 核心仲裁 | 6 动作 + capability gate + epoch 闸门（490 测试全绿）—— 本轮不触发动作，仅保留不删 | `live2d-ai-core`、`supervisor/` |

---

## 4. 环境事实

### 4.1 宿主实配（2026-09-10）

| 项 | 值 | 影响 |
|---|---|---|
| CPU | Intel Core Ultra 7 251HX (2.90 GHz) | 与本地推理无关（本轮不跑模型） |
| 内存 | 24.0 GB（WSL 分到 11GB） | 够 Flutter 构建 + Rust 构建 |
| GPU | RTX 5060 Laptop 8GB | 留给你自己的本地实现 |
| 存储 | 477GB 中余约 139GB | 够 Flutter SDK + 构建产物 |

### 4.2 构建约束

- **无 root**（`sudo` 被 no-new-privileges 禁用）→ 依赖只能用户态装到 `~/`。
- 无 clang/cmake/ninja/libgtk-3-dev → **Flutter 桌面不可构建**；**Flutter Web 可构建**。
- **Flutter SDK 已就绪**：`~/flutter`，**3.47.3 stable / Dart 3.13.3**（实测 `flutter --version` 通过）。

---

## 5. 执行波次

### W0 环境（已完成）
- Flutter SDK → `~/flutter`：**完成（3.47.3）**。

### W1 许可迁移 —— 【已取消】
- 用户裁决保持 `AGPL-3.0-only`（仓库现状），本轮**零改动**。
- 既有 `LICENSE` / `NOTICE` / `COMMERCIAL-LICENSE.md`（双授权）全部原样保留。

### W2 渲染面协议 v1（Rust，改动小）
- 把 `l2d-wasm-demo` 的 postMessage 升级为 `{version:1, type, payload}`。
- 上行：`ready` / `loaded` / `progress` / `error` / `fps`。
- 下行：`sync` / `mouth` / `stage` / `destroy`（**不含 action**）。
- 保留旧 type 别名一版（`audio-volume` / `stage-config` / `load-model`）。
- 验收：wasm 侧单测 + 沿用 `webapp_behavior.rs` 的 Node 桥行为测试。

### W3 Rust 服务托管 Flutter 产物 + 音频单源
- `web_api` 新增 `/app/*` 静态路由（读 `shell/flutter/build/web`），`/` → Flutter `index.html`；保持 loopback + 路径安全。
- **音频单源**：web 模式默认「仅浏览器出声」（cpal 侧 dry-run），消除已知双声缺陷；WS 音频广播默认开启。
- 验收：`curl /` 返回 Flutter 页面；日志确认无 cpal 播放；WS 收到 audio 帧。

### W4 Flutter Web 前端（核心链路）
- 新工程 `shell/flutter/`（Flutter Web）。
- 三件套：`Live2dTransport`（iframe + source/origin 同源校验）/ `Live2dBridge`（v1 协议 + 状态机 + 命令队列）/ `Live2dStage`（Widget）。
- `ws_client`：`text_delta` / `turn_state` / `audio`（**不订阅 action_state**）。
- `audio_player`：WebAudio 播放 s16le 帧；同源 PCM → **RMS + attack/release 平滑** → `mouth`。
- UI：聊天区 + 舞台（最小可用，不做上层）。
- 验收：`flutter analyze` 干净；`flutter build web` 成功；产物可被 W3 路由托管。

### W5 本地 Mod（唯一豁免，指向 OpenAI 标准接口）—— ✅ 已完成
- `local-llm`：`externally_managed` 缺省 `true`（只探活、不 spawn）；探测成功只写 **`base_url`**，仅在 Mod 显式配置 `model` 时才写 `model`（不覆盖 toml 选择）。
- `local-tts`：**本轮不接入**；代码保留但不在默认 manifest 中启用，`[tts]` 配置已从 toml 移除。
- **TTS 可缺失**：`[tts] base_url` 为空 = 明确「未配置」，`resolve` 不报错，引擎走**空句路径**（只出文本、不合成音频、不驱动口型）。
- 缺陷修复：M1（无 `mods.json` 时用内建缺省 manifest 启用 `local-llm`）、M3（默认模型名改 `qwen3:4b` 且不再无条件写回）、M4（spawn 失败只记录、不让 Mod start 失败）。
- 验收：`/api/v1/mods` 显示 `local-llm` running；探测到端点后自动写 `base_url` 并 reload；无 TTS 时对话正常出文本。

### W6 点火脚本 + 清单
- `scripts/ignite.sh`：校验/探活你的 LLM 与 TTS 端点 → 起 `--web` → 打印访问 URL。
- `docs/verification/ignition-core-loop.md`：§1 五条自测清单 + 预期结果 + 失败排查。
- 验收：你按脚本一键点火。

---

## 6. 验收口径

| 域 | 验收 |
|---|---|
| 工程门禁 | `cargo test --workspace --all-targets` 全绿 + `--doc` + `fmt --check` + `clippy -D warnings` + `rust-ratio >= 95` |
| 前端 | `flutter analyze` 无错 + `flutter build web` 成功 |
| 点火 | §1 五条全部通过（你自测，用你自己的本地端点） |
| 接口 | LLM/TTS 均为 OpenAI 标准协议，`base_url` 可任意替换 |
| 许可 | 保持 `AGPL-3.0-only` 现状（无需变更）；模型资产例外说明不变 |
| 外部项 | 真实端点/真机项不 fake 为完成 |

---

## 7. 许可选型：Apache-2.0 vs AGPL-3.0 —— 【已裁决：AGPL-3.0-only】

### 7.1 结论

**用户最终裁决：保持 `AGPL-3.0-only`（仓库现状，W1 取消）。**
以下对比保留作决策记录。

### 7.2 理由（选 Apache 时）

| 维度 | Apache-2.0 | MIT |
|---|---|---|
| 宽松度 | 允许商用/闭源/再许可 | 同左（更短） |
| **专利授权** | **显式授予**（含专利报复终止） | 无 |
| 商标 | 明确不授予商标权 | 未提及 |
| 贡献条款 | 有 + `NOTICE` | 无 |
| 生态 | Rust 主流（wgpu/tokio/reqwest 均 MIT OR Apache-2.0） | 同 |

### 7.3 依赖与资产兼容性

- 主要依赖（Ayagami `MIT OR Apache-2.0`、wgpu/winit/cpal/reqwest/tiny-http/tracing）→ 与 Apache-2.0 完全兼容。
- Flutter / Dart（BSD-3-Clause）→ 兼容。
- **Live2D 模型资产不在授权范围**，沿用各 `MODEL_LICENSE.md`。
- 变更前用 `cargo metadata` 复核一次有无 GPL/AGPL 依赖。

### 7.4 影响面（W1 清单）

- `LICENSE` 全文替换；`NOTICE` 更新；各 crate `Cargo.toml` 的 `license` 字段改为 `Apache-2.0`。
- 源码头注 / README / docs / `index.html` 角标中的 AGPL 文案替换（约 77 处，`dist/` 除外）。
- `COMMERCIAL-LICENSE.md` 删除或改写为「Apache-2.0 已允许商用，无需商业授权」。
- ⚠️ 重新许可需**全部版权人同意**（署名 `Sakura Motion Project`；外部贡献者需逐一确认）。

### 7.5 另一个选项：AGPL-3.0-only（仓库现状）

| 维度 | `AGPL-3.0-only`（现状，零迁移） | `Apache-2.0`（默认执行） |
|---|---|---|
| 传染范围 | 强 copyleft：修改版 + 网络交互须提供源码（§13） | 无 |
| 闭源集成 | 不允许 | 允许 |
| 社区 Mod | 与 host 静态编译进同一 binary 大概率算衍生 → Mod 也得 AGPL | Mod 可任意许可 |
| 商业模式 | 可做 AGPL + 商业授权双许可 | 放弃收费授权 |
| 防 SaaS 白嫖 | **能** | 不能 |
| 迁移成本 | ≈0 | 需改约 77 处 |

**一句话**：要「任何人（含闭源）都能接、Mod 随便授权」→ Apache-2.0；要「防白嫖 + 保留商业授权」→ AGPL-3.0。

---

## 8. 待确认

**无。** 全部已裁决，按 §5 波次执行。

> 已确认：前端 = **Flutter Web**（本机可构建）；**动作不做**；**本地模型本体不碰**，只要 OpenAI 标准接口；
> 许可 = **AGPL-3.0-only 保持现状**（W1 取消）。

---

## 9. 风险与应对

| 风险 | 应对 |
|---|---|
| Flutter Web 平台视图层级 / 点击穿透 | **已发生（2026-09-11）**：当初写「控件不叠其上」，实现却把设置面板/断线横幅/缩放角标/错误重试全叠在了舞台上 → 全部「看得见、点不着」。最终选的不是「不叠」，而是调研给的另一条路 **「让 iframe pointer-events 可控」的按盒子版本**：`StagePointerInterceptor`（透明 `<div>` 平台视图垫层）。**规矩：压在舞台上的可交互控件必须套它**，见 `CHANGELOG.md` v0.5.1 §12 |
| 许可迁移触碰约 77 处 + 版权归属 | 脚本批量改 + `grep` 门禁；外部贡献者逐一确认 |
| 口型与音频不同步 | 以收到的 PCM 为唯一真源计算 RMS，不用播放时钟反推 |
| 你提供的端点非标准 OpenAI 实现 | 探测阶段即暴露（`/v1/models` 或 `/api/tags`），失败只 disable 该 Mod 不影响主链路 |
| Flutter Web 首帧/加载慢 | 舞台先出加载态，模型就绪前不阻塞聊天 |

---

## 10. 变更历史

- 2026-09-10：初稿（只做核心链路 + Mod）。
- 2026-09-10：v2 —— 动作本轮不做；许可改为推荐 Apache-2.0。
- 2026-09-10：v3 —— **本地模型本体从范围移除**；两个本地 Mod 降级为「指向 OpenAI 标准接口的探活 + 写配置」；W0 收敛为「Flutter SDK 已就绪」。
- 2026-09-10：v4 —— **许可裁决为 `AGPL-3.0-only`（保持现状），W1 取消**；开始执行 W2→W6。
- 2026-09-10：v5 —— **TTS 选型待定**：删除 TTS 相关配置（`[tts]` 段、`local-tts` 默认启用）；运行时允许 TTS 缺失并走空句路径；LLM 保持 OpenAI 标准接口。
