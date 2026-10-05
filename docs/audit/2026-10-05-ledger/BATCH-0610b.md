# BATCH-0610b 落盘（极简）· ⭐ 上一批那个推论是错的：扇入**搬走了**，但**换了通道**

## 编号说明
`BATCH-0610.md` **已存在**（内容是 `dist/index.html` 构建判断）⇒ 本批改落 `BATCH-0610b.md`。

## 命令（只读）
```
ls AUDIT-REPO/BATCH-0610.md                                  -> 存在
grep -n "onRemoveBackground|onReorderBackground|BackgroundRuntimeScope" \
     appearance_section.dart                                   -> 9 处
grep -c "onRemoveBackground" appearance_background.dart          -> 0   ← 关键
sed -n '215,230p' appearance_section.dart
grep -n "BackgroundRuntimeScope|maybeOf" appearance_background.dart
```

## 关键结果：**part 里 0 处引用那三个背景回调**
```
宿主 appearance_section.dart:
:78-80  this.onRemoveBackground, / this.onRemoveBackgrounds, / this.onReorderBackground,
:145    final ValueChanged<int>? onRemoveBackground;
:148    final ValueChanged<List<int>>? onRemoveBackgrounds;
:151    final void Function(int oldIndex, int newIndex)? onReorderBackground;
:221    onRemoveItem: onRemoveBackground,
:224    onReorderItem: onReorderBackground,
:225    onRemoveMany: onRemoveBackgrounds,

part appearance_background.dart:
        grep -c "onRemoveBackground"  ->  0
```

## 四个可核点
   我写「拆走 background 域后，它仍要接 background 域的 4 个回调 ⇒ 搬不走」。
   **实测：part 里对这批回调的引用数是 0。**
   我搜的是「回调名」，而 part 拿到它们**不是通过回调名，是通过 `BuildContext`**。
   ```
   appearance_background.dart:60  class BackgroundRuntimeScope extends InheritedWidget
   :61    const BackgroundRuntimeScope({…
   :79    static BackgroundRuntimeScope? maybeOf(BuildContext context) =>
   :40    头注：「运行时上下文与提交滑杆（BackgroundRuntimeScope :60 / …」
   ```
   **「这个 widget 的数据从哪来」有两条路 —— 构造器参数，或 `InheritedWidget`。
   搜回调名只能看到第一条。**
   **B0431 是「值在别处」，这里是「值在同一个 widget 树上、但不走参数」**
   「**有回调名、无参数名**」** —— 只搜参数名会得出「这段没接」的错结论。
   `onPickStageImage` / `onClearStageImage` / `onRemoveItem` / `onAddPattern` /
   `onReorderItem` / `onRemoveMany` / `onPreviewItem`
   **接线块里同一个 widget 会同时接住「宿主域」与「part 域」的参数**；
   本例它被 `_BackgroundBlock` 消费 ⇒ **part 那一侧的参数**
   （`onAddPattern` / `onPreviewBackground` 等）**也在这个块里** ⇒⇒
   **part 并没有减少宿主的扇入，只是把「扇入点」从构造函数移到了 build 里的一个接线块。**
   - 我上一批说「709 行 + 30 参数 ⇒ 值得继续拆」
   - **现在看：参数多是因为「三块域共用一个 widget 入口」，
     而 part 拆分解决的是「实现位置」不是「扇入位置」**
   **⇒ 判据：拆分减少的是「文件行数」，不减少「入口扇入」；
   所以「行数还是超限」不能作为「继续拆」的充分理由。**
   ⇒⇒ 头注 `:24` 写的「**不必强行压到 ≤500**」因此是**有依据的**，不只是「下一轮再说」。**

## 未核
`BackgroundRuntimeScope` 里放了哪些字段（`maybeOf` 的返回类型链）·
`_BackgroundBlock` 的构造器参数（`:215-228` 那个接线块的另一侧）·
`main.dart`（1344）头注 · `_ModConfigTileState` 那 440 行
