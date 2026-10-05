# BATCH-0368 · ✅ **接收端找到了，而且它把 F-0046-01 想要的东西全做了** —— 结论：**不需要重开该条**

Phase 4 · **证伪** —— 结清 B0367 建立排除清单后留下的那条「接收端在哪」

## 跑的命令（全部只读）
```
grep -rn "addEventListener|onmessage|postMessage" crates/l2d/src/ --include=*.rs      # ⇒ 零命中
grep -rln "postMessage" --include=*.html --include=*.js --include=*.dart shell/ crates/ | grep -v /dist/
grep -rn "addEventListener" shell/flutter/lib/ --include=*.dart
sed -n '38,72p' shell/flutter/lib/live2d/live2d_host_web.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**F-0046-01 的实质问题得到肯定回答**
**它不在 `l2d-wasm-demo`（B0367 已证），也不在 `l2d/src`（本批零命中）——
它就是 Flutter 宿主自己。** `shell/flutter/lib/live2d/live2d_host_web.dart`：
```dart
/// iframe 传输：`postMessage(jsonString, location.origin)` 发送，
/// 监听 window message 并**校验 `source == iframe.contentWindow` 且 `origin == location.origin`**。  // :40-42
class WebIframeTransport implements Live2DTransport {
  _listener = ((web.MessageEvent event) {
    try {
      if (**event.origin != web.window.location.origin**) return;              // :48  ① origin
      final source = event.source;
      if (source == null || **source != _iframe.contentWindow**) return;      // :49-50  ② source
      final data = event.data;
      if (data == null || !data.isA<JSString>()) return;                    // :51  ③ 类型
      if (!_controller.isClosed) _controller.add((data as JSString).toDart);  // :52
    } catch (error) { onError('渲染面消息解析失败：$error'); }                  // :54-55
  }).toJS;
  web.window.addEventListener('message', _listener);                          // :58
```
五个可核点：
1. ⭐⭐⭐ **接收端在 `live2d_host_web.dart`**（Flutter 宿主）⇒ ⇒ **B0367 的排除清单帮我定位到了它**
2. ⭐⭐⭐ **`origin` 被校验了**（`:48`）⇒ ⇒ **「任何页面都能往这个 iframe 发消息」这件事**不成立**
3. ⭐⭐⭐ **`source` 也被校验了**（`:49-50`，`source == _iframe.contentWindow`）
   ⇒ ⇒ **双重身份校验**（**同源** + **同一个窗口**）⇒ ⇒ **比 F-0046-01 建议的还多一项**
4. ⭐⭐ **而且是**头注先写明**这条规则**（`:40-42`：「发送用 `location.origin`」+「监听校验 source 与 origin」）
   ⇒ ⇒ **两侧的约定写在实现旁边** ⇒ ⇒ 又一次「指路」
5. ⭐⭐ **`dist` 里的 `postMessage` 是构建产物** ⇒ **B0275 的判断（wasm 源码树里真实调用 = 0）是对的**
   ⇒ ⇒ **而我 B0275 写下的那句「留下的是『这一版没有这条通道』」**—— **它说的是对的，只是通道搬到了宿主侧**

⇒ ⇒ **结论：F-0046-01 的实质（缺 origin 防护）**在当前树上**不成立** ⇒⇒
**该条**不重开**（B0275 的撤回**结论正确**、**理由需要补这一句**）
⇒ ⇒ 而**两轮 B0367/B0368 的实际产出**是：**把一个我一度想「重开」的 P1，用排除清单定位到了它真正的家，并确认了它已按最严的方式实现。**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 定义本体 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 ·
   `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义 · `PersonaCardFilePicker` typedef 注释
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1790 / `live2d_stage` ~580 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~425）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
