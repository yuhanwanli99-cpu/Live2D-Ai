# BATCH-0285 · ⭐ 「忽略未知」的**第四例、且理由写明**；而它与「敏感 ⇒ 移除」**并存**（转前端）

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `dev_tools_section.dart` 的 **Mod 状态视图层**

## 跑的命令（全部只读）
```
grep -nE "obscureText|controller|env|key" shell/flutter/lib/settings/sections/dev_tools_section.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ **「忽略未知」的第四例，而它是跨语言的约定**
```dart
// dev_tools_section.dart:338-340
/// 但通用渲染仍认识这些 key）。
/// **未知 key 不隐藏**（回落原始 key）——未来 Mod / 新字段**不能因为前端不认**…
```
⇒ ⇒ **未知 ⇒ 显示原始 key**，**理由是向前兼容**（未来 Mod / 新字段不能因前端不认识就隐形）
⇒ ⇒ ⭐ 这是本轮**第四例**「忽略未知」（`main.rs:505` · `:508` · `:136` · **本例**）
⇒ ⇒ ⭐⭐ **而它跨语言**：三例在 Rust（wasm `main.rs`）· **一例在 Dart** ⇒ ⇒ **更强地说明这是「约定」而非某处的选择**

### ⇒ 而它与「敏感 ⇒ 移除」**并存且不冲突**（接 B0284 的「两个近义动作被分开」）
| 情形 | 做法 | 出处 |
|---|---|---|
| **未知**字段 | **显示**（回落原始 key） | 本例 `:338-340` |
| **已知但敏感** | **移除**（`obj.remove(key)`） | `mods_routes`（B0066 已核） |
⇒ ⇒ **一个 UI 里两条规则并存、各说各的** ⇒ ⇒ **理由清晰**：
**「脱敏」针对「已知敏感」，「透传」针对「尚未知道」** ⇒ ⇒ **后者保证扩展不会被前端悄悄吃掉**
⇒ ⇒ 而这与 **B0123**（视图字段对等性）**同处一区** ⇒ ⇒ 三条同区结论**互相不打架**

## 未核实项
1. `dev_tools_section.dart` 余约 1845 行未读（**含密钥写入面** —— 本批的 grep 命中的全是 **Mod 状态视图**函数，
   **没有** `obscureText` / env 写入 ⇒ **那部分在别的文件**（`env_api.dart` 已见 B0255；写入面待定位））
2. `apply_bridge_effects` 的**调用顺序**未核（B0279 留）· `stage-bg.dataUrl` 的发送方未核（B0277 留）
3. `main.rs` 余约 930 行未读
4. 真实动作计划的 token 长度（F-0020-01 永久敞口）· `shell/flutter` 是否真可能出现密钥
5. 那四个「未知 id」在文档里有没有被点名（B0270 敞口）· `refresh_from_disk` 敞口（B0266）
6. 能否点名会在持锁期间 panic 的 Mod（B0268 敞口）
7. 面板余面：`live2d_stage` ~600 · `memory_panel` ~545 · `persona_panel` ~520 ·
   `message_bubble` ~470 · `chat_panel` ~465 · `error_banner` ~15
8. 其余 6 处 `mod_count_is_*` 逐条对账未核 · `ErrorResponse` 定义未读
9. `onReady` 是否间接清过 `presetStatus`（F-0263-01 反证 (a)，**结论不依赖它**）
10. **两侧措辞同步无机制**（B0260 敞口）· `chat_session.dart` 的 `maxSessions=50` 施加点未读
11. Mod crates 42 未读；`mod-system` 余 8 文件未读
12. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
13. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
14. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
