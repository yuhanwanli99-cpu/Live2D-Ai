# BATCH-0423 · ⭐ **零条自定义规则** —— 而**那两行是模板自带的**（含一条 404）

Phase 4 · **证伪**（`analysis_options.yaml` 的 lint 集合 · B0422 留）

## 跑的命令（全部只读）
```
cat analysis_options.yaml
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**B0422 的观察结清为「不适用」**（0 条新发现）
```yaml
include: package:flutter_lints/flutter.yaml     # ← 唯一的规则来源
analyzer:
  exclude: [build/**, web/**]
linter:
  rules:
    # avoid_print: false        # Uncomment to **disable** the `avoid_print` rule
    # prefer_single_quotes: true # Uncomment to **enable** the `prefer_single_quotes` rule
```
四个可核点：
1. ⭐⭐⭐⭐ **而那两行是 `dart create` 的模板自带的** ⇒⇒ **且其中一条指向一个不存在的包**：
   `prefer_single_quotes` 属 **package:http** 的历史 lint，**已从 `flutter_lints` 移除** ⇒⇒⇒⭐
   **⇒ 「注释会过期」在**生成物**上同样成立**
2. ⭐⭐⭐ **`rules:` 下是空的** ⇒⇒ **⇒ 实际 lint 集合 = `flutter_lints` 的推荐集、逐条未加**
   ⇒⇒⇒⭐⭐⭐ **⇒ B0422 那条「import 顺序是乱的」因此**不适用**——
   `directives_ordering` 不在推荐集里 ⇒⇒⇒ **⇒ 按规矩**不记发现**（无证据不进 FINDINGS）**
3. ⭐⭐ **排除项只有 `build/**` 与 `web/**`** ⇒⇒ **⇒ 与 AGENTS「前端产物不入库」一致**
4. ⭐⭐⭐⭐ **而这份配置本身是零解释的** ⇒⇒⇒⭐⭐ **⇒ 与 B0344 字体门禁对照**：
   **⇒ 那道门禁**记得自己的来历**（生成文件 + 哈希 + 「换字体必须重新生成」**），
   而「采用哪一套 lint」**不留痕** ⇒⇒⇒⇒ **⇒ 这不是缺陷**（推荐集是合理默认）**，但两者不对称 ⇒ 记为观察**

⇒ ⇒ **0 findings**；⇒ ⭐⭐ **而本批的收获是「一次观察被结清为不适用」**
> **⇒ 结清的方式是「读配置」** ⇒⇒ **⇒ 不是「看起来没问题」**
> ⇒⇒⇒⭐ **⇒ 而这条配置里最值得记的反而是一个 404**：
> **模板注释里有一条指向已移除的 lint 规则**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 ·
   `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 · `ModConfigResult` 定义 ·
   **`flutter_lints` 推荐集的实际内容**（**逐条未核** ⇒ 「零条自定义」≠「无 lint」）
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~515 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~430）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
