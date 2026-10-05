# BATCH-0319 · ✅ **KL-4 两处误列当场回正**；而第 5 条的答案是「**没有，而且这是对的**」

Phase 1 · **账本自身校准**（回正 CONSOLIDATION-22 §⑤ 立下的「不可核」清单）

## 跑的命令（全部只读）
```
grep -c "assert" crates/live2d-ai-runtime/src/secrets.rs ; grep -n "fn secrets_|#\[test\]" …/secrets.rs | wc -l
grep -rn "set_permissions|chmod|from_mode|PermissionsExt" crates/ --include=*.rs | grep -v test
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；✅ **两条永久欠账被证明是普通欠账**
| KL-4 条目 | 核验结果 | 处置 |
|---|---|---|
| #4 `secrets.rs` 十条测试的**断言体** | **35 处 `assert`**、10 个测试点，**全在仓库里、读得到** | **移出永久不可核** ⇒ 回到普通未核实项（**我欠账，不是不可核**） |
| #5 B0120「chmod toml **是否在别处也做**」 | **生产代码里 `set_permissions` 零命中**（`secrets.rs:280-281` 与 `:378` 两处**都在测试模块里**） | **不是欠账 —— 是结论**（见 ②） |

### ① ⇒ 永久不可核从 6 条降到 **4 条**
真正不可核的只剩：`reqwest` `.timeout()` 覆盖 body？（依赖内部，**禁运行**）·
动作计划 token 长度（**需生成本身，禁运行**）· `_finishTurn` 幂等（**需并发实验**）·
GitHub 侧 secret scanning（**仓库设置，看不到**）

### ② ⭐ 而 #5 的答案带一个**理由**，不只是「没有」
生产代码里**没有任何地方 chmod `live2d-ai.toml`** ⇒ ⇒ 而这**不是遗漏，是正确**：
AGENTS 明写「`live2d-ai.toml` **只持 `api_key_env`（变量「名」）**」⇒ ⇒ **里面没有密钥 ⇒ 无需 0600**
⇒ ⇒ 对照：`.env` 走 `secrets` 的写入路径、**有** `0600`（B0120 已核）
⇒ ⇒ ⭐ **两个文件的权限差别不是疏漏，是它们的内容差别** ⇒ ⇒ **B0120 那个问句（挂了 100+ 批）现在有答案了**

### ③ ⭐ 而本批真正的方法论收获是：**「不可核」这个标签本身也会出错，而它需要复核机制**
⇒ CONSOLIDATION-22 §⑧ 写下「哪些是我选择不查的、而不是漏了没查」时，**顺手多写了半句**
「**⇒ 「不可核」这个标签本身也会出错 ⇒ 下一批把它们移出本节**」
⇒ ⇒ **下一批（本批）真的照做了** ⇒ ⇒ ⭐ **账本里写下的自我修正，会在下一批真的被执行**
⇒ ⇒ **而如果没有那半句，这两条误列会一直挂着**，直到读者（或我）把它们当成「已尽力的边界」

## 未核实项（本批后仍开着 5 条 + 4 条永久）
1. `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面（队列 · 两个 Timer · `_modelLoads` 注入点）· `main.dart:1240+`
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）
4. `main.rs` 余约 930 行 · `requestAnimationFrame` 的**暂停语义**
5. 面板余面（`dev_tools_section` ~1845 / `live2d_stage` ~600 / `memory_panel` ~545 /
   `persona_panel` ~520 / `message_bubble` ~460 / `chat_panel` ~465）
6. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
7. `secrets.rs` 的 35 处断言**逐条**核（可核，欠账）—— 下一批可批量核
