# BATCH-0049 · 本地门禁执行点 —— ⭐ **F-0049-01（P1）：红线 R 的密钥扫描门禁整体缺失**

Phase 1 · 域覆盖 → `scripts/`（门禁执行点，0 审）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `scripts/check_public_secrets.py` — 108（**全读**）

`ignite.sh`(211) / `xtask/src/main.rs`(604) / `release-build.yml`(225) / 其余 10 个脚本 **未读** → 顺延 BATCH-0050

## 跑过的命令（全部只读）
```
sed -n '1,108p' scripts/check_public_secrets.py
grep -rni "gitleaks" .github/ scripts/
grep -rn "check_public_secrets" .github/ scripts/ xtask/ Cargo.toml
python3 -c "... INCLUDED_PREFIXES 含 shell/ ?"
```
未跑任何 cargo / flutter / trunk / pnpm 命令。**注：未执行该脚本本身**（执行它会读全树并可能
因真实发现而非零退出码；本批只读其源码与其被调用关系）。

## ★ 本批产出：**F-0049-01（P1）**
三条可复跑的事实叠加：
1. `check_public_secrets.py` **在 CI 里从未被调用**（B0048 已穷举全部 `run:` 步骤；AGENTS.md 的门禁表里也没有它）；
2. 它 docstring 声称的补偿控制 **「GitHub CI 额外会跑 Gitleaks 覆盖完整历史」在仓库里不存在**
   （`grep -rni gitleaks` 唯一命中就是这句话自己）；
3. 它的 `INCLUDED_PREFIXES` **不含 `shell/`** ⇒ 217 个 Dart 文件 + 全部 docs 不在扫描面。
⇒ 误粘进 `.dart` 或文档的 `sk-…` 可以直接进仓库而**无任何自动化会响**。
**这与整套密钥纪律冲突**：`.env` 是唯一真源、不入 GET/日志/WS（B0007/B0017/B0019 已核），
而最后一环「别让它进版本库」是唯一没有门禁的一环。
**附带记录（同类形态）**：行级豁免 `_is_test_line` 用 `marker.lower() in line.lower()` 且标记含
`"INJECTED"` ⇒ 与 "injected" 同行的真密钥会被跳过（与 F-0006-01 的 `redact` 只认键名同形）。

## 维度覆盖（本批）
- 红线 R：**命中**（密钥扫描门禁缺失 + 自我声明不成立）
- 维度 G：发现「键名/标记驱动 ⇒ 会漏」的第三例

## 未核实项
1. 该仓库是否在 **GitHub 平台侧**开了 secret scanning / push protection（属仓库设置，不在代码里）
   —— 如实标注；不影响「代码内无门禁」的结论
2. `ignite.sh`(211) —— **红线 K 的本地执行点**（`--check` 四条体检）未读
3. `xtask/src/main.rs`(604) —— **rust-ratio ≥95% 的统计口径**未读（它决定这条门禁的真假）
4. `verify_core_chain.py`(460) —— E2E 探针本身是否会「假绿」未读
5. 其余 9 个脚本 + `release-build.yml`(225) 未读

## 本批新增
P0 0 · **P1 1** · P2 0 · P3 0
