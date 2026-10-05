# BATCH-0253 · ⭐ 对账**通过**：`409` + `model_active` 在**四处**同拼写，且有回归钉住（0 条新发现）

Phase 1 · 域覆盖 · `models_routes`（Rust 侧错误码定义 —— B0252 留的对账）

## 跑的命令（全部只读）
```
grep -rn "model_active" crates/ shell/flutter/lib/
sed -n '370,380p' crates/.../models_routes/handlers.rs
sed -n '150,160p' shell/flutter/lib/api/models_api.dart
sed -n '403,405p' crates/.../models_routes/handlers.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ **对账通过**（B0252 唯一的未核项关闭）
| 层 | 位置 | 内容 |
|---|---|---|
| **服务端 · 状态** | `handlers.rs:403-404` | `conflict_response(code, message) => json_error_response(**StatusCode(409)**, code, message)` ✔ |
| **服务端 · 码** | `handlers.rs:375` | `"**model_active**"` ✔ |
| **服务端 · 文案** | `handlers.rs:376` | 「**激活中的模型不可删除；请先 activate 另一个模型**」⇒ ⭐ **可执行的下一步** |
| **前端 API 层** | `models_api.dart:153` | 「激活中的模型会 **409 `model_active`**」✔ |
| **前端 UI** | `dev_tools_section.dart:254` | 「会被服务端拒绝（**409 model_active**）」✔ |
| **回归** | `tests_models_handlers.rs:132/136` | 断言 body 含该码 ✔ |

⇒ ⇒ **四处同拼写 + 回归钉住** ⇒ ⇒ **「用户拿界面上的码去日志里搜」这条契约，在这一处完整成立**
⇒ ⇒ 而**两端都给了可执行的下一步**：UI 散文**在点之前**说，服务端文案**在拒之后**说
（服务端：「**请先 activate 另一个模型**」）
⇒ ⇒ 这是「**提前说清 / 不让人白点**」家族里**唯一一个两侧都给路**的样本

### ⭐ 顺带一条方法论确认
我 B0252 明确记「**只核了前端散文，服务端码未读**」，并把它列为第一件事
⇒ ⇒ **本批核完，答案是「对得上」** ⇒ ⇒ **这类「散文说的码」需要逐处对账**，
⇒ 而**它对得上的比例目前是 1/1** ⇒ ⇒ 但**样本还太少**，**不能据此说「这类都靠得住」**
⇒ **如实标注为 1 个样本，不外推。**

## 未核实项
1. `models_routes` 其它错误码（`model_not_found` / `persist_failed` 等）**是否也被前端散文引用** —— 未核
2. `dev_tools_section.dart` 余 ~1845 行未读（四分区本体 · 列表渲染 · 导入表单 · 错误呈现）
3. `memory_panel.dart` 余 ~560 · `persona_panel.dart` 余 ~520 · `message_bubble.dart` 余 ~480 ·
   `chat_panel.dart` 余 ~480 · `live2d_stage.dart` 余 ~615 · `error_banner.dart` 余 ~15 未读
4. `chat_session.dart` 的 `maxSessions = 50` 截断施加点未读
5. 前端 `.dart` 仍 181 未读（**机械值**）；Mod crates 42 未读；`mod-system` 余 8 文件未读
6. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
7. **iframe 重建后宿主是否主动发 `clear`**（B0251 留）—— 未核
8. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
9. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
