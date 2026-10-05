# BATCH-0170 · ⭐ **假设两次被证伪**，而 F-0040-01 的影响**升级**（0 条新发现）

Phase 1 · 域覆盖 · `performance/mod.rs` 头注 + **director 名的消歧核验**

## 跑的命令（全部只读）
```
sed -n '60,68p' performance/mod.rs
grep -rn "rule_cue|fn rule_cues|导演|director" performance/*.rs
ls -d crates/live2d-ai-mod-director
sed -n '/^members/,/^]/p' Cargo.toml
grep -rn "director" mod_registry.rs
grep -rn "mod_count_is_" main.rs
git ls-files crates/live2d-ai-mod-director
grep -rn "fn .*cue|PerformanceCue|rule" mod-director/src/*.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**0 条新发现**；**一个被我两次证伪的假设** + **F-0040-01 影响升级**
### ① 我的假设：「`mod.rs:65` 说『单一真源 = director 的纯函数』，而 director Mod 已删除 ⇒ 陈旧引用」—— **证伪两次**
| 我的推断 | 仓库事实 |
|---|---|
| AGENTS.md 说 director Mod **已删除** | 目录在，且**是 workspace member**（`Cargo.toml` members 第 15 项，注释「Wave 3 G 轨（2026-09-14）：导演**最小骨架**……**收束时由主 agent 注册**」） |
| 所以「director 的纯函数」指一个不存在的组件 | 新骨架**确实拥有**规则 cue 机制：`arbiter.rs` `Cue`/`cues()`/`cue_for()` + `lib.rs:445 rule_cues` 计数 |
⇒ ⇒ **注释是准确的**，它指的是**新** director。
⇒ **两次证伪的收获**：`AGENTS.md` 的「已删除」指的是**旧的动作序列 driver**（归档在
`archive/action-layer-p6`），而**名字被复用**给了一个**刻意 no-op 的新骨架**（`mod_registry.rs:670`
「不关心会话的 Mod（director / voice-input）行为**一字不变**」）⇒ **同名两物**，但注释指的是后者，**没错**。

### ② ⭐ 而 F-0040-01 的影响**升级**（已改规范头，见 FINDINGS）
AGENTS.md 的休眠台账把护栏写成 **`main.rs::mod_count_is_three`**，
而**实际是 `mod_count_is_five`**（`main.rs:471`，:59 注释自称「防回归断言」）
⇒ ⇒ **这不是「文档内部不一致」，而是「文档指向了一个不存在的符号」**：
**照着 AGENTS.md 去核对护栏的人，会去找一个没有的断言。**
⇒ 工厂数也已从 3 漂到 5（新增 persona / template / voice-input / wallpaper / memory / director
在 members 里，其中注册面另有取舍）⇒ **台账的「三个」比实际落后两位**。

## 未核实项
1. `mod.rs` 余 ~300 行未读（`resolve` 后半段 / `fallback()` 实现 / 自测）
2. `plan.rs` 的 `message()` 逐一与自测未读
3. `client.rs` 余 ~430 行（wire 回归**断言体**、`build_body`）
4. `llm.rs` 余 ~200 行 · `sse.rs` 余 ~200 行 · `config.rs:51-140` 未读
5. **`reqwest` 的 `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— **未核，禁止运行**
6. `runtime/src/lib.rs` 的 `join_endpoint` 回归（:184）断言体未读
7. **`mod_count_is_five` 的断言体未读** ⇒ 「5」这个数字我只核了**函数名**，未读它**断言什么**
8. `_finishTurn` 是否幂等 · `secrets.rs` 10 条测试断言体未读 · B0120「chmod toml」未核
