# BATCH-0105 · director 的可观察面：密钥**只回布尔**，且被测试钉住

Phase 1 · 域覆盖 · Mod 根第 2 批

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-mod-director/src/lib.rs` — 902（读 94-149 头注三段 + 定点 804 `state_json`）
2. `crates/live2d-ai-mod-director/src/tests_staging.rs` — （定点 676：`api_key_set` 的断言）
3. `crates/live2d-ai-mod-director/` 目录清单（10 个源文件）

## 跑过的命令（全部只读）
```
grep -rn "staging" director/src/lib.rs | grep -i "enable|设置|settings_spec|key|默认"
sed -n '100,149p' director/src/lib.rs
grep -rn "api_key_set" -B 3 -A 3 director/src/*.rs | head -16        # ← 被 head 截断，见下
git ls-files 'crates/live2d-ai-mod-director'
grep -rn "api_key_set" crates/ --include=*.rs                          # ← 穷举版
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**0 条新发现**；三处纪律核验通过 + **又一次自己的失误被当场抓住**
### ① 密钥**只回布尔**，且**有测试钉住**
```rust
// lib.rs:804
"api_key_set": self.staging.has_api_key(),      // ← 方法返回值（bool），不是值本身
// tests_staging.rs:676
state["staging"]["api_key_set"], false,          // ← 缺省态断言为 false
```
头注 :111「密钥**只回布尔** `api_key_set`，**永不回值**」⇒ **声明与实现一致**。
（对照 B0009 核过的宿主 `SettingsView.has_api_key: bool` —— **两处是同一条纪律的两个实现**。）

### ② 日志只写推导结果与**正文长度**，不写正文原文
`lib.rs:130-133`：「日志只写**推导结果与正文长度**，**不写正文原文**（请求体含用户提示词，
与 `web_api/dispatch.rs` 的『不记录请求体』**同一条纪律**）」⇒ 显式**交叉引用**宿主那条规则。

### ③ 命令路径**不触碰任何下行通道**，且有**指名**的回归
`lib.rs:123-128`：「`ModRuntime::command` 只认 `"clear"`：清空决策账本（**内存**）……
命令路径**同样不触碰**任何下行通道（回归 `tests::command_paths_never_touch_action_or_settings`）」
⇒ 断言**带名字**，可按名复核。

### ⚠ 我又一次栽在**同一族**（规则 3 第 7 变体）：**`head` 截断了 `grep` 的上下文**
我先跑 `grep -rn "api_key_set" -B 3 -A 3 director/src/*.rs | head -16`，
输出被 `head` 截在第 16 行，**恰好没露出 `lib.rs:804`** ⇒ 我据此起了一个假设
「`api_key_set` 只存在于注释、从未被实现」（**正是模式 D 的形状**）。
**穷举版** `grep -rn "api_key_set" crates/ --include=*.rs` 立刻给出 4 处命中（2 注释 + 1 实现 + 1 测试）
⇒ 假设**当场被自己的下一步推翻**。
⇒ **规则 3 第 7 变体**：**`grep … | head -N` 的截断与「不存在」不可区分**；
凡是要据此下**否定式**结论，必须**去掉 `head` 重跑一遍**（或对已列出的文件逐个定点读）。
⇒ 这是本审计**第 7 次**同族失误（B0070×2 / B0075 / B0076 / B0087 / B0088 / 本批），
**但也是被最快抓住的一次**（我在同一批内就做了穷举复核）。

## 未核实项
1. `staging` 的**开关闸门**未核到底：`staging_enabled=false` 时是否**真的**不发请求
   （`DisabledStaging` 的实现体在 `staging.rs`，未读）；谁能打开它（settings_spec 的
   `staging_enabled` 字段 ⇒ 宿主可改 ⇒ 需确认改它需要什么权限）
2. `director` 的其余 8 个源文件（`arbiter` / `decision` / `ledger` / `plan` / `presets` / `staging`）
   未读
3. 其余 7 个 Mod crate 的实现本体未读
4. `tests_staging.rs`(683) / `tests.rs`(840) / `tests_e2e.rs` 的断言体未读
