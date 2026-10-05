# BATCH-0192 · ⭐ **反证 (a) 也被推翻**（按钮从不禁用），但危害**再收窄一格**（失败响应不丢草稿）

Phase 1 · 域覆盖 · `shell_settings.dart:215-232`（F-0189-01 的最后一条反证）

## 跑的命令（全部只读）
```
sed -n '215,232p' lib/app/shell_settings.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**0 条新发现**；**F-0189-01 的两条反证全部结清**
### ① ⭐ 反证 (a) **被推翻**：按钮**从不禁用**
```dart
FilledButton.tonal(
  onPressed: () => unawaited(_settings.load()),      // :224  ← **永远非空**
  child: const Text('重试'),
),
```
⇒ **没有 `onPressed: _settings.loading ? null : …`** ⇒ 连点两次 ⇒ **两个重叠 `load()`**
⇒ 而**上下文让它更容易发生**：这个分支是**「读不到服务端设置」的界面**（:215）
⇒ ⇒ **用户恰恰是在「以为没反应」时连点「重试」** —— 这是最可能连点两次的动作。

### ② ⚠ 而危害**要再收窄一格**（我原记录不够准）
`load()` 的结构决定：**失败响应**（:269-272 的两个 catch）**只设 `_error`**，
而 **`_draft = SettingsDraft()` 在 `try` 内的成功路径**（:267）⇒
⇒ **一个失败的响应既不覆盖 `_remote`、也不重置 `_draft`**
⇒ ⇒ **「迟到响应丢弃未保存草稿」**只发生在**两个都成功**的 `load()` 乱序时；
   **「面板显示较旧的值」**则发生在任何乱序（含一个失败）时
⇒ ⇒ **主危害的触发条件**：**两个成功的 load 乱序**（如 `:108` 的刷新与 `:224` 的重试交叉）
⇒ **未核**：`:108` 那处（`await _settings.load()`）的**触发方式**（是否也是可连点的按钮）

### ③ 两条反证的最终状态
| 反证 | 状态 | 依据 |
|---|---|---|
| (a) 按钮在 `_loading` 期间被禁用 | ❌ **推翻** | `:224` `onPressed` 永远非空 |
| (b) `ApiClient` 内部有串行化 | ❌ **推翻**（B0191） | `api_client.dart:177-183` 无队列/序号/in-flight 判据 |
| (c) 连按两次是常见用户行为 | ⚠ **仍无法从仓库证明** | ⇒ 维持 P2，不升 P1 |

## 未核实项
1. ⭐ `shell_settings.dart:108` 那处 `await _settings.load()` 的**触发方式**（是否也是可连点按钮）
   ⇒ 决定「两个成功 load 乱序」这条主路径的**现实性**
2. `_guard()` 实现未读（超时/重试/去重）
3. `settings_controller.dart:120-222` / `:296-409` 其余未读；`:327` 那处置 `_draft` 的上下文未读
4. 新露出的 208 条自我设防名只看了 11 条；`dev_tools_section.dart`(1874) / `tokens.dart`(928) 未读
5. Mod crates 逐文件（约 53/61）· `shared/` · 根 `tests/`(3 py) · `verification/`
6. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
7. `join_endpoint` 回归断言体 · `mod_count_is_five` 断言体 · `_finishTurn` 是否幂等 ·
   `secrets.rs` 断言体 · B0120「chmod toml」未核
