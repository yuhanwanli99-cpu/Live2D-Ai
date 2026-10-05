# BATCH-0260 · ⭐ 文案**确实带上值**（变量名，非密钥）—— 且**两侧措辞共用**正是前缀匹配能成立的原因

Phase 1 · 域覆盖 · `api_key_env` 错误文案的**原文**（B0259 留的第一件事）

## 跑的命令（全部只读）
```
grep -rn "api_key_env 非法" crates/ --include=*.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ **闭合了 B0259 的脆弱性观察的一整条链**
```rust
"{section} api_key_env 非法: **{name:?}**（仅允许 ASCII …）"     // settings.rs:78
"api_key_env 非法: **{name:?}**（仅允许 ASCII …）"                 // patch.rs:334
} else if msg.contains("api_key_env 非法") { … }                    // settings_routes/mod.rs:188
```
四个可核点：
1. ⭐ **文案确实带上值**（`{name:?}`，Debug 形态 ⇒ 带引号）
   ⇒ ⇒ **但值是环境变量「名」**（`MY-KEY` 这类），**不是密钥本身**
   ⇒ ⇒ **不违反红线 R** ⇒ **B0259 的判断得到逐字确认**
2. ⭐⭐ **措辞在两侧共用**（`settings.rs:78` 与 `patch.rs:334` 同一句形状）
   ⇒ ⇒ 而 desktop 侧**正是靠这个字符串做前缀匹配**（`:188`）
   ⇒ ⇒ ⇒ **两侧共用措辞，不是巧合，是前缀匹配能成立的前提**
3. ⭐ 文案**顺带说明了规则**（「仅允许 ASCII …」）⇒ ⇒ 它**是诊断，不只是标识**
4. ⇒ ⇒ **闭合 B0259 的链**：
   「码靠措辞猜」（脆弱）**之所以今天成立**，是因为**措辞在两侧被刻意收敛**；
   ⇒ ⇒ **而一旦有人只改一侧的措辞，匹配就会静默失效**
   ⇒ ⇒ **B0259 那句「语义不细；按消息前缀判」正是他们为这份收敛付的价、且自认的价**

⇒ ⇒ **0 findings**；**但这一批把「脆弱」从一句评论变成了可核的机制**。

## 未核实项
1. **两侧措辞是否真的「总是」同步**（无机制强制，只靠人）⇒ **这是这条链唯一剩下的敞口**
2. `ErrorResponse` 定义未读；`security.rs` 的 `MutatingCheckError::as_message()` 未读
3. 面板余面（~4495 行：`dev_tools_section` 1845 · `live2d_stage` 615 · `memory_panel` 560 ·
   `persona_panel` 520 · `message_bubble` 480 · `chat_panel` 480 · `error_banner` 15）
4. `chat_session.dart` 的 `maxSessions = 50` 截断施加点未读
5. 前端 `.dart` 仍 181 未读（**机械值**）；Mod crates 42 未读；`mod-system` 余 8 文件未读
6. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
7. **iframe 重建后宿主是否主动发 `clear`**（B0251 留）—— 未核
8. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
9. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
