# BATCH-0612b 落盘（极简）· `==` 写对了，而且**四条字段逐条论证过**

## 编号说明
`BATCH-0612.md` **已存在**（内容是 `--check` 用退出码落地）⇒ 本批改落 `BATCH-0612b.md`。

## 命令（只读）
```
ls AUDIT-REPO/BATCH-0612.md
grep -n "operator ==|hashCode|class BackgroundItem|sealed class" background_item.dart
sed -n '360,390p' background_item.dart
```

## 实测（`shell/flutter/lib/design/background_item.dart`）
```
:54  sealed class BackgroundItem {
:365 @override
:366 bool sameAs(BackgroundItem other) => other is BackgroundImage && other.id == id;
:368 /// 按 [id]（+ 逐图样式）判等，**不看 [dataUrl]**。
:370 /// 为什么不看 [dataUrl]：同一张图在「字节读回来之前」与…
:372 /// **同一项**，否则「刚导入」这一次 [DisplayPrefs] 比较会判…
:373 /// 连带把整个偏好重写一遍，而重写又要重新序列化所有…
:375 /// 为什么**要**看样式：样式是用户改出来的「这一项怎么…
:376 /// 「改了逐图铺法」被判成「什么都没变」⇒ 不落盘（静默失效…
:377 @override
:378 bool operator ==(Object other) => other is BackgroundImage &&
:379   other.id == id && other.opacity == opacity &&
:380   other.fit == fit && other.align == align;
:384 int get hashCode => Object.hash('image', id, opacity, fit, align);
:430-434  BackgroundPattern 同样重写了 == 与 hashCode（'pattern', id）
```

## 五个可核点
   `==` 与 `hashCode` **都重写了**（`BackgroundImage:378/384`、`BackgroundPattern:430/434`）
   ⇒⇒ ① 「按 `[id]`（+ 逐图样式）判等」
   ⇒⇒ ② 「**为什么不看 `[dataUrl]`**：同一张图在『字节读回来之前』与…
   ……否则『刚导入』这一次 `[DisplayPrefs]` 比较会判…**连带把整个偏好重写一遍**，
   而重写又要重新序列化所有…」
   ⇒⇒ ③ 「**为什么要看样式**：样式是用户改出来的『这一项怎么…』——
   **『改了逐图铺法』被判成『什么都没变』⇒ 不落盘（静默失效）**」
   —— 「在」的论证是「漏了会静默失效」，「不在」的论证是「算了会连带重写整个偏好」
   **不只选对一侧，还要把两侧的代价都写出来。**
   （都是 `'image', id, opacity, fit, align`）⇒⇒
   —— **不一致就是 bug**（值相等而 hash 不同 ⇒ 集合/Map 里会漏掉相等项）
   **跨子类型的值域必须靠首元素区分**。
   **`==` 不该同时承担「身份」与「值」两种语义**；
   要用两个方法，**并让名字带上这个区别**（`sameAs` vs `==`）
   `backgroundItemIdentity`（`:101-103`）**手写了前缀**
   `backgroundItemIdentity` 拼成**字符串**、`sameAs` 返回 **bool**，
   若调用方把身份串存进 prefs 而比较时用 `sameAs`，**两套判等可能漂移**
   **本批不立**（需核调用点，下一批）。
   `sealed` + 穷尽 `switch`（无 `default`）= 编译器帮你守住了「新增子类要改这里」**
   **`sealed` + 无 `default` 的 `switch` 是一对** ——
   只写 `sealed` 而 switch 里带 `default`，等于把编译器刚给的门禁自己拆了。
   `toString` 用来**排障**（要看到 `state`/`dataUrl` 长度），
   `==` 用来**决定要不要重写**（不能因 `state` 变而重写）⇒⇒
   不该「统一」** —— 我核之前还担心「`==` 漏了 `state`」。

## 未核
`sameAs` 与 `backgroundItemIdentity` 的调用点是否混用（第 3 点，P2 候选）·
`AppShell._settingsSurface` 注入点 · `main.dart`（1344）头注 ·
`_ModConfigTileState` 那 440 行
