# BATCH-0256 · ⭐ 四个 `model_not_found` 站点**全走同一个写死 404 的函数** ⇒ 码↔状态是**结构上单值**的

Phase 1 · 域覆盖 · `models_routes/handlers.rs`（把 B0254 留下的该项核完）

## 跑的命令（全部只读）
```
grep -n "fn not_found_response" -A 3 handlers.rs
grep -n "not_found_response(" handlers.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ **一条比 B0254 更强的性质**
```rust
pub(crate) fn not_found_response(code: &'static str, message: &str) -> Response<…> {
    json_error_response(**StatusCode(404)**, code, message)          // :399-401
}
```
四个调用点 `:133` `:254` `:325` `:371` **全部**走它，**都传 `"model_not_found"`**
⇒ ⇒ **该码在任何站点都恒为 404** ⇒ **没有任何站点能漂移**

⇒ ⭐ **比 B0254 记的更强**：B0254 只记了「该码出现在 4 处生产代码」；
⇒ **现在核到的是「4 处全被一个写死状态的函数收口」**
⇒ ⇒ **码↔状态的配对是**结构上单值**的**，不只是「靠约定保持一致」

⇒ ⇒ **P2 的第五种落点**（也是最纯的一种）：**状态与码的绑定由一个函数保证**
（`not_found_response` 写死 404 · `conflict_response` 写死 409）
⇒ ⇒ **任何调用点都改不了那个状态码，除非改那个函数** —— 而**改那个函数是显眼的改动**
⇒ ⭐ 与 **B0159 的 `ErrorKind::code()`** 构成一组**方向相反的同形**：
那里是**码由变体算出来**（status 另说），这里是**状态由帮助函数写死**（码是参数）
⇒ ⇒ **两侧都是「让配对由一处保证」**。

## 未核实项
1. `json_error_response` 本体未读（它是否**只**做「拼 `{code,message}` 的 JSON + 设状态」，
   **有没有可能改写 code** ⇒ 那是这条结论的**最后一块地基**）
2. `dev_tools_section.dart` 余 ~1845 · `memory_panel.dart` 余 ~560 · `persona_panel.dart` 余 ~520 ·
   `message_bubble.dart` 余 ~480 · `chat_panel.dart` 余 ~480 · `live2d_stage.dart` 余 ~615 ·
   `error_banner.dart` 余 ~15 未读
3. `chat_session.dart` 的 `maxSessions = 50` 截断施加点未读
4. 前端 `.dart` 仍 181 未读（**机械值**）；Mod crates 42 未读；`mod-system` 余 8 文件未读
5. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
6. **iframe 重建后宿主是否主动发 `clear`**（B0251 留）—— 未核
7. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
8. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
