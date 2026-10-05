# BATCH-0372 · ⭐⭐ **一次外部谎报被记在了它造成后果的地方**；而入口的存在理由是一条**验收句**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `dev_tools_section` 的**模型登记入口**

## 跑的命令（全部只读）
```
sed -n '166,182p' lib/settings/sections/dev_tools_section.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐⭐ **一个「谎报」被记在它造成后果的地方**
> 「后端不做文件上传（ZIP 上传是后置项且**曾被我方能力快照谎报为 supported**）——
> 用户把模型放进目录，这里登记 + 校验。
> **没有这个入口，「导入 → 激活 → …」这条 DoD 主路径在界面上就没有起点**。」    // :167-170
三个可核点：
1. ⭐⭐⭐ **一次外部谎报被写下来了**（`曾被我方能力快照谎报为 supported`）
   ⇒ ⇒ **而这与 rc.2 ② 的「假广告」（`script_invoke_supported` 假广告）是同一类事件的两个面**：
   **假广告从能力声明里被删了**（B0366 核过：面板现在只剩四个**真**事实）·
   **而这次的假来自**外部快照** ⇒⇒ **两个来源，两处都记了**
2. ⭐⭐ **「用户把模型放进目录」是既成事实** ⇒ ⇒ **模型是用户放的、不是上传的**
   ⇒ ⇒ 与 AGENTS rc.2 ③「**补上一直缺的导入入口**」一致
   ⇒ ⇒⇒ **而形态是「指向一个目录的登记」而不是文件选择器**
   ⇒⇒⇒ **同 B0361「粘贴是主路径、文件选择要宿主接线」—— 同一个约束（避开 `package:web`）的另一个应用**
3. ⭐⭐ **入口的存在理由是一条验收句**（「这条 DoD 主路径在界面上就没有起点」）
   ⇒ ⇒ **不是「方便」** ⇒⇒ **同 B0331 核的那条**（「关闭开关会**还原成主链 `system_prompt`**」）
   ⇒⇒⇒ **验收句既进代码，也进界面**

⇒ ⇒ **0 findings**；⇒ ⭐ **而本批最值钱的是第 1 点**：
**谎报被记在它造成后果的地方** —— 而不是只记「我们不做上传」。

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 定义本体 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 ·
   `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义 · `PersonaCardFilePicker` typedef 注释
2. **模型登记的「校验」本体**（`:168` 说「登记 + 校验」，**校验做了什么未核**）
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~580 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~425）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
