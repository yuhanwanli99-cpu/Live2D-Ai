# BATCH-0237 · ⭐ `personaStatusIsFailed`：**两个来源用了不同字面量，而「只认一个 ⇒ 失败提示永不出现」**

Phase 1 · 域覆盖 · 前端 `.dart`（46/218）—— `lib/settings/mods/persona_panel.dart` 的失败态判定

## 跑的命令（全部只读）
```
grep -nE "失败|坏卡|_error|_message|import.*失败|保留|原样" persona_panel.dart
sed -n '100,112p' persona_panel.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ **教科书级地拦住了一类我记过 9 条的缺陷**
```dart
/// 运行状态是不是「启动失败」。
///
/// **两个值都要认**：GET /api/v1/mods 序列化的是 `ModStatus::as_str()` 的
/// **failed**，而 `ModInfo.statusLabel` 认识的历史写法是 **error**。
/// **只认一个，失败提示就永远不会出现**——「**界面看起来没事**」的那类缺陷。   // :103-110
bool personaStatusIsFailed(String status) => status == 'failed' || status == 'error';
```
四个可核点：
1. ⭐ **两个来源用了不同的字面量**（API 序列 `failed` / `statusLabel` 的历史写法 `error`）⇒ ⇒ **只认一个就会漏**
2. ⭐ **漏掉的后果被点名**：「失败提示就永远不会出现 —— 「**界面看起来没事**」的**那类缺陷**」
   ⇒ ⇒ **他们把「缺陷类别」写了出来**，而不只是描述这一次的现象
3. ⭐⭐ **函数名就说清它决定什么**（`personaStatusIsFailed`）⇒ ⇒ 它是**纯判据**（可测、可复用）
   ⇒ ⇒ **不是散在 6 个调用点的内联比较**
4. 且是**公开**的（`bool` 非 `_bool`）⇒ ⇒ **测试目录能直接测它**（与 B0187 的测试面一致）

⇒ ⇒ **而这正是我在别处记成缺陷的那一类**：**同一状态的两个表示不一致**
（B0123 的 settings 视图字段对等性 · F-0060-01 的 `SettingsPatch` 静默忽略未知键）⇒
**这里他们找到了这个不一致，并让判据两个都认。**

### ⭐ 而它与 B0159 的 `ErrorKind::code()` 构成一组对照
| | 做法 | 适用 |
|---|---|---|
| **错误码**（B0159） | **契约层统一**：一个 `code` 串，两侧同源 | 新设计 |
| **状态字面量**（本批） | **判据层兼容**：`failed \|\| error` 两个都认 | **迁移留下的疤**，让它显形 |
⇒ ⇒ 两种都是对的，**取决于你是不是那个引入迁移的人**：
**引入方负责统一，消费方负责兼容**。

## 未核实项
1. `persona_panel.dart` 余 ~560 行未读（会话绑定卡片呈现 · 停用还原 · 坏卡失败处置的 UI 本体）
2. `message_bubble.dart` 余 ~490 · `chat_panel.dart` 余 540 · `error_banner.dart` 余 45 未读
3. `live2d_stage.dart` 余 ~645 · `director_observer_section.dart` 余 ~650 未读
4. 前端 `.dart` 仍 172 未读；Mod crates 19/61；`mod-system` 余 8 文件未读
5. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
6. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
7. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
