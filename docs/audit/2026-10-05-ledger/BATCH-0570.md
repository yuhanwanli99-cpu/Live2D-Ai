# BATCH-0570 落盘（无装饰）· 行数上限**没有任何自动检查** => B0569 那条降为 P3

## 跑的命令（只读）
grep -rniE "800|line.?limit|max.?lines|wc -l" xtask/src/ .github/workflows/
ls xtask/src/  ->  只有 main.rs（一个文件）
grep -nE '"rust-ratio"|未知子命令' xtask/src/main.rs
grep -rn "xtask" .github/workflows/

## 结果：全仓只有一个子命令
xtask/src/main.rs:91-94：
  if first != "rust-ratio" {
      return Err(format!("未知子命令 `{first}`（可用子命令：rust-ratio、help）"));
      }
=> **xtask 只有一个子命令 rust-ratio**（外加 help）。**没有** line-limit / file-size 之类。
CI 里 xtask 只被调用一次，且两次都是同一个子命令：
.github/workflows/pr-checks.yml:68   run: cargo run -p xtask -- rust-ratio
.github/workflows/nightly.yml:59     run: cargo run -p xtask -- rust-ratio

## 那个唯一带「行数」的地方不是检查
xtask/src/main.rs:11  「每文件按**物理行数**计数（与 wc -l 一致，可独立复核）」
xtask/src/main.rs:328 「物理行数：\n 的个数；末尾无换行的残行补 1」
xtask/src/main.rs:348 「统计口径 : 物理行数（每文件 \n 计数…）」
=> 这三处是 **rust-ratio 的统计口径说明**，不是上限检查。
=> 换句话说：**「<=500 行」「测试 <=800 行」两条规则，在仓库里没有任何一道门禁在看**。

## 结论（B0569 那条降级）
B0569 说 director/tests.rs 840 行违反 AGENTS 自定的 800 行上限、无豁免理由。
现在核完：**没有任何 CI / xtask 检查行数**。
=> 严重度从候选 P2 降为 **P3（纯文档与仓库的偏差）**：
   - 不是「CI 会红而没人修」（没有这道 CI）
   - 是「规则只存在于 AGENTS 一句话里，靠人自觉」
=> **仍记为 P3**，因为它是一条可核的、**规则被引用过（tests_staging.rs:4 引用了它）
   但没被满足**的事实。

## 三个可核点
1. 「一条禁令要带后果」（B0500）的反例：这条禁令**没有后果** —— 没有任何门禁消费它。
   ⇒⇒ 与 B0500 核过的三处「禁令 + 后果」形成对照（那三处都能找到执行点）。
2. 而 B0007 核的 500 行上限同样**无门禁** ⇒ 两条上限规则是同一族。
3. **⇒⇒ 可提炼的新判据**：「文档里的一条规则，先问『谁会执行它』，
   找不到执行点 ⇒ 它是 P3 而不是 P2」⇒⇒ 与 B0500「缺省值写在参数表里」**互补**：
   **那一条要求「谁负责执行」写清楚；这一条要求「先确认有没有执行者」。**

## 未核
arbiter/decision/ledger/presets/plan/staging* 本体 · 其余 8 个 mod crate 本体 ·
mod-system 的 tests/ 四行 · shared/ 其余 8 个 json · verification/ 目录
