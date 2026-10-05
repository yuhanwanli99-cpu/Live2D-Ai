# BATCH-0053 · E2E 探针 + 门禁轴结项

Phase 1 · 域覆盖 → 门禁轴最后一块

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `scripts/verify_core_chain.py` — 460（**全读**：决策结构 + 291-390 收帧与判据 + 391-460 收尾与退出码）

## 跑过的命令（全部只读）
```
grep -n "def |sys.exit|return True|return False|except|timeout" scripts/verify_core_chain.py
sed -n '291,390p' scripts/verify_core_chain.py ; sed -n '391,460p' scripts/verify_core_chain.py
```
**未执行该脚本**（它会对一个真实服务发 `POST /api/v1/chat`，即产生副作用；本任务禁副作用）。

## 本批产出
**0 条新发现**；门禁轴**结项**。结论：任务书 §2 的「绝不伪造绿灯」纪律
在**脚本侧成立**（退出码由失败集决定、超时判失败、三条必查项显式禁止跳过），
且探针把「动作系统已拆除」写成了**可执行断言**（:437-440，`action_state` 出现即失败）。

## ⭐ 门禁轴最终结论（CONSOLIDATION-06 + 本批合成）
**「门禁不弱，弱的是门禁自己的诚实性」** —— 现补上最后一块：
- ✔ **可靠的三处**：`xtask` rust-ratio（窄排除表 + 豁免仍可见）·
  `nightly.yml` E2E opt-in（skipped 而非 passed）· **`verify_core_chain.py`（本批：退出码、
  超时、必查项、句界、休眠断言全部可核）**
- ✘ **三处覆盖洞**：F-0048-01（红线 K 无 CI 网）· F-0049-01（红线 R 密钥扫描整体缺失 +
  docstring 不成立）· F-0050-01（唯一本地图探错文件，被复制到两处）
⇒ **代码纪律强、门禁设计强，但门禁覆盖面薄，且有一处用假话掩盖缺失。**

## 未核实项
1. `release-build.yml`(225) 未读（门禁轴最后一个未读 workflow）
2. `ignite.sh` 的 `.env` 加载段未读（是否 echo 泄露，红线 R）
3. 其余 8 个脚本未读（`diag-stale.sh` / `slop_miner.py` / `font_subset_ranges.py` /
   `adb_ui_tap.py` / `run-web.sh` / `setup_linux.sh` / `deploy_android.sh`）
4. 探针的**实测**未做（需真服务，禁跑）—— 故只核了它的判定逻辑，未核它跑起来是否 18 跳全过

## 本批新增
**0 条**（净产出：门禁轴结项 + 一条正面样本「休眠台账→可执行断言」）
