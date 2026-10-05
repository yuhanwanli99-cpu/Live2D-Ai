# BATCH-0050 · ⭐ **`ignite.sh --check` 探针文件选错**（F-0050-01 P2）

Phase 1 · 域覆盖 → `scripts/`（门禁执行点）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `scripts/ignite.sh` — 211（定点 88-101 的 gstatic 体检、:167-190 的构建与产物探测）
2. `xtask/src/main.rs` — 604（定点 30-89：扩展名表 / 排除目录表 / 门槛）
3. `shell/flutter/build/web/**` — **构建产物**（只读 grep 计数；非源码，属实测证据）

`verify_core_chain.py`(460) / `release-build.yml`(225) 未读 → 顺延 BATCH-0051

## 跑过的命令（全部只读）
```
grep -n "EXCLUDE|SKIP|exclude|THRESHOLD|0.95" xtask/src/main.rs
sed -n '30,89p' xtask/src/main.rs
grep -n "gstatic" -B 6 -A 14 scripts/ignite.sh
grep -rl "gstatic" shell/flutter/build/web/
grep -c 'gstatic\.com/flutter-canvaskit' shell/flutter/build/web/{index.html,main.dart.js,flutter_bootstrap.js,flutter.js,flutter_service_worker.js}
grep -o "canvasKitBaseUrl[^,}]*" shell/flutter/build/web/flutter_bootstrap.js
```
未跑任何 cargo / flutter / trunk / pnpm 命令（**未执行 ignite.sh 本身**：它会起服务）。
本批读了 `shell/flutter/build/web/**` 仅为 grep 计数取证据，**不作为审计对象**。

## ★ 本批产出：**F-0050-01（P2）** —— 红线 K 连本地网都没有
用门禁那条**一模一样的**模式逐文件实测真实产物：
`index.html` 0、`main.dart.js` 0 ← **门禁只探这两个**；
`flutter_bootstrap.js` **1**、`flutter.js` **1** ← **门禁从不探**。
而活的 CDN 决策就在 `flutter_bootstrap.js`：
`canvasKitBaseUrl ? n.canvasKitBaseUrl : e.engineRevision && !e.useLocalCanvasKit ? W("https://www.gstatic.com/flutter-canvaskit", …)`
⇒ 门禁的**模式对、层次对（先查 200 再 grep，避开「404 空 body 假绿」）**，**唯独文件清单选错**。
**当前构建是安全的**（`--no-web-resources-cdn` 置 `useLocalCanvasKit` ⇒ CDN 分支为死代码）——
本条指控的是「**门禁抓不住它被写出来要抓的那个故障**」，不是「产品现在坏了」。
**边界如实标注**：只读约束下无法构造坏构建来实证那次假绿（禁跑 `flutter build`），
故定 P2 而非 P1；能确证的是「探针集 ∩ 字符串所在文件 = ∅」这一结构事实。
⇒ **对 F-0048-01 的影响升级**（已显式改写）：红线 K **没有任何可用的自动化防护**（CI 无 + 本地不可靠）。
⇒ 修法有先后依赖：**必须先修 F-0050-01 再把 `ignite.sh --check` 接进 CI**，
否则等于把一个探错文件的门禁自动化。

## 维度覆盖（本批）
- 红线 K：命中（F-0050-01）
- 门禁健壮性：`xtask` 的 rust-ratio **排除表窄且有据**（7 个目录名逐条有理由，`dart` 豁免但
  **仍单独打印行数**「防止前端悄悄膨胀成为审计盲区」，:41-42）⇒ 我「宽松排除表让门禁自己及格」的
  假设**证伪**；`DEFAULT_THRESHOLD = 95.0`、`EXIT_BELOW_THRESHOLD = 1`（:60-64）闭环

## 未核实项
1. `verify_core_chain.py`(460) —— E2E 探针是否会「假绿」未读（任务书 §2 纪律点名）
2. `release-build.yml`(225) 未读
3. 其余 9 个脚本（`ignition-precheck.sh` 402 / `run-web.sh` / `setup_linux.sh` /
   `deploy_android.sh` / `diag-stale.sh` / `slop_miner.py` / `font_subset_ranges.py` /
   `adb_ui_tap.py`）未读
4. `ignite.sh` 只读了 88-101 与 167-190 两段；`.env` 加载段（`set -a; . ./.env`）**未读**
   （B0050 NEXT 里点的「是否 echo 泄露」尚未核）

## 本批新增
P0 0 · P1 0 · **P2 1** · P3 0 ｜ 另：**升级 F-0048-01 的影响段**（红线 K 无任何可用自动化防护）
