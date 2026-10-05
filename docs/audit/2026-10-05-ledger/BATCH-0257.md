# BATCH-0257 · ⭐ 地基成立，且比预期更硬：**`code: &'static str`** ⇒ 站点**无法**自造拼写

Phase 1 · 域覆盖 · `models_routes/handlers.rs`（B0256 留的最后一块地基）

## 跑的命令（全部只读）
```
grep -n "fn json_error_response" -A 12 handlers.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；**B0256 的结论现已完全落地**
```rust
fn json_error_response(status: StatusCode, **code: &'static str**, message: &str) -> … {   // :411-415
    let detail = ErrorDetail::new(code, message);                    // :416  ← **原样传，不改写**
    let body = serde_json::to_vec(&ErrorResponse::new(detail)).unwrap_or_default();  // :417
    Response::from_data(body).with_status_code(status)…             // :418-419
}
```
三个可核点：
1. ⭐⭐ **`code` 被原样传入、不改写** ⇒ ⇒ **B0256 的结论成立**（该码在任何站点恒为 404）
2. ⭐⭐⭐ **而更硬的一层是那个类型：`code: &'static str`**
   ⇒ ⇒ **码是编译期常量** ⇒ ⇒ **调用点无法传入运行时算出来的码**
   ⇒ ⇒ **「四个站点拼写一致」不只是纪律，而是**类型**保证的**
   ⇒ ⇒ 这是 P2 第五落点（码↔状态由一处保证）的**实现机制**
3. ⭐ 另两点：`ErrorDetail::new(code, message)` ⇒ **`ErrorDetail` 是跨错误面共用的载体**
   （与我在 B0015 核的 WS 侧 `ErrorDetail` 同一类型）⇒ **错误细节只有一个类型**；
   `serde_json::to_vec(…).unwrap_or_default()` ⇒ **序列化失败给空体、不 panic**
   ⇒ ⇒ 与本仓的 no-panic 取向一致（B0117 `catch_unwind` · B0138 `try_recv`）

## 未核实项
1. `ErrorDetail` / `ErrorResponse` 的**定义**未读（在哪个模块、字段是否只 `code`+`message`）
2. `dev_tools_section.dart` 余 ~1845 · `memory_panel.dart` 余 ~560 · `persona_panel.dart` 余 ~520 ·
   `message_bubble.dart` 余 ~480 · `chat_panel.dart` 余 ~480 · `live2d_stage.dart` 余 ~615 ·
   `error_banner.dart` 余 ~15 未读
3. `chat_session.dart` 的 `maxSessions = 50` 截断施加点未读
4. 前端 `.dart` 仍 181 未读（**机械值**）；Mod crates 42 未读；`mod-system` 余 8 文件未读
5. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
6. **iframe 重建后宿主是否主动发 `clear`**（B0251 留）—— 未核
7. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
8. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
