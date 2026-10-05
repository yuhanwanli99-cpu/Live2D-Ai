# BATCH-0258 · `ErrorDetail.details` 是自由字段，但**两处都只塞枚举/状态**，没塞提交值（0 条新发现）

Phase 1 · 域覆盖 · `web_api/dto.rs` 的 `ErrorDetail` + 其两个 `details` 产出点

## 跑的命令（全部只读）
```
grep -rn "pub struct ErrorDetail" -A 12 crates/ --include=*.rs
grep -rn "with_details|\.details\b|details:" crates/ --include=*.rs | grep -v test
sed -n '250,268p' web_api/security.rs ; sed -n '186,193p' web_api/settings_routes/mod.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ 核一处**看起来危险**的自由字段
```rust
pub struct ErrorDetail {
    pub code: &'static str,                                  // :159
    pub message: String,                                      // :161
    /// 附加字段（`field` / `value` / `id` 等；…）              // :162  ← 名字里**有 `value`**
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,                   // :164
}
```
**两个产出点，逐一核**：
| 产出点 | 塞了什么 | 判定 |
|---|---|---|
| `security.rs:255-259`（**mutating 校验 403**） | `json!({"status": err.as_status()})` | ⭐ **只有一个状态数字** ⇒ **没有把被拒的 body 回显** —— 而这**正是**最容易被写成「回显被拒内容」的那条路 |
| `settings_routes/mod.rs:189-193`（`api_key_env` 非法） | `json!({"section": llm\|tts})` | ⭐ **只有段名** ⇒ **不回显那个非法的变量名** |

⇒ ⇒ **文档点名「`value`」的那个字段，实际两处都没用**
⇒ ⇒ ⭐ 由此得到一条：**「有一个自由字段」本身不是缺陷，「往里塞了什么」才是**
⇒ ⇒ 而这两处塞的是**枚举 / 状态数字**，不是**提交值** ⇒ ⇒ **红线 R 在这个口上是关着的**

### ⚠ 但我**不夸大**：一处仍未核
`message: String` 是**自由文本**，而 `settings_routes` 的 `msg` 由**别处**拼出
（`:186` 判的是 `msg.contains("api_key_env 非法")`）
⇒ ⇒ **那个 `msg` 里会不会带上用户填的变量名/URL，我未核** ⇒ 列入未核实项
⇒ ⇒ 而**这恰恰是 `details` 之外的另一个自由文本口** ⇒ 「两个自由字段，一个核了（`details`），一个没核（`message`）」

## 未核实项
1. ⭐ **`msg` 的产地**（`settings_routes` 那两处的 `message` 会不会回显用户填的值）—— **本批明确未核**
2. `ErrorResponse` 的定义未读
3. `dev_tools_section.dart` 余 ~1845 · `memory_panel.dart` 余 ~560 · `persona_panel.dart` 余 ~520 ·
   `message_bubble.dart` 余 ~480 · `chat_panel.dart` 余 ~480 · `live2d_stage.dart` 余 ~615 ·
   `error_banner.dart` 余 ~15 未读
4. `chat_session.dart` 的 `maxSessions = 50` 截断施加点未读
5. 前端 `.dart` 仍 181 未读（**机械值**）；Mod crates 42 未读；`mod-system` 余 8 文件未读
6. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
7. **iframe 重建后宿主是否主动发 `clear`**（B0251 留）—— 未核
8. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
9. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
