# Stabilize 收束报告 —— 为「用户 Win 真实点火」备好闭环（2026-09-15）

> **这不是 release 文件**。本波**不发布**：不 push、不打 tag、不写
> `docs/releases/`、**不 bump 版本**（三处版本行一行未动，仍是 **`0.2.0-rc.3`**）。
>
> - **真源**：`/home/skystar/Live2D-Ai-wave3` @ `57e32ae5`（Wave 3 已验收）
> - **分支 / worktree**：`mod/stabilize` @ `/home/skystar/Live2D-Ai-stabilize`
> - **稳定化代码 tip**：**`0d33aa5b`**
> - **范围真源**：任务书（本波无独立 PLAN 文档，逐条按任务书必做/禁止执行）
> - **口径**：FACTORIES / `mod_count_is_seven` / 缺省 manifest / 版本号 /
>   `l2d-wasm-demo` framebuffer 与 stage_bg 渲染算法 —— **全部未动**。

---

## 1. 一句话

代码侧把「用户点 ignite 后**照单能验**」做成闭环：**修好一个挡点火的真 bug**
（前端 Mod 管理启停恒 415），产出**一份可勾选的点火清单** + **一份可跑的预检脚本**，
并**强制加做无头浏览器自点火**（本项目「测试全绿 ≠ 界面是对的」那条教训）。

---

## 2. 改了什么（4 个文件，最小 diff）

| 文件 | 改动 | 为什么挡点火 |
| --- | --- | --- |
| `crates/live2d-ai-desktop/src/web_api/mods_routes.rs` | **无 body 的 `enable`/`disable` 不再要求 `Content-Type: application/json`**（仅带 body 时校验）；补 3 条回归 | **前端「Mod 管理」的启用/停用整个点不动** |
| `docs/voice-input.md` §7 | enable 的 curl 补 loopback `Origin` | 原命令在缺省配置下**必失败**（无 Origin → 403） |
| `docs/examples/voice-sidecar/README.md` §3.2 | 同上 | 同上（这是用户照抄的第一步） |
| `scripts/ignition-precheck.sh`（新增） | loopback 预检脚本，输出 PASS/FAIL/SKIP 表 | 清单 §2 的机器预检 |

### 2.1 那个 bug（本波唯一的代码修复）

`POST /api/v1/mods/{id}/enable|disable` 是**无 body 控制命令**，但 `mods_routes`
**无条件**要 `application/json` → `415`。而 Flutter `ModsApi.setEnabled`
（`shell/flutter/lib/api/mods_api.dart`）发的正是「POST + 无 body + 无 Content-Type」。

**无头浏览器实测（修复前）**：

```
fetch('/api/v1/mods/memory/enable', {method:'POST'})                          → 415
fetch('/api/v1/mods/memory/enable', {method:'POST', headers:{'Content-Type':'application/json'}}) → 200
```

即：**用户在界面里点不动任何 Mod 开关**——而清单里「启用 X Mod」是每一步的前置。
**修复后**同一条无 CT 请求 → **200**。

修法是让口径与项目**已有的** mutating 契约一致
（`security::check_mutating_request`：无 body 放行 CT、**Origin 才是 CSRF 闸门**）：

- 无 body + 有 loopback Origin → 放行（200）；
- 无 body + 无 Origin（且未开 allow_no_origin）→ **403 `origin_required`**（**不是 415**，错误面给对）；
- **带 body + 非 JSON CT → 仍 415**（CSRF 防护不回退）。

### 2.2 回归（`mods_routes` 共 14 测全绿，+3）

- `bodyless_enable_disable_skip_content_type_check`
- `body_with_non_json_content_type_is_415`
- `bodyless_without_origin_is_403_not_415`

---

## 3. 预检表（同 tip 实跑）

| 检查 | 命令 | 结果 |
| --- | --- | --- |
| 官方点火体检 | `./scripts/ignite.sh --check` | **4/4 ok** |
| 本波预检 | `./scripts/ignition-precheck.sh --fsm` | **PASS 34 / FAIL 0 / SKIP 1** |
| Rust 测试 | `cargo test --workspace --all-targets` | **1081 / 0**（Wave 3 基线 1078） |
| Rust doc | `cargo test --doc --workspace` | **3 / 0** |
| 格式 | `cargo fmt --all -- --check` | **clean** |
| Clippy | `cargo clippy --workspace --all-targets -- -D warnings` | **0 warning / exit 0** |
| Rust 占比 | `cargo run -p xtask -- rust-ratio` | **96.2897% PASS** |
| 两侧 sidecar | `--selftest` | voice **70 项** / bilibili **16 项** 全过 |
| 前端 | — | **未改 Dart**，按约定未跑 flutter analyze/test |
| 无头浏览器 | 见 §4 | 壳 + 舞台（WebGPU 60FPS，画面在动）+ 一轮对话正文上屏 + 零外部请求 |

**唯一 SKIP**：token 鉴权项（本机未配 `EXTERNAL_INPUT_TOKEN`）。

完整逐项表：[`../../legacy/plans/STABILIZE-PRECHECK-RESULT.md`](../../legacy/plans/STABILIZE-PRECHECK-RESULT.md)。

---

## 4. 无头浏览器自点火（强制加做，本波最硬的证据）

环境：无系统浏览器 → 装 Playwright Chromium；用 **SwiftShader Vulkan** 拿到 WebGPU
适配器（默认 headless 只有 WebGL2，而该渲染面的 wgpu 只有在 WebGPU 适配器存在时才起得来）。

| 证据 | 值 |
| --- | --- |
| 零外部请求 | 一次加载 16 个请求**全部** loopback（含 CanvasKit wasm + CJK 字体），无 gstatic |
| 舞台真渲染 | HUD `GPU: webgpu/WebGPU`、`FPS 60.1`、`bg: solid` |
| **画面在动** | 两帧相隔 1.6s：舞台区 **1.29%** 像素变化 / 右侧面板 **0.0000%**（静止 UI + 呼吸眨眼） |
| 主链 | 一轮对话：LLM 正文上屏并落盘 `{"unfinished":true}`（TTS 未起） |
| 错误契约 | `code=tts_transport stage=tts fatal=true` + 界面「去语音合成设置」 |
| Mod 启停 | 页面内无 CT 的 `enable` → **200**（修复点） |

**没做的**（脚本代替不了）：真实听感、口型肉眼、壁纸观感、桌宠真窗、
Windows Chrome/Edge 的 WebGPU 差异。**不声称「真实点火已完成」**。

---

## 5. 交付物

| 给谁 | 文件 |
| --- | --- |
| **给用户勾选** | [`../../legacy/plans/IGNITION-CHECKLIST-stabilize.md`](../../legacy/plans/IGNITION-CHECKLIST-stabilize.md) |
| 给审查 | [`../../legacy/plans/STABILIZE-PRECHECK-RESULT.md`](../../legacy/plans/STABILIZE-PRECHECK-RESULT.md) |
| 机器预检脚本 | `scripts/ignition-precheck.sh`（`--fsm` 可选） |

### 5.1 留给用户的肉眼 / Win 项（清单 §3）

§3.0 打开页面 · §3.1 舞台模型可见且会动 · §3.2 聊天一轮（**有/无 TTS 两种期望**）·
§3.3 external-input 注入 + 停用 403 · §3.4 B 站 sidecar（dry-run / 可选正式）·
§3.5 voice-input（dry-run / selftest / transcript 注入）· §3.6 wallpaper ·
§3.7 memory · §3.8 persona（好卡/坏卡）· §3.9 pet-desktop（软闭环）· §3.10 director（只观察）。

---

## 6. 与 Wave 3 §8 未决的对照（本波处置）

| Wave 3 未决 | 本波 |
| --- | --- |
| §6.1 活服务/肉眼验收仍是缺口（降级到 rc.4） | **部分收口**：agent 侧脚本 + 无头浏览器证据已备；**肉眼项清单化**留给用户（§5.1）。**仍未做真实 TTS 验收** |
| §7.1 活服务 / 肉眼验收 | 同上 |
| §7.2 director 投递通道 | **未做**（本波不做新功能；`channel:"none"`、`delivered:false` 实测保持） |
| §7.3 版本与发布 | **未做**（不 bump、不写 release） |
| §7.4~7.6 记忆/壁纸工程化、桌宠硬闭环 | **未做**（范围外） |
| §7.7 push | **保持未 push** |
| §8 归档点 | 见下 |

---

## 7. 归档点

- 集成 tip：`mod/stabilize`（**本文件所在提交即最终 tip**；之后不再追加任何改动）。
- 稳定化**代码**提交 `0d33aa5b`（修复 + 预检脚本 + 两条 doc curl）；
  其后一个提交 `91c27fa9` 只追加三份文档 + `docs/README.md` 索引。
- 查最终 tip：`git -C /home/skystar/Live2D-Ai-stabilize rev-parse --short HEAD`。
- 回滚：本波是**最小 diff**（4 个文件），`git revert` 或整体丢弃 `mod/stabilize` 都可；
  不影响其余 wave worktree。
- **未 push**。

---

## 8. 版本号：只提议，不执行

- 当前三处（`Cargo.toml` / `shell/flutter/pubspec.yaml` / `README` 首屏）
  仍统一为 **`0.2.0-rc.3`**，本波**一行未改**。
- **建议**：若用户在本轮聊天里明确说「改成 0.2.0」，再单独做一次
  「三处同步 + AGENTS「当前版本」段 + `docs/releases/v0.2.0.md`」的收束提交；
  **在此之前不动**。
- **不建议**在没有用户确认时 bump：本波只做了稳定化，没有新增能力，
  把它标成稳定版会把「肉眼未验收」这件事藏起来。
