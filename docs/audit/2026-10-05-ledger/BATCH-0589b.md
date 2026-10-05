# BATCH-0589b 落盘（极简）· 四个调用点，enabled 正是折叠那个

## 编号
`BATCH-0589.md` 已存在（内容是「Q1/Q2 未在任何地方被回答」）⇒ 本批落 `BATCH-0589b.md`。

## 命令（只读）
```
ls AUDIT-REPO/BATCH-0589.md
grep -n "StagePointerInterceptor" shell/flutter/lib/app/app_shell.dart
```

## 四个调用点（逐字）
```
:32  /// 修法：给它们套一层 [StagePointerInterceptor]（原理见其头注）。

:532  child: StagePointerInterceptor(
:533    child: _settingsSurface(          <- 设置浮层（surface 宿主）

:722  child: StagePointerInterceptor(
:723    enabled: widget.wsStatus != WsStatus.connected,   <- 断线时才启用
:724    child: CollapsiblePanel(          <- **折叠那个**，与 enabled 同层

:753  child: StagePointerInterceptor(
:754    child: widget.stageCorner!,       <- 缩放角标

:843  second: StagePointerInterceptor(
:844    // 收起时把舞台那一块还给 iframe（否则 compa…
:845    // **永远拖不动模型**）。与 `CollapsiblePanel` …
```

## 三个可核点
1. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 上一批的因果链收口了一半**：`:722-724` 显示
   `StagePointerInterceptor(enabled: wsStatus != connected, child: CollapsiblePanel(…))`
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 这个 `enabled` 判的是「断线未连」，不是「折叠状态」**
   ⇒⇒ ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 与 `:75-78` 头注说的「垫层必须能跟着收起」不是同一个开关**
   ⇒⇒ ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ 折叠的收放在 `:843` 那个**第二个** `StagePointerInterceptor` 上
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 判据：一个类有 `enabled` 参数 ≠ 它的每个用法点都用了它。**
2. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ `:722` 的 `enabled` 判据是**断线**而不是折叠 ⇒ 有意思**：
   断线时舞台那一块**不需要**接收指针（反正没连）⇒ 垫层没必要 ⇒ 关闭
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 判据：`enabled` 的条件要与「垫层存在的理由」同向** ——
   垫层的理由是「这块区域要收指针」，那么**不需要收的时候**就该关；
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 这正是一条可核的自洽性检查**（本批新增）。**
3. **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ `:32` 的「修法：给它们套一层」把四个调用点归成一处纪律** ⇒⇒
   **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 判据：头注里点名的调用点，要能在代码里数出个数对上** ——
   这里是「它们」（复数）⇒⇒ 若只出现在一处，头注就该改成单数。
   ⇒⇒ **本批实测四处** ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 对得上**。

## 未核
`:843` 那个 `second:` 用的是哪个条件（折叠态？还是别的宿主）·
`stage_pointer_interceptor_web.dart` 改 DOM 的那几行 · `:532` 设置浮层为何不加 enabled
