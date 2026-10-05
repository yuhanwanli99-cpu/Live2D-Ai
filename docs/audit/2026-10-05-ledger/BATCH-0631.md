# BATCH-0631 落盘 · severity 的三色确实在「一处」

## 跑的命令（只读）
```
grep -n "severity|border|surface" lib/../crates/live2d-ai-mod-system/src/../../../../shell/flutter/lib/ui/inline_notice.dart
```
（实跑：sed -n '78,96p' shell/flutter/lib/ui/inline_notice.dart）

## 逐字（shell/flutter/lib/ui/inline_notice.dart:85-95）
```
:85  final (Color surface, Color border, Color ink) = switch (severity) {
:86    NoticeSeverity.danger  => (palette.dangerSurface, palette.dangerBorder, palette.danger),
:90    NoticeSeverity.warning => (palette.warning.withValues(alpha: 0.14),
                                   palette.warning.withValues(alpha: 0.45),
                                   palette.warning),
:95    NoticeSeverity.info    => (…
```

## 四个可核点
1. 「决定底色/描边/图标三件事，**一处定义**」**字面成立**：
   `:85` 一个 `switch` 同时解出 `(surface, border, ink)` 三个值
   ⇒ 与 B0543「判据在纯函数、采集在宿主」同族、而这里**判据与取值在同一处**
2. 具名元组解构 `(Color surface, Color border, Color ink)`
   ⇒ 三个值同名出现两次（声明处 + 每臂的返回位置）
   ⇒ 与 B0440 核的「5 行 3 臂 4 值 + 一个占位」那个形状**同族**（Rust 侧有 `_dark` 占位、
   **Dart 这一侧三个值全用上了、没有占位**）
3. `danger` 与 `warning` 两条臂**取法不同**：
   `danger` 用**三个命名 token**（`dangerSurface` / `dangerBorder` / `danger`）、
   `warning` 用**一个 token + 两个 alpha**
   ⇒⇒ **⇒ 与 B0627 核的那三处旧写法对照：三档都走同一个 `switch`、只是 token 不同
   ⇒⇒ ⇒ 「三套写法」确实塌成了一处**
4. 而 `ink` 给的是**纯 `palette.danger`（不透明）**、`surface`/`border` 带 alpha
   ⇒⇒ **三档里「字的对比度」与「底的通透度」是分开调的**
   ⇒⇒ 与 B0491「两个不同原因不该共用一处」**互补**（这里是**同一处、不同调法**）

## 未核
`InlineNotice` 的 `onDismiss` / `dense` 两条参数 · 其余 mod-system 两个文件本体 ·
shared/ 其余 8 个 json · setup_linux.sh / ignition-precheck.sh /
verify_core_chain.py / check_public_secrets.py / font_subset_ranges.py 本体 ·
面板三份余下部分
