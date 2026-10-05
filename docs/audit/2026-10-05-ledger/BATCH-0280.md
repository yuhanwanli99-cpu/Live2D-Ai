# BATCH-0280 · ⭐ F-0278-01 的敞口**答了**；而**冒出来的第二个注入面**是**守得最严的一处**

Phase 1 · 域覆盖 · 回读 `l2d-wasm-demo` 余面 —— `web/surface/render.rs`(551) 头注 + `input.rs` 的注入面

## 跑的命令（全部只读）
```
sed -n '1,14p' crates/l2d-wasm-demo/src/web/surface/render.rs
grep -rnE "set_property|setProperty|background_image|background-image" crates/l2d-wasm-demo/src/ --include=*.rs
sed -n '292,314p' crates/l2d-wasm-demo/src/web/surface/input.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ **F-0278-01 的唯一未核项结清**
### ① 「全 wasm 树还有没有活的 CSS 背景写」⇒ **没有**
`grep` 六处命中里，`input.rs:101`（F-0278-01 那条过时注释）、`:239`（正确说明）、
`background.rs:203`（「与旧的 CSS background-image …」）**都是注释**；
**唯一含 `background-image` 字面的活代码是 `input.rs:311` —— 而它在 `#[test]` 的黑名单里**
⇒ ⇒ **F-0278-01 确实只是注释问题**，**成立、不扩大**

### ② ⭐ 而**第二个注入面**（`input.rs:296-311`）是**整个文件里守得最严的一处**
```rust
/// **注入面**：这个值会进 `style.setProperty("background-color", …)`，
/// 所以**只认纯十六进制**。任何其他构造一律拒绝（返回 None = 回落默认色）。
#[test]
fn stage_color_rejects_anything_that_is_not_plain_hex() {
    for bad in ["", "#", "#12", "#12345", "#1234567",   // 形状：空 / 截断 / 长度错
                "#gggggg",                                // 字符集
                "red", "rgb(1,2,3)",                      // CSS 关键字与函数
                "url(https://example.com/x.png)", "var(--x)",   // 外链与间接引用
                "#fff; background-image: url(x)",         // ⭐ **声明拼接**
                "#fff}body{background:red",               // ⭐ **规则逃逸**
                "expression(alert(1))"],                  // ⭐ 老式 JS 表达式
```
⇒ ⇒ ⭐ **五类注入面被逐条列为反例**，且**测试名就把判据写成了白名单**
（「rejects **anything that is not plain hex**」）
⇒ ⇒ ⭐ **注释先说「这个值会去哪里」，再说「所以只认什么」** ⇒ **先知道去处、再定判据**
⇒ ⇒ 这与 B0213 的读法**互为镜像**：B0213 是「**先挡 `NaN` 再转换**」，
这里��「**先知道去处再定判据**」⇒ ⇒ **同一条纪律的两种用法**
⇒ ⇒ ⭐ 而「拒绝 ⇒ `None` = **回落默认色**」⇒ ⇒ **失败有确定的落点，不是任意值**
⇒ ⇒ 与 B0238 的「闸门型字段必须有定值」**同族**

### ③ 顺带：`render.rs:1-2` 的头注把**单帧状态机契约**写在头里
> 「取交换链纹理 → **`render_to_view_submit`** → present，以及 surface 生命周期契约（C4）与运行时 HUD。」
> `:4-6`「Suboptimal 先 render/present 再 configure；Outdated 无 texture 直接 configure；
> Lost configure + 同步视口；Timeout/Occluded 跳过；**Validation 连续两次视为 fatal 并停循环**（P0-2-2）」
⇒ ⇒ **四种 Surface 状态 + 「连续两次」这个阈值**都写在头注 ⇒ ⇒ **又一次「阈值写在决策点」**（B0242 的 `actions` 0–2 同族）

## 未核实项
1. ⭐ **一处值得追的线**：`render.rs` 头注写的是 **`render_to_view_submit`**，
   而 AGENTS rc.4 ⑦ 说背景走 **`render_to_view_submit_with_load`**（`LoadOp::Load`，只叠 Live2D、**不再 clear**）
   ⇒ ⇒ **带 `_with_load` 的那个变体在这个文件里没出现** ⇒ **它在哪被调用？** ⇒ **下一批第一件事**
2. `web/surface/gpu.rs`(168) 与 `main.rs`(962) 未读
3. `apply_bridge_effects` 的**调用顺序**未核（B0279 留）
4. `stage-bg.dataUrl` 的**发送方**在 Flutter 侧哪一处 —— 未核
5. 真实动作计划的 token 长度（F-0020-01 永久敞口）· `shell/flutter` 是否真可能出现密钥
6. 那四个「未知 id」在文档里有没有被点名（B0270 敞口）· `refresh_from_clock` 敞口（B0266）
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
