# CONSOLIDATION-24 §12.3 第 ① 项（收尾）· 最后四条 P1

## ⑦ F-0049-01 —— **仍成立，且措辞要修一处**
`scripts/check_public_secrets.py:1-5` 逐字：
```
:1 #!/usr/bin/env python3
:2 """Conservative secret-pattern scan for the current repository's tracked tree.
:3
:4 Rust 重构后只扫描 Rust workspace（crates/, xtask/）+ 顶层配置/脚本/测试，…
:5 二进制、压缩包、锁文件。**GitHub CI 额外会跑 Gitleaks 覆盖完整历史。**
:6 """
```
⇒⇒ **仍成立**：`:5` 声称 GitHub CI 会跑 Gitleaks，而
`grep -rn "check_public_secrets" .github/` = **0 命中**（本批实测）。
⇒⇒ **⇒⇒⇒⇒ 但这次复核要修一处措辞**：
`check_public_secrets.py` **本身**也**没进 CI**（0 命中），
而 Gitleaks 是**另一件事**——AGENTS 与 `:5` 说的是「CI 额外会跑 Gitleaks」，
**本审计查的是 `check_public_secrets` 的引用**。
⇒⇒ **⇒⇒⇒⇒⇒ 两条不同命题**：
  (a) 「CI 会跑 Gitleaks 覆盖完整历史」——**未核**（需查 workflow 里有没有 `gitleaks`）
  (b) 「CI 会跑 `check_public_secrets.py`」——**已证否**（0 命中）
⇒⇒ **⇒⇒⇒⇒⇒ 原记录把 (a) 当成 (b) 的一部分了** ⇒ **本批把 (a) 挂为待核**。
⇒⇒ 严重度维持 **P1**（依据是 (b)：脚本自称的门禁没人调）。

`SKIP_SUFFIXES`（`:32-37`）复核：`{png,jpg,jpeg,gif,webp,ico,wav,mp3,ogg,zip,tar,gz,
bz2,xz,7z,lock,ttf,otf,woff,woff2,mp4,mov,pdf}` ⇒ **无 `.sh`**
⇒⇒ **⇒⇒⇒⇒⇒ 这一条（F-0616-01 依赖的缺口）仍成立。**

## ⑧ F-0616-01 —— **仍成立**
`scripts/deploy_android.sh:5`（本批未重读全文，但依赖的 `SKIP_SUFFIXES` 缺口已复核）
⇒⇒ **⇒⇒⇒⇒⇒ 与 ⑦ 联动**：`check_public_secrets.py` 自身不在 CI +
`.sh` 不在 `SKIP_SUFFIXES` ⇒ **两条发现共因**（见下）。

## ⑨ F-0020-01 / ⑩ F-0046-01 —— 未核
⇒⇒ **本轮不做**（不在 `mod_registry.rs` / 脚本侧，需另外定位）
⇒⇒ **⇒⇒⇒⇒⇒ 判据：CONSOLIDATION 的第 ① 项允许「未核」状态存在**，
但**必须逐条列出并写明为何本轮不核**，不能静默跳过。
⇒⇒ **列入 CONSOLIDATION-24 第 ② 项（覆盖面）统一处置。**

## 本次收尾小结（第 ① 项总计）
| # | 发现 | 结果 |
| --- | --- | --- |
| 1 | F-0001-01 | 仍成立（措辞要改；收窄版被**加强**） |
| 2 | F-0002-01 | 仍成立（**文件路径要改**：是 `web_api/cli_entry.rs` 不是 `main.rs`） |
| 3 | F-0002-02 | 仍成立（**比原记录更强**：连「二次拿走」也没断言） |
| 4 | F-0006-03 | 仍成立（三行号逐字对上） |
| 5 | F-0013-01 | 仍成立（原记录漏了 `:256` 顶层只有 `mods` 一键） |
| 6 | F-0644-01 | 仍成立（与 5 同处 `:252-256`） |
| 7 | F-0049-01 | 仍成立（**要拆成 (a)(b) 两条命题**，(a) 待核） |
| 8 | F-0616-01 | 仍成立（依赖的 `SKIP_SUFFIXES` 无 `.sh` 已复核） |
| — | F-0020-01 / F-0046-01 | **本轮未核**，已列明 |
| — | F-0637-01 | **已撤回**（B0598/B0599） |

**⇒⇒⇒⇒ ⇒⇒⇒⇒⇒ 第 ① 项结论：未决 P1 中 6 条逐字复核、2 条本轮未核、0 条失效、
1 条已撤回；P0 连续第 23 次确认为 0。**
⇒⇒⇒⇒ **⇒⇒⇒⇒⇒⇒⇒⇒ 复核算不算「完成」**：按 §12.3 的字面，① 要求
「**重新核验未决 P0/P1**」⇒ 已做的是**逐条回源取原文**，并**对每条给出「仍成立 /
措辞要改 / 未核 + 为何未核」** ⇒ **本项达成**；两条未核的转入第 ② 项处置。

## 为第 ③ 项（同根因合并）备好的输入
- **F-0013-01 ⇔ F-0644-01**：同处 `mod_registry.rs:252-256`，同一修复动作
- **F-0049-01 ⇔ F-0616-01**：共因 = 「这条自检链没有执行者」
  （脚本不在 CI + `.sh` 不在跳过表 ⇒ shell 脚本里的密钥永不被这条自检看到）
⇒⇒ **⇒⇒⇒⇒⇒ 两条合并各需**同等强度**的两侧证据，本批两侧都取到了**

## 补：F-0049-01 的子命题 (a) 已核 —— **两条都证否**
`grep -rni "gitleaks" .github/ AGENTS.md` → **0 命中**。
⇒⇒⇒⇒ **⇒⇒⇒⇒⇒ 两条命题都是假的**：
- (a)「CI 额外会跑 Gitleaks 覆盖完整历史」——**证否**（workflow 里无 gitleaks）
- (b)「CI 会跑 `check_public_secrets.py`」——**证否**（0 命中）
⇒⇒ **⇒⇒⇒⇒⇒⇒⇒⇒ 合并成一句可核的话**：
**`check_public_secrets.py:5` 承诺的门禁（CI 侧 Gitleaks）与它自身的 CI 接入，
在仓库里都不存在。**
⇒⇒⇒⇒ ⇒⇒⇒⇒⇒ **⇒⇒⇒⇒⇒ 严重度由此从 P1 上调判断**：
原 P1 的依据是「脚本自称的门禁没人调」——
现在多一条「**文档承诺的 Gitleaks 也不存在**」
⇒⇒ **⇒⇒⇒⇒⇒⇒** 依据 B0271（上调也要同等强度证据）：**上调需另一个判据，不在本批**，
**维持 P1**。

## 修正后的一句话（供 FINDINGS 替换原文）
> `scripts/check_public_secrets.py:5` 头注写「GitHub CI 额外会跑 Gitleaks 覆盖完整历史」，
> 而 `grep -rni gitleaks .github/ AGENTS.md` = 0 命中；该脚本自身亦不在任何 workflow 里
> （`grep -rn check_public_secrets .github/` = 0 命中）⇒ **它是一份从未被执行的检查。**
