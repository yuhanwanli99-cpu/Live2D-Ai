# BATCH-0254 · 两个码**服务端单侧存在** ⇒ **我的对账样本仍是 1**（不把两类东西混着数）

Phase 1 · 域覆盖 · `models_routes` / `settings_routes` / `env_routes` 的错误码（扩展 B0253 的样本）

## 跑的命令（全部只读）
```
for c in model_not_found persist_failed; do grep -rn "\"$c\"" crates/ shell/flutter/lib/; done
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ 一个**关于我自己的计数**的修正
| 码 | 服务端出现处 | 前端散文引用 |
|---|---|---|
| `model_not_found` | `models_routes/handlers.rs:133 / :254 / :326 / :371`（4 处生产）· 回归 `:108 / :276` | **零** |
| `persist_failed` | `settings_routes/mod.rs:212 / :218 / :223` · `env_routes.rs:147` · `models_routes/handlers.rs:408` | **零** |

⇒ ⇒ **两者都只存在于服务端** ⇒ ⇒ **它们根本不是「散文 ↔ 码」的对** ⇒ ⇒
⇒ ⇒ **B0253 的对账样本**（前端**引用了**某个码、且与实现一致）**仍然是 1**，**没有增长**
⇒ ⇒ **我把它们单列，是因为它们验证的是另一个性质**，见 ②

### ② ⭐ 而 `persist_failed` 验证的是**「码的一侧收敛」**（正面模式 **P2** 的第四种落点）
**一个码字符串跨三个端点模块复用**：`settings_routes` · `env_routes` · `models_routes`
⇒ ⇒ **一次搜索命中三类失败**
⇒ ⇒ 代价：**「哪个端点」不在码里** ⇒ ⇒ 而 **B0198 已核 `dispatch.rs` 会把 `route` 记进日志**
⇒ ⇒ **两个维度（码 + route）合起来仍然可搜** ⇒ ⇒ **这是一个正确的分工，而不是信息丢失**
⇒ ⇒ P2 的落点至此有**四种**：一致性（B0215）· 安全属性（B0236）· 资源上界（B0240）·
**码的一侧收敛**（本批）

## 未核实项
1. `models_routes` 其余码（如 `:326` 附近的第三个 `model_not_found` 用在哪个端点）未逐一核
2. `dev_tools_section.dart` 余 ~1845 行未读（四分区本体 · 列表渲染 · 导入表单 · 错误呈现）
3. `memory_panel.dart` 余 ~560 · `persona_panel.dart` 余 ~520 · `message_bubble.dart` 余 ~480 ·
   `chat_panel.dart` 余 ~480 · `live2d_stage.dart` 余 ~615 · `error_banner.dart` 余 ~15 未读
4. `chat_session.dart` 的 `maxSessions = 50` 截断施加点未读
5. 前端 `.dart` 仍 181 未读（**机械值**）；Mod crates 42 未读；`mod-system` 余 8 文件未读
6. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
7. **iframe 重建后宿主是否主动发 `clear`**（B0251 留）—— 未核
8. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
9. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
