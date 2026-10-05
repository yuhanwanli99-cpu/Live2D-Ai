# BATCH-0147 · 第五次换根 → `web_api/` 余 8 文件：**全是测试**，且是**三向对照**写法

Phase 1 · 域覆盖 · `web_api/`（补齐 §③ 判据的唯一实证来源）

## 跑的命令（全部只读）
```
python3 - <<'PY'   # 机械算 web_api 下哪些 .rs 未在 INDEX 记过
  … 先按文件名比（错）→ 改成按 "web_api/<相对路径>" 比（对）…
PY
grep -n "    fn |^fn " web_api/models_routes/tests_models_p1.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## ① 覆盖率结论（**修正我自己的机械错误**）
第一次跑机械比对时我把 INDEX 的 `web_api/<文件>` 路径与**裸文件名**比较 ⇒ 得出
「未审 44 个」——**完全错误**（我此前已审 40 个）。
修正匹配后：**`web_api` 下 48 个 .rs，已记 40，未审 8，合计 2472 行**，
而这 8 个**全部是测试文件**：
`voice_routes_tests.rs`(793) · `voice_routes_tests_gate.rs`(209) · `voice_routes_tests_say.rs`(148) ·
`voice_routes_tests_token.rs`(58) · `models_routes/tests_models_p1.rs`(534) ·
`tests_models_core.rs`(320) · `tests_models_handlers.rs`(279) · `tests_models_common.rs`(131)
⇒ **`web_api` 的 40 个生产文件已全部审过**；剩下的只有测试面。
⇒ 而测试面**正是两条假绿发现（B0001-02 的 `assert!`、B0002-02 的测试体脱节）所在的层**
⇒ 补这一层的价值**高于**再开新目录。

## ② 本批核 `tests_models_p1.rs`(534)：**13 个用例里是成组的三向对照**
```
p1_3_validate_unit_rejects_bad_values        p1_3_dispatch_rejects_invalid_params
p1_3_dispatch_accepts_valid_hex_colors      ← 「接受合法值」**让前两条有意义**
p1_4_registry_round_trip_uses_relative_paths  ← **round-trip**（正是 B0104 抓到「起点为空」那类假绿的正确形状）
p1_4_handle_import_stores_relative_paths
p1_4_absolute_path_derived_per_call_assets_root_churn
p1_5_from_disk_backs_up_corrupt_registry
p1_5_from_disk_missing_file_does_not_backup  ← **「不备份」的反向对照**（否则「永远备份」也能过）
p1_5_backup_filename_has_unix_secs_suffix
a1_nested_layout_imports_and_activates_with_a_getable_url   ← 端到端 + URL 可取
a1_activate_reports_hot_swap_not_restart      ← 把 API 的**声明**钉成断言
a1_active_model_id_follows_the_registry
```
⇒ **0 条新发现。** 而这批的**方法论收获**比结论重要：
> ⭐ **正面模式 P7：每个「会做 X」的断言，都配一个「不会做 X」的对照。**
> 本文件三处成组：`拒绝坏值` + `接受合法值`（否则「全拒」也能过）·
> `坏档要备份` + `缺档不备份`（否则「永远备份」也能过）· 另有 `round-trip`（写入再读回）而非单点。
⇒ **这正是本审计在 B0001/B0002 抓到的那两处假绿所缺的东西** ——
那两处不是「没有测试」，而是「**测试的对照面缺失**」，所以断言可以恒真。

## ⚠ ③ 记一次我自己的机械错误（本轮第三次）
今天第三次：**自己的机械检查逻辑写错**（前两次：B0130 的 `head` 截断、B0129 的假阳性）。
这次若未警觉，会把覆盖率记成「`web_api` 未审 44/48」——**比真实值差 5 倍**，
并据此错误地安排下一批。
⇒ **执行要求（补进模式 L）**：
> **凡是用脚本算覆盖率/差集，脚本本身要先用一个已知答案自检**
> （如「我确知这 40 个审过」⇒ 输出必须显示 ≤8）。
> **脚本输出与我的既有认知冲突时，先怀疑脚本。**

## 未核实项
1. 其余 7 个测试文件未读（1938 行）
2. `web_api` 的**生产文件虽已记 40，但其中多为定点读**（不是逐行）—— 「审过」的含义是「按轴核过」，
   **不等于**「逐行读过」⇒ **如实标注，不夸大覆盖率**
3. `live2d-ai-runtime` 余面（`client.rs` 652 / `config.rs` / `sse.rs` 余段）未读
4. `performance/plan.rs` 余 700 行未读
5. `_finishTurn` 是否幂等 · `secrets.rs` 10 条测试断言体未读 · B0120「chmod toml」未核
