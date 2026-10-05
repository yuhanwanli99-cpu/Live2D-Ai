# BATCH-0051 · 门禁面收尾 —— 第二次同类核查（命中）+ 一次自我更正

Phase 1 · 域覆盖 → `scripts/`

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `scripts/ignition-precheck.sh` — 402（定点 120-159：托管层 + gstatic 体检）
2. `AGENTS.md` — （grep :105 / :608，核对 F-0040-01 的指控范围）
3. `scripts/ignite.sh` / `main.rs` / `ignition-precheck.sh` 三处「Mod 工厂数」表述对照

## 跑过的命令（全部只读）
```
grep -n "gstatic|canvaskit|index.html|main.dart.js|flutter_bootstrap|flutter.js" scripts/ignition-precheck.sh
sed -n '120,159p' scripts/ignition-precheck.sh
grep -n "mod_count_is_three" AGENTS.md ; grep -n "fn mod_count_is_five" crates/live2d-ai-desktop/src/main.rs
grep -n "五个已注册" scripts/ignition-precheck.sh
grep -n "for f in index.html main.dart.js" scripts/ignite.sh scripts/ignition-precheck.sh
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## ★ 本批两件事

**(1) F-0050-01 升级为「被复制的错误假设」** —— `ignition-precheck.sh:134` 复制了
**逐字相同**的 `for f in index.html main.dart.js`，且比 `ignite.sh` **更弱**（只 grep `main.dart.js`
一个文件，而那正是实测命中 0 的两个之一）。同一句错误探针在仓内出现 **2 次**，
**两处都漏掉 `flutter_bootstrap.js`（活配置所在）** ⇒ 不是笔误，是一个被复制的错误假设。

**(2) 自我更正 F-0040-01①（范围收窄）** —— 我在 B0040 写「AGENTS.md 点名的护栏锚点已不存在」。
本批 grep 到 AGENTS.md **另两处已经更新**（`:105` 与 `:608` 都写了
`` `mod_count_is_three` → `mod_count_is_five` ``）。⇒ 准确的说法是 **AGENTS.md 自身不一致**：
「变更历史/目录约定」跟上了，**「休眠台账」那一节没跟上**。
指控范围由「通篇未更新」收窄为「单节未更新」，严重性不变（P3）。**已显式改写 FINDINGS。**

## 维度覆盖（本批）
- 红线 K：命中（第二处同形，升级为模式）
- 模式 J（台账漂移）：**收窄为「单份文档内部不一致」**，比原判更准确

## 未核实项
1. `scripts/verify_core_chain.py`(460) 未读 —— **E2E 探针是否假绿**（任务书 §2 点名，本批未做）
2. `.github/workflows/release-build.yml`(225) 未读
3. `ignite.sh` 的 `.env` 加载段（是否 echo 泄露，红线 R）未读
4. `ignition-precheck.sh` 其余 260 行（Mod 注册表检查 / 诊断面板等）未读
5. 其余 8 个脚本未读

## 本批新增
P0 0 · P1 0 · P2 0 · P3 0 ｜ 另：**F-0050-01 升级为「被复制的错误假设」** + **F-0040-01 范围收窄并显式改写**
