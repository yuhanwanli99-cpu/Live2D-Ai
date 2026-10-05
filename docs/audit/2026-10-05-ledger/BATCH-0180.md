# BATCH-0180 · ⭐ 源码扫描型测试：**头注把「为什么降级」与「覆盖不到的部分归谁」都写了**

Phase 1 · 域覆盖 · `shell/flutter/test/action_scales_wiring_test.dart`（结构型测试核验）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `shell/flutter/test/action_scales_wiring_test.dart` — 737（定点 **16-21 文件头注** + 482-488 断言体）

## 跑的命令（全部只读）
```
grep -rn "源码扫描" -A 18 test/action_scales_wiring_test.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**0 条新发现**；⭐ 立**正面模式 P17**
### ① 头注把降级的**技术原因**写成**三条可核事实**（:16-21）
> 「`main.dart` 经 `app/browser_io.dart` 依赖 `package:web`，在 `flutter test`（Dart VM）里**加载不了**；
> `buildLive2DHost` 又走 `_stub.dart`（**不回调 `onTransport`，没有 bridge**）。
> 所以『构造点真的传了 actionScales』**只能**按 `wiring_test.dart` 的**先例**扫源码钉住，
> **发帧那条路由真机验收**。」
⇒ 三条原因**都不是「懒得测」**：① VM 加载不了 `package:web` ② stub 不回调、没有 bridge ③ **有先例**
⇒ 而断言本身是**字符串包含**（`settings.contains('pinnedScales: _actionScalesSyncer.pinned')`）
⇒ **它当然比行为测试弱**（字符串可能出现在注释或另一处调用点）——**但它能失败** ⇒
**不属模式 H 的「结构上无法失败」**，**不记发现**。

### ② ⭐ 而真正让它合格的是**头注最后那句**
「**发帧那条路由真机验收**」⇒ **扫源码覆盖不到的那一半，被显式指给了另一个验证手段**（真机）
⇒ ⭐ **正面模式 P17（新）**：**一道测试方法被降级时，需要两样东西才合格 ——
① 降级的技术原因（可核，不是「懒得测」）② 降级覆盖不到的部分由谁负责。**
⇒ 缺 ① ⇒ 是**偷懒**；缺 ② ⇒ 是**半个洞**；两样都有 ⇒ 是**有意识的取舍**。

### ③ ⭐ 而这条降级的**根因在别处**，值得记：它是**红线 K 的下游后果**
`app/browser_io.dart` 这层间接是为了**离线优先**（红线 K：隔离 `package:web`、不依赖 CDN）
⇒ **正因为**它隔离了 `package:web`，`main.dart` 才在 **Dart VM 里加载不了**
⇒ **一道红线的要求，直接导致了一条测试只能扫源码。**
⇒ ⇒ 而他们的应对**不是忽略**，是**降级 + 写明原因 + 指派缺口**。
⇒ ⭐ 这给出一条跨层观察：**一处好的设计会制造新的约束**；
**判断设计好不好，要看它制造出来的约束有没有被诚实地处理**，而不只看它本身。
