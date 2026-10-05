# BATCH-0374 · ⭐⭐⭐ **同一判据，路由层做对了、断言层做漏了** —— 我第一次能给出一条 P1 的「对/错两处对照」

Phase 1 · 域覆盖 · **Rust**（回到服务端）—— `models_routes::handle_import` 的校验（B0373 留）

## 跑的命令（全部只读）
```
grep -rn "fn handle_import|\"import\"|import" crates/live2d-ai-desktop/src/web_api/models_routes/*.rs | head -6
sed -n '137,168p' crates/live2d-ai-desktop/src/web_api/models_routes/handlers.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**F-0001-02 的定位被显著收窄**（结论本身不变，**证据变强**）
```rust
pub(crate) fn handle_import(store: &ModelStore, body_str: &str) -> Response<…> {   // :138
  let req: ImportRequest = match serde_json::from_str(body_str) {
    Ok(r) => r, Err(e) => return bad_request("**invalid_payload**", &format!("请求 body JSON 解析失败: {e}")) };  // :139-144
  let id = match normalize_relative_id(&req.id) {
    Ok(s) => s, Err(e) => return bad_request("**invalid_resource_path**", &format!("id 非法: {e}")) };     // :145-148
  let abs_dir = match resolve_safe_path(&store.assets_root, &id) {
    Ok(p) => p, Err(e) => return bad_request("**invalid_resource_path**", &e) };                              // :150-153
  if !abs_dir.is_dir() { return bad_request("**invalid_resource_path**", &format!("目录不存在: {}", …)); }     // :154-159
  let model3_path = match **find_model3_json**(&abs_dir) {
    Some(p) => p, None => return bad_request("**invalid_model3_json**",
      &format!("目录 {} 下找不到 *.model3.json", abs_dir.display())) };                                       // :160-166
```
⭐ **五道关卡、四个码** ⇒ 而**第 5 道正是 F-0001-02 的现场**

### ① ⭐⭐⭐ 我第一次能给出一条 P1 的「正确 / 错误」两处对照
| 层次 | 代码 | 对「找不到 `*.model3.json`」的处理 |
|---|---|---|
| **路由层** | `handlers.rs:160-166` | ✅ **有码**（`invalid_model3_json`）+ **有文案**（「目录 X 下找不到 *.model3.json」）⇒ **用户会看到** |
| **断言层** | `model_root.rs:111` | ⚠ `assert!(root.join("…/bai.runtime.json").is_file() **\|\| !root.join("bai").is_dir()**)` ⇒ 而 `bai` **不存在** ⇒ **后半段空洞为真** ⇒ ⇒ **那个 assert 守不住任何东西** |
⇒ ⇒⇒ **同一个判据，路由层做对了、断言层做漏了。**
⇒ ⇒ ⇒ **我此前只记了「错的那处」；本批补上了「对的那处」**
⇒ ⇒⇒ ⭐ **而这让 F-0001-02 的修法更窄了**：
**不需要改路由层**（它已经对）**只需要把 `model_root.rs:111` 那条 assert 换成同样的四段判定，或直接删掉**（因为它的职责已被路由层承担）—— **这是一行级改动，而不是设计改动**。

### ② 而这一批也顺手确认了 B0373 的判断
`handle_import` 的头注（`handlers.rs:11`）记着「**写盘失败回滚内存状态**」⇒ ⇒ **而 Dart 侧完全看不到这一层**（B0373 只看到「只发请求」）⇒ ⇒ **「换个文件问」第四次得到印证**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `model_root.rs:111` 那条 assert 的**上下文**（它在哪个函数里、被谁调用）—— **未读**
   ⇒ ⇒ **B0375 的第一件事**：核它**还有没有调用者**（若无 ⇒ 删除即彻底了结）
2. `UiSignals` 定义本体 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 ·
   `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义 · `PersonaCardFilePicker` typedef 注释
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~580 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~425）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
