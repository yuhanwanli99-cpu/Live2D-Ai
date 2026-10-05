# BATCH-0614 · 那条路径存在；而三个形近目录里两个是真的、一个不是

Phase 4 · 证伪（兑现 B0613 留的：docs/verification/ignition-core-loop.md 是否存在）

## 跑的命令（全部只读）
```
git ls-files docs/verification/ | grep -i ignition
git ls-files docs/verification/ | grep -ic ignition
```
未跑任何脚本。

## 结果：三个形近目录的真实状态
| 名字 | 是否存在 | 内容 |
|---|---|---|
| verify/（单数） | 不存在（B0605 核过：工作区与 git 都没有、.gitignore 也没有） | 无 |
| verification/（根） | 存在 | 4 个文件（3 个对比 PNG + soullink-p0-report.md，B0590 核过） |
| docs/verification/ | 存在 | 21 个文件（B0614 核过），含 ignition-core-loop.md |

## 三个可核点
1. ignite.sh:206 的指路有效（git ls-files 命中 1 处）
   => B0613 那条「先定位」的待核结清；而它记的是「三个形近目录」这件事本身、不是「路径失效」。
2. 而两个真目录的分工不同：根 verification/ 是证据（对比 PNG）· docs/verification/ 是清单与报告（21 个 md + assets/*.png）
   => 与 B0590 核的「三个对比 PNG 已入库」同族。
3. 而 verify/（单数）只出现在 scripts/verify_edge_alpha.py 的头注里（B0604/B0605 核过）
   => 与 B0605 那条观察（.gitignore 里没有它 ⇒「文档里写了、仓里没有」）同一条。

## 未核
docs/verification/ 其余 20 个文件的内容（该目录属参考资产、不在批次范围）·
ignition-precheck.sh 本体（402 行只读 10 行）· verify_edge_alpha.py 本体（177 行只读 14 行）·
diag-stale.sh 后续 32 行 · adb_ui_tap.py / deploy_android.sh / setup_linux.sh
