# BATCH-0616 · scripts/ 结项 · 找到 1 条 P1

Phase 1 · 域覆盖 · 脚本（scripts/deploy_android.sh 147 行 + scripts/setup_linux.sh 31 行）

## 跑的命令（全部只读）
```
wc -l scripts/deploy_android.sh scripts/setup_linux.sh
sed -n '1,8p' scripts/deploy_android.sh
git ls-files --error-unmatch scripts/deploy_android.sh
grep -nE "SKIP_SUFFIXES|\.sh" scripts/check_public_secrets.py | head -6
grep -rn "check_public_secrets" .github/
grep -c "check_public_secrets" AGENTS.md
```
未跑脚本（**运行脚本不在允许清单内**：deploy_android.sh 会连设备、setup_linux.sh 会装依赖）。

## 本批产出
**F-0616-01 (P1)** —— deploy_android.sh:5 的头注里有**设备 IP + 锁屏 PIN**，
而**扫描器结构上能看到这个文件（SKIP_SUFFIXES 不含 .sh）、却不被调用（.github/ 零命中、AGENTS 门禁表零命中）**。
（详见 FINDINGS.md）

## scripts/ 结项（12 个文件）
| 文件 | 状态 |
|---|---|
| ignite.sh 211 | ✅ 结项（B0609–B0613） |
| ignition-precheck.sh 402 | 头注已核、本体未读 |
| verify_core_chain.py | AGENTS 记（18 跳） |
| check_public_secrets.py | F-0049-01 + 本批读了 SKIP_SUFFIXES |
| font_subset_ranges.py | AGENTS 记（FNV-1a） |
| slop_miner.py 161 | ✅ B0601 |
| verify_edge_alpha.py 177 | ✅ B0604–B0606（+ F-0606-01） |
| diag-stale.sh 40 | ✅ B0603（头注 8 行） |
| run-web.sh 30 | ✅ B0608（全文） |
| **deploy_android.sh 147** | ✅ **B0616（+ F-0616-01）** |
| **setup_linux.sh 31** | 仅行数（**未读**） |
| adb_ui_tap.py 147 | ✅ B0615 |

## 未核
setup_linux.sh 本体（31 行）· ignition-precheck.sh 本体 · verify_core_chain.py 本体 ·
check_public_secrets.py 本体 · font_subset_ranges.py 本体 · diag-stale.sh 后续 32 行 ·
F-0616-01 反证条款（该 PIN 是否已轮换 —— 需访问设备，**超出只读范围**）。
