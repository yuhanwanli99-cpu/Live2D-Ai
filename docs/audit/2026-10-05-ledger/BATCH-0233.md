# BATCH-0233 · `director_observer_section.dart`：**双重 dev_mode 门（第二次为单点测试）+ 假数据口不发网络**

Phase 1 · 域覆盖 · 前端 `.dart`（43/218）—— `lib/settings/sections/director_observer_section.dart`(668) 的头注

## 跑的命令（全部只读）
```
wc -l lib/settings/sections/director_observer_section.dart ; sed -n '1,16p' director_observer_section.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；**第 6 处「把可测性设计进去并写下来」**
> 「挂载点：`DeveloperSection.build` 的 `if (devMode)` 块内（`DebugPanels` 之后）。
> **`dev_mode=false` 时整块不在语义树**；本组件**自己也短路一次，测试可单点**。」   // :2-4

三个可核点：
1. ⭐ **双重门控，且**第二次的理由写明**：「自己也短路一次，**测试可单点**」
   ⇒ ⇒ 与 B0226/B0228 的 `import` 边界**同形**（都是「**为了让测试能单点跑**」而写的代码）
2. ⭐ **「数据怎么进来（**不改 `shell_settings.dart` 的硬约束**）」** ⇒ 它**点名了自己正在绕开的约束**，
   再给机制：单例 [`DirectorObserverFeed`]（`ChangeNotifier`），`main.dart` **只往里 push**，
   组件用 `ListenableBuilder` 消费
3. ⭐ **「构造函数仍保留可注入的假数据口（`loadState` / `loadLogLines`），widget 测试注入 fake，
   **不发真网络**」** ⇒ ⇒ **生产路与测试路在构造函数处分开** ⇒ 测试**永不碰网络**

### ⚠ 而我也要记下这个方案的**代价**（它写在头注里，不是藏着的）
那个**单例 feed** 意味着 `main.dart` 持有一个**全局可变**句柄
⇒ ⇒ 这是「**不改 `shell_settings.dart`**」这个**真实约束**的**代价**
⇒ ⇒ **代价被写下来了**（不是被隐藏）⇒ ⇒ 这与「有理由的重复」（B0152/B0221）是**同一类自觉取舍**

### 顺带：`dev_mode` 门控 ⇒ **非开发者看不到它**，且**头注说了为什么**
⇒ ⇒ 又一处 **P1 的 UI 版**（「否则普通用户会看到一个自己用不了的面板」）

## 未核实项
1. `director_observer_section.dart` 余 ~650 行未读（四栏本体、状态读取、错误呈现）
2. 三个面板余面（`persona_panel` ~560 · `message_bubble` ~500 · `memory_panel` ~600）+ `chat_panel` 540 + `error_banner` 45 未读
3. `live2d_stage.dart` 余 ~645 行未读
4. 前端 `.dart` 仍 175 未读；Mod crates 19/61；`mod-system` 余 8 文件未读
5. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
6. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
7. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
