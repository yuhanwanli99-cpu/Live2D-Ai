# BATCH-0277 · 🔻 **我更正自己撤回时的一句话** —— 通道在，只是形态变了（B0274 规则第三次生效）

Phase 1 · 域覆盖 · 回读 `l2d-wasm-demo` 余面（`web/surface/{background,input,render,idle,gpu}.rs` + `main.rs`）

## 跑的命令（全部只读）
```
wc -l crates/l2d-wasm-demo/src/web/surface/*.rs .../gpu.rs .../main.rs
grep -rnE "stage_bg|stage-bg|StageBg|background" crates/l2d-wasm-demo/src/web/ --include=*.rs
sed -n '96,108p' web/surface/input.rs
grep -rnE "addEventListener|\"message\"|onmessage" crates/l2d-wasm-demo/src/ --include=*.rs
grep -rnE "emit_event" crates/l2d-wasm-demo/src/main.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**更正 F-0046-01 撤回里的一句错话**（0 条新发现）
### ① 我在 B0275 写「**这一版没有这条通道**」—— **这句话是错的**
```
input.rs:100-102  /// **stage-bg.dataUrl**（自定义背景图 dataURL；None = 无背景图）。
                 /// 应用为 canvas CSS background-image（cover 缩放）…
                 pub bg_data_url: Option<String>,
```
⇒ ⇒ **通道在。** 准确的三段形态是：
| 方向 | 形态 |
|---|---|
| **发送侧**（Flutter→iframe） | **仍用 `postMessage`**，但 `live2d_host_web.dart:78` 带 **`web.window.location.origin.toJS`** ⇒ **不是 `"*"`** |
| **接收侧**（iframe→Rust） | **版本化 JSON 消息的字段**（`stage-bg.dataUrl`）⇒ **`bg_data_url: Option<String>`** |
| **回程**（Rust→父页） | **`emit_event`**（`main.rs:151/618/675/682`）⇒ **不再由 Rust 调 `postMessage`** |
⇒ ⇒ **撤掉的是「我引用的那段代码」与「Rust 侧直接调 postMessage」，不是「这条通道」**

### ② ⭐ 而这**正是 B0274 规则的第三次生效**
B0274 立的规则是「**别让一个否定结果硬化成肯定断言**」，形态是「**凭零命中**」
⇒ ⇒ 本批是它的**第三次现身**，而这次**反噬的是我自己写下的撤回措辞**：
⇒ ⇒ ⚠ **我为了让撤回显得干脆，把「我没找到」写成了「它不存在」**
⇒ ⇒ ⭐ **这比原先那条发现错得更隐蔽**（它藏在一条**看起来严谨**的撤回里）

### ③ ⭐ 而本批顺手带出一条**文档与实现的口径问题**（**列为未核实，不记发现**）
`input.rs:101` 说背景「应用为 **canvas CSS background-image**」
⇒ 而 **AGENTS rc.4 ⑦ 逐字说**：「canvas 的 CSS `background-color` / `background-image` 写入**已删除**、
`stage_css.rs` 随之删除」
⇒ ⇒ **两者不一致**：要么 AGENTS 这一句过期、要么 CSS 路径**已复活**、要么这条**注释本身**过期
⇒ ⇒ **我未读真正「应用」它的代码**（`background.rs` / `input.rs:239` 起）⇒ **不记发现**，列入未核实

## 未核实项
1. ⭐ **背景到底走 CSS 还是 framebuffer**（`input.rs:101` 说 CSS · AGENTS rc.4 ⑦ 说已删）——
   **需读 `web/surface/background.rs` 与 `input.rs:239` 起**
2. `web/surface/{render,idle,gpu}.rs` 与 `main.rs`(962) 未读
3. `stage-bg.dataUrl` 这条消息的**发送方**在 Flutter 侧哪一处（`live2d_bridge.dart`?）
4. 真实动作计划的 token 长度（F-0020-01 撤回的永久敞口）· `shell/flutter` 是否真可能出现密钥
5. 那四个「未知 id」在文档里有没有被点名（B0270 敞口）· `refresh_from_disk` 敞口（B0266）
6. 能否点名会在持锁期间 panic 的 Mod（B0268 敞口）
7. 面板余面：`dev_tools_section` ~1845 · `live2d_stage` ~600 · `memory_panel` ~545 ·
   `persona_panel` ~520 · `message_bubble` ~470 · `chat_panel` ~465 · `error_banner` ~15
8. 其余 6 处 `mod_count_is_*` 逐条对账未核 · `ErrorResponse` 定义未读
9. `onReady` 是否间接清过 `presetStatus`（F-0263-01 反证 (a)，**结论不依赖它**）
10. **两侧措辞同步无机制**（B0260 敞口）· `chat_session.dart` 的 `maxSessions=50` 施加点未读
11. 前端 `.dart` 仍 181 未读（**机械值**）；Mod crates 42 未读；`mod-system` 余 8 文件未读
12. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
13. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
14. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
