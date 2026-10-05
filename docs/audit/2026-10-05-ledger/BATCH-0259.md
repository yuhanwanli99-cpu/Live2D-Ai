# BATCH-0259 · ⭐ 链子追到底：**码↔文案的绑定是「按消息前缀判」**（脆弱，但**在决策点被写明了**）

Phase 1 · 域覆盖 · `settings_routes/mod.rs:176-195` + `apply_patch` 的错误串（B0258 留的另一半）

## 跑的命令（全部只读）
```
sed -n '170,190p' crates/.../settings_routes/mod.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；两个**被如实记下**的观察
### ① `msg` 的产地 = `apply_patch` 的错误串，而**码是靠「消息前缀」判的**
```rust
let (next, outcome) = apply_patch(current, patch).map_err(|msg| {
    // **apply_patch 返回的 String 形态错误码语义不细；按消息前缀判。**     // :185
    if msg.contains("base_url 非法") { … "url_invalid" … }
    else if msg.contains("api_key_env 非法") { … "invalid_env_name" … }
```
⇒ ⇒ **码 ↔ 文案的绑定是「在人类可读字符串上做子串匹配」** ⇒ ⇒ **脆弱**：改了措辞，码就静默变错
⇒ ⇒ **但这不是疏漏** —— 它**在决策点被写明了**（「语义不细；按消息前缀判」）⇒ **是已知的、被接受的限制**
⇒ ⭐ **与 B0159 `ErrorKind::code()`（`code()` 由**枚举变体**算出）形成一组鲜明对照**：
**一边是「码是算出来的」，一边是「码是从措辞里猜出来的」** ⇒ ⇒ 而**两边都把自己的形态写在脸上**
⇒ ⇒ **可提炼**：**知道自己弱在哪，并写在旁边**，比假装强更有用。

### ② 而 `message` **可能**会回显用户填的值（B0122 已核 `apply_patch` 的文案形态）
B0122 我读过 `apply_patch` 的校验文案形态，含 `{raw:?}` 形态的插值
⇒ ⇒ 若 `api_key_env 非法` 的文案也带上那个值，则 **message 里会出现用户填的变量名**
⇒ ⇒ **但这不违反红线 R**：那是**环境变量「名」**（如 `DEEPSEEK_API_KEY`），**不是密钥本身**
⇒ ⇒ 而 B0123 已核 `SettingsView` **刻意不暴露** `api_key_env`（只给 `has_api_key`）
⇒ ⇒ **口径不对称**（视图脱敏 / 错误文案回显）—— **不记发现**：
**值非法时告诉用户「你填的是什么」正是用处**，且此处**只回显名字、不回显密钥**
⇒ ⚠ **如实标注**：**本批未逐字确认** `api_key_env 非法` 那条文案**是否真的带值**
（B0122 读到的是 `base_url` 那条的形态）⇒ **列为未核实**

## 未核实项
1. ⭐ `api_key_env 非法` 的**文案原文**（是否带值）—— 未逐字核
2. `ErrorResponse` 定义未读；`security.rs` 的 `MutatingCheckError::as_message()` 未读
3. 面板余面（~4495 行：`dev_tools_section` 1845 · `live2d_stage` 615 · `memory_panel` 560 ·
   `persona_panel` 520 · `message_bubble` 480 · `chat_panel` 480 · `error_banner` 15）
4. `chat_session.dart` 的 `maxSessions = 50` 截断施加点未读
5. 前端 `.dart` 仍 181 未读（**机械值**）；Mod crates 42 未读；`mod-system` 余 8 文件未读
6. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
7. **iframe 重建后宿主是否主动发 `clear`**（B0251 留）—— 未核
8. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
9. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
