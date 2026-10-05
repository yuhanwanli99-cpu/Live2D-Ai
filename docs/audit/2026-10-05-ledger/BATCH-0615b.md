# BATCH-0615b 落盘（极简）· `postMessage` 四处：三段链闭合，但职责分成两个 Dart 文件

## 编号
`BATCH-0615.md` 已存在 ⇒ 本批落 `BATCH-0615b.md`。

## 命令（只读）
```
ls AUDIT-REPO/BATCH-0615.md
grep -rln "postMessage" shell/flutter/lib/ crates/l2d-wasm-demo/src/
```

## 四处命中
```
shell/flutter/lib/live2d/live2d_transport.dart
shell/flutter/lib/live2d/live2d_host_web.dart
shell/flutter/lib/ui/stage_host.dart
crates/l2d-wasm-demo/src/web/surface/input.rs
```
⇒⇒⇒⇒ **三处 Dart + 一处 Rust** ⇒⇒⇒⇒ **P6 写的「Bridge」在 Dart 侧（`live2d_transport`），
接收方在 Rust 侧（`input.rs`）**

## 三个可核点
1. **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ P6 的三段链 `GlobalKey → Bridge → postMessage` **三段全部存在**：
   - 第一段 `GlobalKey<Live2DStageState>`（B0614b 核，`live2d_stage.dart:179`）
   - 第二、三段 `live2d_transport.dart`（Dart 发）+ `input.rs`（Rust 收）
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 与 B0614b 第 ④ 点合起来 ⇒ 三跳全核到**
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ 「30Hz 不进 Widget 树」这条红线**可当事实用**
2. **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ 而 `live2d_stage.dart` 自己**不发** `postMessage`**
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 判据（本批新增，可复用）：
   **一条链的各段不必在同一个文件** ——「舞台控件」负责**入口**（`GlobalKey`），
   「传输层」负责**出口**（`postMessage`）⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ 判据：核一条跨文件链时，
   **先画出「每一跳在哪个文件」，再逐跳核**；否则容易因为「上一段文件里没有下一段的关键词」
   而误判成「这条链断了」** ⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ 又是 B0440 第 6 形态（凭片段）的一次潜在陷阱 ——
   本批是**躲开**了它，不是踩中它。**
3. **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ 而 `ui/stage_host.dart` 也发 `postMessage`**
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 未核**：`stage_host.dart` 那条是**另一条链**（宿主↔舞台）还是同一条
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 「postMessage」在这仓不止一条通道** ⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ 判据：看到某个 API 出现 N 处时，
   要问「这 N 处是同一条链还是 N 条链」—— 它们通常不是**（与 B0611c 第 ④ 点
   「两处同义表述」同族）

## 未核
`stage_host.dart` 那条链的用途 · `live2d_transport.dart` 的发送协议
（`{version,type,payload}`？）· `input.rs` 侧对应的处理

## ⚠ 一次覆盖事故（记账，第 5 次）
`write` 返回 **`Updated file`** ⇒ `BATCH-0615b.md` 本轮之前已存在，我未 `ls` 就覆盖
（`ls AUDIT-REPO/BATCH-0615.md` 查的是**相邻号**，不是要写的那个）。
读回确认：内容为本次（46 行），`grep -rn "BATCH-0615b" *.md` 无其它文件引用它。
⇒⇒⇒⇒ ⇒⇒⇒⇒⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒⇒⇒⇒⇒⇒ 这次错因比前四次更具体**：
**我查了「有没有撞 0615」，却没查「有没有撞 0615b」** ⇒⇒
**⇒⇒⇒⇒ ⇒⇒⇒⇒⇒⇒⇒⇒ ⇒⇒⇒⇒⇒⇒⇒⇒ ⇒⇒ 纪律（升级版）：
落盘前 `ls` 的对象必须是「即将写的那个文件名本身」，
不是「这一批附近的那几个号」。**
