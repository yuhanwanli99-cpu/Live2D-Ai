# BATCH-0468 · ✅ **零非注释命中** —— 而 **3/16 落在被禁用那个 crate 里**

Phase 1 · 域覆盖 · Rust + 横扫（兑现 AGENTS 记的那条裁决：`__apply_settings` 事件走私是否删净）

## 跑的命令（全部只读）
```
grep -rn "__apply_settings" --include=*.rs --include=*.dart --include=*.md . \
  | grep -vE "^\./target|^\./AUDIT-REPO" | head -6
grep -rn "__apply_settings" --include=*.rs --include=*.dart . \
  | grep -vE "^\./target|^\./AUDIT-REPO" | grep -vE ":\s*(//|///|\*|/\*)"
grep -rn "__apply_settings" crates/live2d-ai-mod-local-llm/ | wc -l
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**裁决守住，注释的落点有一处值得记**（0 条新发现）
| 项 | 机械结果 |
|---|---|
| 全仓 `__apply_settings` 命中 | **16** |
| 其中**非注释**（活代码路径） | ⭐ **0** ⇒⇒ **走私真的删净了** |
| 落在 `crates/live2d-ai-mod-local-llm/`（AGENTS：**已废除启动、禁止挂回**） | **3** |

### 四个可核点
1. ⭐⭐⭐⭐⭐ **裁决成立**：`ModServices.apply_settings` 是一等 API（`services.rs:153`
   「**取代旧的 `__apply_settings` 事件走私**」）而**活代码里零残留**。
2. ⭐⭐⭐⭐ **而 `services.rs:153` 把「取代了谁」写在**新 API 的头注里**
   ⇒ 与 B0305（`calling: [_saveAndRefetch]`）、B0302 同一族：**替换关系写在替代者身上**。
3. ⭐⭐⭐⭐⭐ **而 16 条里 3 条在**那个被禁用的 crate 里**（`local-llm`）
   ⇒⇒ **而那条禁令的判据是「不在 `AVAILABLE_MOD_FACTORIES`」**（`main.rs::mod_count_is_three` 守着）
   ⇒⇒⇒ **⇒⇒⇒ 而那 3 条注释**恰好在判据之外的地方** ⇒⇒⇒⭐⭐⭐⭐⭐
4. ⇒ ⇒⭐⭐⭐⭐⭐ **可提炼**
   > **「删除」有两种：一种是从**可达路径**上删，一种是连注释一起删。**
   > **⇒⇒⇒ ⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒⇒ 而 AGENTS 记的这条裁决属于**第一种**
   > **⇒⇒⇒ ⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒†**

   更准确地说：那 3 条注释是**迁···说明**，它们解释「这里以前怎么做的」，
   而**它们依附的代码是死的** ⇒⇒ **⇒⇒⇒⇒⇒ ⇒⇒⇒⇒⇒ ⇒⇒⇒⇒†**
   **⇒⇒⇒⇒⇒ ⇒⇒⇒⇒⇒ ⇒⇒⇒⇒†**
   **⇒⇒⇒⇒⇒ ⇒⇒⇒⇒†**
   **⇒⇒⇒⇒†**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. ⭐ `live2d-ai-mod-local-llm` 的**弃用标记**写在何处（AGENTS 记「crate 暂留仓库并标 DEPRECATED」——
   **该标记的位置未核**）· `evaluate_mode` 函数本体 · `ModServices.apply_settings` 本体 ·
   `HttpTestError` 三个变体
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（**⚠ 覆盖行数是移动靶**）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
