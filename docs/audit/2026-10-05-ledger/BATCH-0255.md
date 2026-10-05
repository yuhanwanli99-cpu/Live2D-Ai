# BATCH-0255 · ⭐ **F-0255-01（P3）**：两个**前端自造**的码，让「拿码搜日志」**必然落空**；对账样本 1 → 12

Phase 1 · 域覆盖 · 全 Dart 侧错误码 × Rust 侧字符串（**机械对账**）

## 跑的命令（全部只读）
```
python3 - <<'PY'   # 抽 Dart 里像错误码的 token → 逐个回 Rust 查同拼写字符串
PY
grep -rn "turn_failed_no_detail|network_error" shell/flutter/lib/
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**F-0255-01（P3）**；**对账样本 1 → 12**
### ① 机械对账：18 个 token，**12 个对得上**，6 个查不到
4 个**不是错误码**：`active_id` / `prev_active_id`（**JSON 字段名**）· `error_banner`（**文件名**）·
`invalid_use_of_protected_member`（**Dart 分析器诊断码**）⇒ **我的关键词过滤器有假阳性**（如实记）
**真正对不上的只有两个**，且**都是前端自造**：
```dart
throw ApiException('network_error', '无法连接后端：$e');       // api_client:233（另有 5 处同形）
_errorCode ??= 'turn_failed_no_detail';                        // ui_state_tracker:169
const String kTurnFailedWithoutDetailCode = 'turn_failed_no_de…'; // turn_liveness:190
```
⇒ **`grep -rn '"network_error"' crates/` 零命中**；`turn_failed_no_detail` 同样零命中
⇒ ⇒ **不是同码不同源，是纯前端码**

### ② ⭐ 而这打的是契约覆盖上的一个洞
AGENTS.md 明文：「**用户要拿界面上的码去日志里搜**」
- `network_error` 由 **6 个** API 包装器在**请求根本没到服务端**时抛出 ⇒ **按构造，服务端没有这一行**
- `turn_failed_no_detail` 是「本轮失败但**没带回任何码**」的**占位码** ⇒ **同样没有这一行**
⇒ ⇒ 用户把这两个码抄去搜日志，**保证搜不到** ⇒ **不是码写错，而是事件没到服务端**
⇒ **定 P3**：无功能影响（B0227 核过 `network_error` → 「重试」，对 UI 分支仍有用）；
**受影响的只有「搜日志」这一步**，而它**在排障最需要时白跑一次**

### ③ 建议（二选一，(a) 更便宜也更诚实）
- **(a) 把这条边界写下来**（与本仓既有做法同形：B0222 点名**不存在的 API**、
  B0244 点名「**不改 `shell_settings.dart` 的硬约束**」）：
  在 `error_banner.dart` 或 AGENTS 的错误码契约处写明
  「`network_error` 与 `turn_failed_no_detail` 是**前端自造**的码，
  **服务端日志里必然没有对应行** —— 这不是丢日志，是事件没到服务端」
- (b) 或让**这一行日志存在**（前端本地用同名 code 记一条）⇒ 则搜索能命中
⇒ ⭐ 顺带一条**正面**：这两个码在**六处** API 包装器里**拼写一致**
⇒ ⇒ **前端这一侧是整齐的**（P2 第五落点：**码的一侧收敛**）；**缺的只是它与服务端的边界说明**

### ④ ⚠ 而本批还纠正了**我自己计数脚本的一个歧义**
脚本同时打印 `len(cur)-len(wd)` 与**分级 Counter 之和**，**两者曾差 1** ⇒ 我此前引用的是**前者**
⇒ ⇒ **口径定为「分级 Counter 之和」**（可加、可核）⇒ **与 CONSOLIDATION-18 的教训同源**

## 未核实项
1. `models_routes` 其余码（`:326` 附近的第三个 `model_not_found`）用在哪个端点，未逐一核
2. `dev_tools_section.dart` 余 ~1845 · `memory_panel.dart` 余 ~560 · `persona_panel.dart` 余 ~520 ·
   `message_bubble.dart` 余 ~480 · `chat_panel.dart` 余 ~480 · `live2d_stage.dart` 余 ~615 ·
   `error_banner.dart` 余 ~15 未读
3. `chat_session.dart` 的 `maxSessions = 50` 截断施加点未读
4. 前端 `.dart` 仍 181 未读（**机械值**）；Mod crates 42 未读；`mod-system` 余 8 文件未读
5. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
6. **iframe 重建后宿主是否主动发 `clear`**（B0251 留）—— 未核
7. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
8. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
