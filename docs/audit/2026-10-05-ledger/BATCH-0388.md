# BATCH-0388 · ⭐⭐⭐ **横扫证伪了我自己的推广** —— 而**三种不同情况都表现为「计数不等」**

Phase 4 · **证伪**（跨 Mod 横扫：把 F-0387-01 当作可推广的形态去检验）

## 跑的命令（全部只读，全部机械）
```
for c in external-input voice-input pet-desktop persona; do
  n=$(grep -rhoE 'key: "[a-z_]+"' crates/live2d-ai-mod-$c/src --include=*.rs | sort -u | wc -l)
  t=$(grep -rhoE '^\s*//!?\s*\| `[a-z_]+` \|' crates/live2d-ai-mod-$c/src --include=*.rs | sort -u | wc -l)
  echo "$c: 代码 key=$n · 文档表行=$t"
done
grep -rhoE 'key: "[a-z_]+"' crates/live2d-ai-mod-persona/src | sort -u
grep -rhE '^\s*//!?\s*\| `[a-z_]+` \|' crates/live2d-ai-mod-persona/src
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**我的推广被证伪，而证伪它的是逐项确认**
| Mod | 代码 key | 文档表行 | **真相** |
|---|---|---|---|
| `external-input` | 4 | **7** | **表比代码多** ⇒ 表里含**不是本 Mod 设**的键（env-only 之类） |
| `voice-input` | 9 | **3** | ⚠ **表自称「配置字段（settings_spec v1）」、真的漏了 6** ⇒ **F-0387-01** |
| `pet-desktop` | 3 | 3 | 相等 |
| `persona` | 8 | **2** | ⭐ **那张表是「命令」表**（`import_card` / `clear_import`）**、不是配置索引**；而那 8 个 key 是 **SillyTavern 卡片字段**（`personality`/`scenario`/`say_first_mes`/`name`/`description`…）**、不是配置旋钮** |

⇒ ⇒⭐⭐⭐ **所以 F-0387-01 是 `voice-input` 独有的**（在「那张表**自称**是配置字段索引」这个前提下）
⇒ ⇒⇒ **而三种不同的情况都表现为「计数不等」** ⇒ ⇒⇒ **计数不是判据**

### ⭐⭐⭐ 可提炼（本批唯一的产出）
> **跨仓库横扫时，「数字不一致」是**线索**，不是**判据**；
> 必须逐项确认**那张表自称为是什么**。
⇒ ⇒ **我若停在计数这一步，就会把 `persona` 的「命令表」当成「漏了 6 个配置项」** ⇒ ⇒ **一个假发现**
⇒ ⇒⇒ **而这条正好是 B0369「**凭片段**」的第七次复发**：
**我只看了两列数字，没看那张表在讲什么**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `external-input` 那张表里**多出的 3 行**是什么（表比代码多 ⇒ 可能是 env-only 或历史残留）
2. `evaluate_mode` 函数本体（245 行里我读了 10 行）· `UiSignals` 定义本体 ·
   `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 ·
   `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义 · `PersonaCardFilePicker` typedef 注释
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~420）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
