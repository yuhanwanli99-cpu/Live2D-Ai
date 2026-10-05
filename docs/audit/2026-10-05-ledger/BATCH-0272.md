# BATCH-0272 · ⭐ **Phase 4 攻 F-0049-01**：两半都仍是今天的**事实**，且**修法收窄到一句话**

Phase 4 · **证伪 / 自相矛盾** —— 攻第六条 P1

## 跑的命令（全部只读）
```
sed -n '1,8p' scripts/check_public_secrets.py
grep -rn "check_public_secrets|gitleaks|Gitleaks" .github/
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**F-0049-01 的 Phase 4 结果 = 攻击失败 + ② 收窄（修法更紧）**
```
scripts/check_public_secrets.py:5  「**GitHub CI 额外会跑 Gitleaks 覆盖完整历史**。」
$ grep -rn "check_public_secrets|gitleaks|Gitleaks" .github/
（**零命中**）
```
⇒ ⇒ **274 批之后**：「自称」仍在 · **CI 里仍零命中**（既没调它，**也没有 Gitleaks**）
⇒ ⇒ **攻击失败**，**两半都还是今天的事实**

### ⭐ 而**修法收窄到了一句话**（本批真正的收获）
该脚本头注**自陈扫描范围**是「Rust workspace（`crates/`, `xtask/`）+ 顶层配置/脚本/测试」
并**跳过二进制**
⇒ ⇒ 所以「`INCLUDED_PREFIXES` 没有 `shell/`」**不是列表的 bug，而是它的范围选择** ⇒ **与自述一致**
⇒ ⇒ 而**覆盖 `shell/` 的恰恰是那句 Gitleaks 承诺**，而**那句话不成立**
⇒ ⇒ ⇒ **真正的缺陷就是一句话**，修法只剩两条：
① **加一个真的 Gitleaks 步骤**（兑现承诺）② **或删掉那句话**（并说明 `shell/` 不在范围内）
⇒ ⇒ ⚠ **而不是**「去补 `INCLUDED_PREFIXES`」—— 那会**悄悄改变脚本的自陈范围**，**比原状更糟**
⇒ ⇒ ⭐ **可提炼**：**修一条「文档说 X」时，先分清「X 是范围选择」还是「X 是错误承诺」** ——
**前者不该动、后者该改句子** ⇒ **改错了会比不改更坏**。

## 未核实项
1. Phase 4 攻完最后两条 P1：**F-0020-01**（`MAX_TOKENS=1024` vs 主链 4096）· **F-0046-01**（wasm `origin` 检查）
2. ⭐ **F-0046-01 的「世界状态前提」也要重核**（B0271 立的规则）—— 它依赖 wasm 侧的实际代码
3. `shell/flutter` 里**是否真的可能出现密钥**（决定 ① 与 ② 哪条更该选）—— 未核
4. 那四个「未知 id」在文档里有没有被点名（B0270 敞口）
5. 能否点名会在持锁期间 panic 的 Mod（B0268 敞口）· `refresh_from_disk` 的文件变更路径（B0266 敞口）
6. 面板余面：`dev_tools_section` ~1845 · `live2d_stage` ~600 · `memory_panel` ~545 ·
   `persona_panel` ~520 · `message_bubble` ~470 · `chat_panel` ~465 · `error_banner` ~15
7. 其余 6 处 `mod_count_is_*` 逐条对账未核 · `ErrorResponse` 定义未读
8. `onReady` 是否间接清过 `presetStatus`（F-0263-01 反证 (a)，**结论不依赖它**）
9. **两侧措辞同步无机制**（B0260 敞口）· `chat_session.dart` 的 `maxSessions=50` 施加点未读
10. 前端 `.dart` 仍 181 未读（**机械值**）；Mod crates 42 未读；`mod-system` 余 8 文件未读
11. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
12. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
13. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
