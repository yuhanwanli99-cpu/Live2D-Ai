# BATCH-0367 · ⚠ **一个诚实的否定结果**：渲染面的 `message` **接收端不在这个 crate 里**

Phase 1 · 域覆盖 · `l2d-wasm-demo` —— F-0046-01 撤回后，其**实质**（origin 检查）在哪

## 跑的命令（全部只读）
```
grep -rn "addEventListener|\"message\"" crates/l2d-wasm-demo/src/ --include=*.rs
grep -rn "set_onmessage|onmessage|Closure::wrap" crates/l2d-wasm-demo/src/ --include=*.rs
git ls-files crates/l2d-wasm-demo | grep -vE "\.rs$"
grep -nE "addEventListener|message|postMessage|origin" crates/l2d-wasm-demo/index.html
cat crates/l2d-wasm-demo/Trunk.toml
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⚠ **一个必须如实说的否定结果**
| 搜索 | 结果 |
|---|---|
| `addEventListener` / `"message"` in `src/**/*.rs` | **零命中** |
| `set_onmessage` / `onmessage` / `Closure::wrap` in `src/**/*.rs` | **零命中** |
| `index.html` 里的 `message` / `postMessage` / `origin` | **零命中** |
| `git ls-files crates/l2d-wasm-demo` 的非 `.rs` 文件 | `Cargo.toml` · `README.md` · **`Trunk.toml`** · **`Trunk.toml.bak`** · **`index.html`** |
| `Trunk.toml` | **`target = "index.html"`**（**唯一入口**）· `dist = "dist"` · `release = true`（P0-1a「debug WASM 23MB 会低帧」） |

⇒ ⇒⭐⭐⭐ **结论**：**trunk 只有一个 HTML 入口、且那个 HTML 里没有 `message` 监听**
⇒ ⇒⇒ **这个 crate 里没有应用自有的 JavaScript** ⇒ ⇒⇒
⇒⇒⇒ **渲染面的 `message` 接收端不在 `l2d-wasm-demo` 里。**
⇒ ⇒⇒⇒ **而父页在往 iframe 里 `postMessage`**（`live2d_host_web.dart:78`，
**B0275 已核**它带的是 **`web.window.location.origin.toJS`**、**不是 `"*"`**）
⇒ ⇒⇒⇒ ⇒ **接收端在别处**（`l2d` crate？生成的 glue？或渲染面被别的方式驱动）

⇒ ⇒ **而这对 F-0046-01 的意义要说清**
| 结论 | 状态 |
|---|---|
| 我 B0275 引用的那段代码（`main.rs:380-386` / `:705-707`）**已不在树上** | ✅ **撤回仍然正确** |
| 「这个通道现在**没有** origin 防护」 | ❌ **不能这么说** —— **接收端根本没在这个 crate 里** |
| **接收端在哪、有没有 origin 校验** | ⚠ **未核实** —— 而**现在我知道它不在哪**了 |

⇒ ⇒ ⭐ **而这条未核实项因此变成我账本里最具体的一条**：
**排除清单已建立**（`src/**.rs` 无 · `index.html` 无 · `Trunk.toml` 无第二入口）
⇒ ⇒ **下一个该问的地方**：`crates/l2d/src/**`（AGENTS 说 `l2d` 提供渲染核心）· 生成的 glue（`dist/`，按我的排除清单**不审**）
⇒ ⇒ ⚠ **按我自己的规矩：拿不出证据就不进 FINDINGS** ⇒ **本条只入「未核实」**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. ⭐⭐ **渲染面的 `message` 接收端在哪**（**排除清单已建立** —— 见上表）
2. `UiSignals` 定义本体 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 ·
   `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义 · `PersonaCardFilePicker` typedef 注释
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1790 / `live2d_stage` ~580 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~425）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
