# BATCH-0320 · ✅ **KL-4 第 4 条结清**：`secrets.rs` 的断言**就是红线 R 的要求清单**，逐条兑现

Phase 1 · **清账续**（把 KL-4 回正后变成可核欠账的那一条核掉）

## 跑的命令（全部只读）
```
grep -nE "fn [a-z_]+\(\)|assert" crates/live2d-ai-runtime/src/secrets.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；✅ **一条欠账结清**
十个测试、**35 处断言**，六组性质，**每组的测试名就是说它在保护什么**：

| 测试（行） | 钉住的性质 | 断言形态 |
|---|---|---|
| `parse_handles_comments_quotes_and_export`（:295） | 注释 / 引号 / `export` / **值里的 `#` 与 `=`** / **空值 ≠ 缺键** | 9 处；含 `!m.contains_key("1BAD")` + **消息「非法键名必须整行跳过」** |
| `parse_last_wins`（:321） | 同名键**后者胜** | 1 |
| `valid_key_shape`（:327） | **键名形状**：收 `DEEPSEEK_API_KEY`/`_x1`，拒 `""`/`1A`/`A-B`/`A B` | 6 |
| `merge_replaces_in_place_and_keeps_comments`（:337） | **注释保留 + 就地改值 + 旧值消失** | 4，含 **消息「注释必须保留」** |
| `merge_preserves_export_prefix_and_indent`（:347） | **`export` 前缀与缩进逐字节保留** | 1（**整串相等** `'  export A=2\n'`） |
| `merge_appends_when_missing`（:353） | 缺键时**追加** | 1（整串相等） |

⇒ ⇒ ⭐⭐ **而这六组性质 == AGENTS 对密钥真源那份口径的逐条兑现**：
「**就地改行**（**保留注释**，与 `merge_into_toml` 同一条教训）」⇒ **第 4 组**
「查找优先级 `.env` 快照 > 进程环境 > 无」⇒ 由 `refresh_from_disk`（`:110`）与 `snapshot`（`:130`）承担
⇒ ⇒ ⭐ **三处断言带人类可读的失败消息**（`非法键名必须整行跳过` · `注释必须保留`）⇒
**⇒ 失败时直接告诉维护者「哪条纪律被破了」**

### ② ⭐ 而它**顺带校准了我自己的一条 P2**（B0125）
B0125 我记的是「`parse` **严格对待键、不严格对待值**」⇒ 而**这里有测试逐字确认了这是设计**：
`WITH_EQUALS = 'a=b=c'` **被接受**、`EMPTY` 返回**空串而非缺键** ⇒ ⇒ **那不是缺口，是刻意的非对称**
⇒ ⇒ **「键严格、值宽松」是一条被测试固定的规则** ⇒ ⇒ **按 B0125 的判据，它的可接受性成立**
⇒ ⇒ ⭐ **顺带**：B0125 的**键校验**那一半**如今有了直接测试**（`valid_key_shape` 6 处）⇒
⇒ ⇒ **那条 P2 的「修法：键校验不严」在值这一侧不成立**（值宽松是**故意的**）⇒ ⇒ **措辞应收窄**（本批只记，不改 FINDINGS）

## 未核实项（本批后仍开着 4 条 + 4 条永久）
1. `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面（队列 · 两个 Timer · `_modelLoads` 注入点）· `main.dart:1240+`
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）
4. `main.rs` 余约 930 行 · `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1845 / `live2d_stage` ~600 / `memory_panel` ~545 /
   `persona_panel` ~520 / `message_bubble` ~460 / `chat_panel` ~465）
5. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
