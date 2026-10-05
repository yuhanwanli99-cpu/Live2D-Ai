# BATCH-0614b 落盘（极简）· P6 描述的形状仍在：`GlobalKey<Live2DStageState>` + `setMouth`

## 编号
`BATCH-0614.md` 已存在（内容是「那条路径存在；而三个形近…」）⇒ 本批落 `BATCH-0614b.md`。

## 命令（只读）
```
ls AUDIT-REPO/BATCH-0614.md
grep -rln "GlobalKey" lib/live2d/ lib/app/
grep -n "GlobalKey|postMessage" lib/live2d/live2d_stage.dart
```

## 逐字
```
:179 /// 公开 State：宿主用 `GlobalKey<Live2DStageState>` 调 [setMouth] / …
:536 /// 宿主只需要 `GlobalKey<Live2DStageState>.currentState?.retry()`，
```
⇒⇒ `GlobalKey` 命中两个文件：`lib/live2d/live2d_stage.dart`、`lib/app/app_shell.dart`

## 四个可核点
1. **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ P6 描述的**形状仍然成立**：`GlobalKey<Live2DStageState>` +
   `setMouth` 都在（`live2d_stage.dart:179`）
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ 判据：验证一条原则是否还成立，要核**它描述的机制**，
   而不是核它引用过的行号** ⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 这是 B0613c 第 ② 点那个「行号腐烂」的**正确查法**：
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 行号会漂、机制不会**
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 修正方向**：上一批记「P3 候选（行号漂移）」，
   本批补上**它不影响原则本身**这一面 ⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ 判据（升级）：
   **一个陈旧引用要让两件事分开评——「它指的东西还在吗」与「它指的是那个东西吗」**。
2. **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ 而 `:536` 用的是 `currentState?.retry()`** ⇒⇒
   **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ 判据：`GlobalKey` 提供的 `currentState` 是个**可空**能力** ——
   宿主必须处理「舞台还没挂上」的情况 ⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 这一点与 P6 的意图一致**：口型驱动**不通过 Widget 树**，
   走的是「一个能被宿主直接拿到的 State 引用」⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 而它**可空**这件事，正是 P4「静默失效最危险」
   要防的那类**：舞台没挂上时 `currentState == null` ⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ 判据：**当一条原则依赖一个可空能力时，
   要看「为 null 写了什么」——「什么都不做」就是静默失效。**
3. **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ `postMessage` 在 `live2d_stage.dart` 里零命中** ⇒⇒
   ⇒⇒ **判据未完成**：P6 写的三段链是 `GlobalKey → Bridge → postMessage`，
   本批只核了第一段 ⇒⇒ `Bridge` 与 `postMessage` 应在别的文件（渲染面侧）
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ ⇒ 未核**（下一批核 `postMessage` 在哪）。
4. **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ 而 `GlobalKey` 只在**两个**文件里出现**
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ 判据：本仓对 `GlobalKey` 的用法是**收敛的** ——
   绕过 Widget 树的口型通道只有一条，其余 Widget 树是正常的
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 与 B0613c 第 ④ 点合看**：
   P6（spec）→ AGENTS 红线 → 代码里只有两处 `GlobalKey`
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ 是一条**三跳都核到了**的链**，
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ 判据（本批最值钱）：链式治理的证据强度
   随「核到的跳数」递增** —— 三跳齐 ⇒ 可以当事实用（与 B0611c 三个案例对照）。

## 未核
`Bridge` / `postMessage` 现在在哪（P6 三段链的第二、三段）·
`currentState == null` 时调用方写了什么 · app_shell.dart 那处 GlobalKey 的用途
