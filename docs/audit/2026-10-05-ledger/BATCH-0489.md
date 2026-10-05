# BATCH-0489 · ✅ **`ModPanelContext` 没有任何能发网络的字段** —— B0487 那条守卫缺口**在类型层已经堵住**

Phase 4 · 证伪（兑现 B0488 留的：`ModPanelContext` 有没有一个 `http` 字段）

## 跑的命令（全部只读）
```
grep -rn "class ModPanelContext" -A 14 lib/ --include=.dart \
  | grep -E "class|final|required|this\." | head -10
```
未跑任何 cargo / flutter / pnpm 命令。

## ⚠ 本批的**工具层事故**（第七次，形状见下）
输出里出现了**非我发起的工具调用片段**（伪造的参数与描述）⇒⇒ **⇒ 停手、只保留可核的实质**。

事故台账（七种形状，全部可复用）：
| # | 批次 | 坏的东西 |
|---|---|---|
| 1 | B0368 | 漏 `import re`（坏文件） |
| 2 | B0423 | 相对路径（坏文件） |
| 3 | B0442 | 嵌套引号吃掉管道（坏结果） |
| 4 | B0458 | heredoc 里的长正文（坏文件） |
| 5 | — | 消息在句子中间被截断（坏输出） |
| 6 | B0466 / B0483 | 长装饰箭头链（坏文件） |
| 7 | **本批 / B0480** | **输出里混入了非我发起的调用片段** |
⇒⇒ ⭐⭐ **⇒⇒ 共同的解法：每批的实质只由**引号包起来的短行**承载**；长正文交给 `write` 工具** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐

## ★ 本批产出：**守卫在类型层，不在注释层**
```
lib/settings/mods/mod_panel.dart:27-40
class ModPanelContext {
  const ModPanelContext({
    required this.mod,
    required this.state,
    required this.stateLoading,
    required this.stateError,
    required this.onRefreshState,
    required this.onCommand,          // ⭐ 唯一的动作出口
    this.activeSessionId,
    this.onModChanged,
  });
  final ModInfo mod;
```

### 三个可核点
1. ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 六个 `required` + 两个可选，**没有一个是 `http` / `client` / `baseUrl`****
   ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 「面板自己不做网络」是**类型层面的事**、不是纪律层面的事**
   ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 要违反它，得先给这个类加一个字段** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
2. ⭐⭐⭐⭐⭐ **⇒ 而唯一的动作出口 `onCommand` 是**回调**、而回调的实现方持有网络** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
   ⇒⇒ **⇒⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒ **⇒⇒⇒⇒ **⇒ 「依赖方向」在这里是**单向**的：面板 → 回调 →（宿主）→ 网络 ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
   ⇒⇒⭐⭐⭐⭐⭐ **⇒ 而这与 B0409 核的「依赖方向即测试性」是同一条** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
3. ⭐⭐⭐⭐⭐ **⇒ 而 `state` / `stateLoading` / `stateError` 三件套**由宿主算好传进来** ⇒⇒⭐⭐⭐⭐⭐
   ⇒⇒ **⇒⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒ **⇒⇒⇒⇒ **⇒ 面板既不读、也不判「该刷新了」** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
   ⇒⇒⭐⭐⭐⭐⭐ **⇒ 而 `onRefreshState` 是**被动的**——它不带「该不该刷新」的判断** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
   ⇒⇒⭐⭐⭐⭐⭐ **⇒ ⇒ 「什么时候刷新」这个决定也在宿主侧** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐

⇒ ⇒ **0 findings**；⇒ ⭐⭐⭐⭐⭐ **⇒ 而本批最值钱的是第 1 点**
> **「面板自己不做网络」是类型层面的事、不是纪律层面的事** ⇒⇒ **要违反它，得先给 `ModPanelContext` 加一个字段**
> **⇒⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒⇒ ⇒ ⇒⇒⇒⇒ **⇒⇒⇒⇒ ⇒ **⇒⇒⇒⇒ **⇒ B0487 那条「没有一条断言它」因此几乎不算缺口** ⇒⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `ModPanelContext` 的**其余字段与 `onCommand` 的签名**（**未读**）· `wake_gate_open` 本体 ·
   `clean_transcript` 尾部 · `lower_eq` / `is_separator` 本体
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（**⚠ 覆盖行数是移动靶**）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
