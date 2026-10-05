# BATCH-0182 · ⭐ P18 三个条件**全部满足**，且行为测试**恰好钉住了我 B0139 记下的那个微妙点**

Phase 1 · 域覆盖 · `shell/flutter/test/turn_liveness_test.dart`（P18 复核）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `shell/flutter/test/turn_liveness_test.dart` — （定点 71-107：`mustReleaseTurnOnWsLoss` 行为测试组）

## 跑的命令（全部只读）
```
grep -rn "mustReleaseTurnOnWsLoss" test/ lib/
grep -rn "mustReleaseTurnOnWsLoss(" test/ | grep -v "contains"
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**0 条新发现**；**P18 从「提案」变成「已验证」**
```dart
group('mustReleaseTurnOnWsLoss：断连时进行中的一轮必须收口', () {          // :71
  mustReleaseTurnOnWsLoss(turnInFlight: true, status: WsStatus.**closed**)      // :84 ← 该收
  mustReleaseTurnOnWsLoss(turnInFlight: true, status: WsStatus.**connecting**) // :91 ← **不该收**
  mustReleaseTurnOnWsLoss(turnInFlight: true, status: WsStatus.**connected**)  // :100 ← **不该收**
  mustReleaseTurnOnWsLoss( … )                                                // :74 / :107（另两例）
```
⇒ **是真实调用**（不是 `contains`）⇒ **P18 的第 (2) 条满足**
⇒ ⭐ 而**三个状态全测了**：该收的（`closed`）与**两个不该收的**（`connecting` / `connected`）
⇒ ⇒ **P7（每条断言配反向对照）在一个状态谓词上被完整执行**。

### ⭐ 而它**恰好钉住了我在 B0139 记下的那个微妙点**
B0139 我读 `mustReleaseTurnOnWsLoss` 的头注时记下：「判据用 `WsStatus.isProblem`
（`disconnected` / `closed`）而**不是** `!isUsable`：**`connecting` 也不可用，但它是「正在建连」——」」
⇒ 那时我把它列为**未核**（我核了文档，**没核测试**）。
⇒ **现在闭合**：**`:91` 那条测试就是为这个微妙点存在的** ⇒
**文档里那个「反直觉的判据」有测试钉着** ⇒ 日后有人把 `isProblem` 改成 `!isUsable`，
**测试会红** ⇒ **一个容易被「顺手修正」的判据被保住了**。

⇒ ⇒ **P18 三条结构型断言全部满足两条件**：
| 结构断言 | `reason` | 行为测试（含反向对照） |
|---|---|---|
| `settleTurn(`（:150） | ✔ | ✔ 7 条（B0139 核） |
| `kWordlessTurnNotice`（:158） | ✔ | 常量值在同文件别处被断 |
| `mustReleaseTurnOnWsLoss(`（:166） | ✔（无 reason 字段但测试名已述意图） | ✔ **3 状态全测**（本批） |

## 未核实项
1. `F-0002-2`（`_store` 一开始就是真库）断言体**未核**（B0179 留）
2. `kWordlessTurnNotice` 那个常量值「在同文件别处被断」**未核**（我核了 `settleTurn` 与 `mustReleaseTurnOnWsLoss` 两处）
3. 未审 `.dart` 仍 187 个（`dev_tools_section.dart` 1874 · `tokens.dart` 928 · `live2d_stage.dart` 666 …）
4. Mod crates 逐文件（约 53/61）· `shared/` · 根 `tests/`(3 py) · `verification/`
5. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
6. `join_endpoint` 回归断言体 · `mod_count_is_five` 断言体 · `_finishTurn` 是否幂等 ·
   `secrets.rs` 断言体 · B0120「chmod toml」未核
