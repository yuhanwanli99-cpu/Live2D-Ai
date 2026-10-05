# BATCH-0592 落盘（极简）· W1 块 = 给 F-0637-01 定的「现状更正」清单，AGENTS 却仍未改

## 命令（只读）
```
grep -n "W1" docs/plans/IMPL-PROMPTS-actions-performance-round.md
  -> :8 Wave 0 / :35 分隔 / :50 任务标题
sed -n '50,80p' …
```

## W1 块逐字（要点）
```
:50 【任务 W1：文档对齐——把「动作/表演现行状态」写进文档（只改文档…】
:51 问题：AGENTS.md（-l1）仍写「LLM 工具层与动作系统已整体拆除」（:112）…
:52 docs/plans/PRODUCT-L1-GOALS-2026-09-15.md:39 的非目标写「不复活 Action」、direct…
:53 而分支现实是：[action] 段存在、9 条动作包（assets/actions/presets.json）、…
:54 director 在 mods.json 里缺省启用、[performance] 段存在但缺省关。文档与代码…
必做：
1. 在 AGENTS.md 的「动作与表演的归属（休眠台账）」一节后新增小节，逐条列出
   ① 动作包（表情/手势）经 preset 帧直接驱动渲染面参数
   ③ **director Mod 缺省启用且会产出 latest.preset_id 与 action_cue**
   ④ [performance] …
   ⑤ core 的 action/performance 子系统仍无驱动方
   **每条都要写「真源文件」，不要只写结论。**
2. 把 :112 与 :256-268 里**已不成立**的断言改成带现状更正的说法（保留…）
3. PRODUCT-L1-GOALS-2026-09-15.md：在 §5「非目标」的「不复活 Action」与 §2 表…
   （该文件是 2026-09-15 的历史任务书，**不要重写历史结论**，只加「已…」）
4. action-packs-v0.md：加一节「与实现的偏差（2026-09 实测）」…
   ⚠ §3.4 的 D1 那行**已被维护者裁决撤回**，**不得只引 D1 的结论** ——
   否则等于把已被否掉的口径又写回文档。
   **director-rfc.md 不在本块范围**（归 W8，避免同文件并发）。
5. docs/README.md 索引补上新文档
6. **把产品口径写进 AGENTS.md**（维护者 2026-09-21 裁决，原文照抄，不要…）
   「本项目 = **Live2D 皮套 + AI 接入的底座**，由 **Mod 分化各场景**。…
     情绪/表演决策的**输入是用户输入**（用户说了什么 → 皮套怎么反…
   放在「动作与表演的现行状态」小节的最后一段，并注明这是维护者…
禁止：改任何 .rs / .dart；在文档里下架构裁决（裁决是维护者的事）；…
验收：grep -rn '动作系统已整体拆除' AGENTS.md 命中的行后面必须紧跟现状…
     新增小节里的每个「真源」路径都能在树上找到（自己 ls 一遍并把…）
【文件归属】AGENTS.md、PRODUCT-L1-GOALS-2026-09-15.md、action-packs-v0.md、
           docs/README.md（**只这些**；director-rfc.md 归 W8）
```

## 本批核到的东西（与 F-0637-01 直接相关）
1. **⇒⇒⇒⇒ W1 写明「director Mod 缺省启用且会产出 `latest.preset_id` 与 `action_cue`」**
   （必做第 1 条第 ③ 项），而 **`AGENTS.md` 今天仍写「director 已于 0.1.0-rc.2 删除」**。
   ⇒⇒ **⇒⇒⇒⇒** 派工清单里点名要改的那一行**至今没改**。
2. **⇒⇒⇒⇒ 验收条款是可机械检查的**：
   「`grep -rn '动作系统已整体拆除' AGENTS.md` 命中的行后面**必须紧跟现状**…」
   ⇒⇒ **⇒⇒⇒⇒** 判据：一条验收如果能写成 `grep + 「紧跟」`，就说明它当初是能自动判的
   —— 现在没判，说明这条验收**没被执行**，不是「无法执行」。
3. **⇒⇒⇒⇒ 一处派工内部的不一致（本批发现）**：
   必做第 1 条第 ③ 项说「director **缺省启用**」，
   而 B0565 核过 `cli_entry::default_mods_manifest` **只含 `external-input`**
   （`cli_entry.rs:651-656`），`REGISTER-director-v0.md:44` 也写
   「**不改缺省 manifest**：director **缺省停用**」。
   ⇒⇒ **⇒⇒⇒⇒** 「缺省启用」与「缺省停用」**直接矛盾**。
   ⇒⇒ **⇒⇒⇒⇒** 但**本审计不下结论**：「缺省」这个词在三处可能指不同的层
   （`mods.json` 文件的 `enabled` / `default_mods_manifest` / 面板显示）。
   ⇒⇒ **⇒⇒⇒⇒** 判据：碰到「缺省」先问「**谁的缺省**」。
   **⇒ 未核**：`mods.json` 里 director 的 `enabled` 现值（本批未读）。

## 一条正面模式
W1 第 3 条：「该文件是 2026-09-15 的历史任务书，**不要重写历史结论**，只加「已…」」
⇒⇒ **⇒⇒⇒⇒** 判据：**历史任务书不重写，只追加更正**（与 P34「保留正文 + 作废清单」同源）。

## 未核
`mods.json` 里 director 的 `enabled`（决定第 3 点）· W1 第 4 条提到的 D1 撤回 ·
`PRODUCT-L1-GOALS-2026-09-15.md` / `action-packs-v0.md` 是否存在
