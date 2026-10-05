# BATCH-0366 · ⭐⭐ **「说真话」与「让真话不被误读」是两件事**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `dev_tools_section` 的 **capabilities / 运行态展示**

## 跑的命令（全部只读）
```
grep -nE "capabilit|Capabilities" lib/settings/sections/dev_tools_section.dart | head -6
sed -n '1096,1112p' lib/settings/sections/dev_tools_section.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐ **去广告之后的下一步**
```dart
ReadonlyField(label: '服务端版本', icon: Icons.info_outline,
  text: '${capabilities.app} ${capabilities.version}（schema v${…}，WS 协议 v${…}）'),   // :1098-1102
ReadonlyField(label: '服务端音频后端', icon: Icons.speaker_outlined,
  text: '${…['backend'] ?? '未知'}（sample_rate=${…['sample_rate'] ?? '?'}）',      // :1106-1108
  **description: 'Web 模式下音频单源是**浏览器**，服务端没有声卡是**预期的**'**),            // :1109
```
四个可核点：
1. ⭐⭐⭐ **一个「看着像异常」的读数，配一句「这是预期的」**
   —— 「服务端没有声卡」在 **Web 部署下确实是正常的**，而**新人一定会以为那是 bug**
   ⇒ ⇒ **P1 家族**（不要让用户以为坏了）
   ⇒ ⇒ ⇒ **同 B0351「产物里搜不到那句话」的思路**：**让一个事实当场可被判断**
2. ⭐⭐ **`description` 字段被用来解释「为什么这个读数看着不对」** ⇒ ⇒ **不是用来讲用法，是用来消误解**
3. ⭐⭐ **rc.2 ② 记的那些假广告**（`actions` / `action_sources` / `strength_levels` /
   `model_upload_supported` / `script_invoke_supported`）**都不在这里**
   ⇒ ⇒ **去广告的结果：面板只显示四个「确实存在」的事实**（版本 ×3 · 音频后端 + 采样率）
4. ⭐ **三个版本号并列**（app / schema / **WS 协议**）⇒ ⇒ 而 **WS 协议版本**在 UI 上可见
   ⇒ ⇒ **协议演进是可见的** ⇒ ⇒ 与 B0358 核的 `apiVersion` 可见**同族**
⇒ ⇒ **0 findings**；⇒ ⭐⭐ **而本批的收获是把两件事分开**：
> **「说真话」**（rc.2 的去广告：删掉不存在的能力声明）
> **「让真话不被误读」**（本批：给一个正常的异常读数配一句「这是预期的」）
⇒ ⇒ **前者防「界面在说假话」· 后者防「界面在说真话、而用户以为是假话」**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 定义本体 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面 · `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义 · `AdminEmpty` 本体 ·
   `PersonaCardFilePicker` typedef 注释
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1790 / `live2d_stage` ~580 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~425)
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
