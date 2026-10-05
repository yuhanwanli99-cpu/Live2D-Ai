# BATCH-0281 · ⭐ 追到了底：规则是**写成禁令放在边界上**的，而 `_with_load` 是**下一层的原语**

Phase 1 · 域覆盖 · 回读 `l2d-wasm-demo` 余面 + `l2d/src/renderer/model_core.rs`（B0280 留的线）

## 跑的命令（全部只读）
```
grep -rn "render_to_view_submit_with_load|render_to_view_submit" crates/ --include=*.rs
sed -n '10,14p' crates/l2d-wasm-demo/src/web/surface.rs
sed -n '144,152p' crates/l2d/src/renderer/model_core.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ **B0280 那条线**闭合，且解出三件事
### ① **规则是写成禁令、放在边界上的**（不是我以为的「约定」）
```rust
// l2d-wasm-demo/src/web/surface.rs:12-13
//! 实时路径只调用 **`render_to_view_submit`**（**不得调用阻塞的 `render_to_view`**
//! ——C1/C6/C8 **硬门禁**）；单帧契约见 [`render::tick`]。
```
⇒ ⇒ 禁令**带编号**（C1/C6/C8）⇒ ⇒ **可被引用、可被核对**，不是「我们打算只用它」
⇒ ⇒ ⭐ **又一次「禁令要指名被禁的那个」**（B0229 P26-a · B0235 P26-b · B0254「不说 X 而说会怎样」同族）

### ② 而 `_with_load` 是**下一层的原语**，`render_to_view_submit` 是**它上面的具名包装**
```rust
// l2d/src/renderer/model_core.rs:144-152
pub fn render_to_view_submit(&mut self, view, format) -> … {
    // **兼容入口：历史行为 = 清成全透明**（离屏读回 / 桌面壳依赖它）。
    self.render_to_view_submit_with_load(view, format, …)
}
```
⇒ ⇒ **B0280 的疑问有了解**：「`_with_load` 没在这个文件出现」是因为**它在下一层**
（`render.rs:413` 调的就是它）⇒ ⇒ **又一次「顺着痕迹追」得到答案（B0274 规则）**

### ③ ⭐ 而**头注没写「包装关系」**⇒ 留下一个**可查证的阅读障碍**
`surface.rs:12` 与 `render.rs:2`/`:349` 都说「只调用 `render_to_view_submit`」，
而 `render.rs:413` 的真实调用是 **`render_to_view_submit_with_load`**
⇒ ⇒ 单看这两处**像自相矛盾** ⇒ ⇒ **但真相是包装关系** ⇒ ⇒ **而这个关系没有任何一处头注写明**
⇒ ⇒ 读者必须**自己推出**「前者是后者的包装」
⇒ ⇒ **建议（一句话、零风险）**：在 `surface.rs:12` 或 `render.rs:349` 补一句
「**`render_to_view_submit` 内部即 `render_to_view_submit_with_load`；实时路径只应经前者**」
⇒ ⇒ ⚠ **不记发现**（不是缺陷，是**读法成本**），但**值得写进账本** ⇒ 因为本审计自己就撞上了它

## 未核实项
1. ⭐ `render.rs:413` 的**第三个参数**（`con…`）是什么、以及背景分支怎么选 —— 未读
2. `web/surface/gpu.rs`(168) 与 `main.rs`(962) 未读
3. `apply_bridge_effects` 的**调用顺序**未核（B0279 留）
4. `stage-bg.dataUrl` 的**发送方**在 Flutter 侧哪一处 —— 未核
5. 真实动作计划的 token 长度（F-0020-01 永久敞口）· `shell/flutter` 是否真可能出现密钥
6. 那四个「未知 id」在文档里有没有被点名（B0270 敞口）· `refresh_from_disk` 敞口（B0266）
7. 能否点名会在持锁期间 panic 的 Mod（B0268 敞口）
8. 面板余面：`dev_tools_section` ~1845 · `live2d_stage` ~600 · `memory_panel` ~545 ·
   `persona_panel` ~520 · `message_bubble` ~470 · `chat_panel` ~465 · `error_banner` ~15
9. 其余 6 处 `mod_count_is_*` 逐条对账未核 · `ErrorResponse` 定义未读
10. `onReady` 是否间接清过 `presetStatus`（F-0263-01 反证 (a)，**结论不依赖它**）
11. **两侧措辞同步无机制**（B0260 敞口）· `chat_session.dart` 的 `maxSessions=50` 施加点未读
12. 前端 `.dart` 仍 181 未读（**机械值**）；Mod crates 42 未读；`mod-system` 余 8 文件未读
13. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
14. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
15. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
