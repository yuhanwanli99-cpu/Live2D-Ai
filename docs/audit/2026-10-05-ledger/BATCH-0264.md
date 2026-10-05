# BATCH-0264 · ⭐ **排除了一个真实失败模式**：「测试的纯函数 ≠ UI 调的那个」在此**不成立**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `memory_panel.dart` 的桶说明文案

## 跑的命令（全部只读）
```
grep -rn "memoryBucketNotice" lib/ test/
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ **一类失败模式被排除**
| 位置 | 内容 |
|---|---|
| `lib/settings/mods/memory_panel.dart:55` | **唯一**定义 `String memoryBucketNotice(String? activeSessionId)` |
| `lib/settings/mods/memory_panel.dart:577` | **唯一**生产调用点 `message: memoryBucketNotice(ctx.activeSessionId)` |
| `test/memory_panel_test.dart:181-187` | 测试调的**正是同一个符号**；且 `:187` 传 `'  s-1  '`（**带前后空格**）⇒ **顺带钉住了 trim** |

⇒ ⇒ **「一个被充分测试的纯函数，而 UI 实际调的是另一个」—— 这是我见过的一类真实缺陷**（测试与实现分叉，
测试全绿而界面是另一套文案）⇒ **此处不成立**：**一个定义、一个生产调用点、测试调它**
⇒ ⇒ ⇒ **B0246 核的那两句文案，就是用户看到的那两句**（不是「被测的那两句」）

### ⭐ 而这与 B0236 构成一组**两个都对的形态**
| 文案函数 | 形态 | 判定 |
|---|---|---|
| `memoryRecordText`（B0236） | **显示与删除确认共用** | ⭐ 正面：**确认框不可能显示出与列表不同的东西** |
| `memoryBucketNotice`（本批） | **单一调用点 + 测试调同一符号** | ⭐ 干净：**测试与实现不可能分叉** |
⇒ ⇒ **两种形态都合格** ⇒ ⇒ **「共用」不是唯一正确，「单一且被测」同样正确**（P25 的又一次体现：**看约束在哪一层保证**）

### 顺带一处**如实的命名观察**（非缺陷）
测试名说「null 说清全局桶降级；有会话说清只影响该会话」，而**测试体还钉了 trim**（`:187` 传带空格的 id）
⇒ ⇒ **名字比覆盖面窄** ⇒ 按我的既有判据（B0227 的「名字写明被否掉的方案」），这属**可选的精确化**，
⇒ **不记发现**（测试**多**覆盖了，不缺）。

## 未核实项
1. 面板余面：`dev_tools_section` ~1845 · `live2d_stage` ~600 · `memory_panel` ~545 ·
   `persona_panel` ~520 · `message_bubble` ~470 · `chat_panel` ~465 · `error_banner` ~15
2. `onReady` 是否间接清过 `presetStatus`（F-0263-01 的反证 (a)，**结论不依赖它**）
3. `ErrorResponse` 定义未读；`MutatingCheckError::as_message()` 未读
4. **两侧措辞同步无机制**（B0260 敞口）· `chat_session.dart` 的 `maxSessions=50` 施加点未读
5. 前端 `.dart` 仍 181 未读（**机械值**）；Mod crates 42 未读；`mod-system` 余 8 文件未读
6. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
7. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
8. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
