# BATCH-0271 · ⭐ **Phase 4 攻 F-0001-02**：对着**当前工作树**重核（不是凭记忆）⇒ 攻击失败 + 附带一条红线正面证据

Phase 4 · **证伪 / 自相矛盾** —— 攻第五条 P1

## 跑的命令（全部只读）
```
test -e assets/models/bai ; ls assets/models ; git ls-files assets/models
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**F-0001-02 的 Phase 4 结果** = **攻击失败**（+ 独立正面证据）
| 核验 | 结果 |
|---|---|
| `test -e assets/models/bai` | **不存在** |
| `ls assets/models` | 只有 `README.md` |
| `git ls-files assets/models` | 只有 `.gitkeep` 与 `README.md` |

⇒ ⇒ **273 批之后**（B0001 记的「不存在」）**仍然成立** ⇒ **攻击失败**
⇒ ⇒ 那个 `||` 的后半段在当前树上**依然是空的** ⇒ **`assert` 实际从未校验过那张 model3.json**
⇒ ⇒ ⭐ **顺带一条独立正面**：`assets/models` **没有任何被跟踪的二进制**
⇒ ⇒ 与 AGENTS「模型由用户合法导入、`assets/models/` **不捆绑二进制**」**完全一致**
⇒ ⇒ 这是那条红线的**独立正面证据**（**不是因为本条才去看**）

### ⚠ 而本批最大的收获在**我差点犯的错**上
我**差点直接引用**「B0001 记过 `bai` 不存在」⇒ ⇒ **那正是 CONSOLIDATION-18 立的规矩禁止的事**
（「**关于世界的事实会过期；前提必须重核，不能靠记忆**」）
⇒ ⇒ **本批的收益本身就是那次教训的兑现**：**因为去核了，才没把一条 273 批前的记忆当成今天的事实**。

## 未核实项
1. Phase 4 待攻 P1：F-0020-01 · F-0046-01 · F-0049-01
2. 那四个「未知 id」在文档里有没有被点名（B0270 的敞口）
3. 能否点名会在持锁期间 panic 的 Mod（B0268 敞口）· `refresh_from_disk` 的文件变更路径（B0266 敞口）
4. ⭐ **同类检查应推广**：**凡是以「世界状态」为前提的发现，都应像本条这样重核一遍**
   （候选：`F-0046-01` 依赖 `l2d-wasm-demo` 的构建产物布局 · `F-0049-01` 依赖 CI 配置内容）
5. 面板余面：`dev_tools_section` ~1845 · `live2d_stage` ~600 · `memory_panel` ~545 ·
   `persona_panel` ~520 · `message_bubble` ~470 · `chat_panel` ~465 · `error_banner` ~15
6. 其余 6 处 `mod_count_is_*` 逐条对账未核 · `ErrorResponse` 定义未读
7. `onReady` 是否间接清过 `presetStatus`（F-0263-01 反证 (a)，**结论不依赖它**）
8. **两侧措辞同步无机制**（B0260 敞口）· `chat_session.dart` 的 `maxSessions=50` 施加点未读
9. 前端 `.dart` 仍 181 未读（**机械值**）；Mod crates 42 未读；`mod-system` 余 8 文件未读
10. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
11. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
12. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
