# BATCH-0610 · 「确保」= **缺失才构建**（判据是 `dist/index.html` 在不在），而注释**说了 trunk 要联网**

Phase 4 · 证伪（兑现 B0609 留的：`ignite.sh` 的「确保 WASM dist」是否含 `trunk build`）

## 跑的命令（全部只读）
```
grep -n "trunk|确保|dist" scripts/ignite.sh | head -8
```
未跑脚本（**运行脚本不在允许清单内**）。

## 逐行
```
:6   #   2) 确保渲染面产物：crates/l2d-wasm-demo/dist（/render 用）；
:7   #   3) 确保前端产物：shell/flutter/build/web（/app/ 用，可选 --build 自动构建）
:164 if [ ! -f "crates/l2d-wasm-demo/dist/index.html" ]; then
:165   echo "==> 渲染面产物缺失，正在用 trunk 构建 l2d-wasm-demo …"
:166   # trunk 会先跑 `cargo metadata`（需网络解析全平台依赖）；
:168   (cd crates/l2d-wasm-demo && unset NO_COLOR && if [ -n "${https_proxy:-}" ]; then exp…
```

## 五个可核点
1. ⭐⭐⭐⭐⭐⭐ **⇒ 判据是 `dist/index.html` 在不在**（不是 `dist/` 在不在）⇒⇒ **⇒ 与 B0447 核的「防御不在路过的对象上、防御在构造它的那个动作上」**不同族、
2. ⭐⭐⭐⭐⭐ **⇒ 所以「确保」= 缺失才构建**，而**B0609 那条待核结清** ⇒⇒ **⇒ `ignite.sh` 会 `trunk build`** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
   ⇒⇒ **⇒ 这与 B0407 核的「两个有网时看不出来的缺陷」（断网豆腐块 / 断网白屏）**是同一类**：
   **「有网时一切正常、没网时这一步才失败」** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
   ⇒⇒ **⇒ 与 B0608 核的 `run-web.sh:15 unset NO_COLOR` 是同一件事的第二处** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐

## 结论
- **B0609 第 2 点的待核结清**：`ignite.sh` **会** `trunk build`，**判据是 `dist/index.html` 缺失**。
- ⇒⇒ **B0609 的 P3 观察因此要再收窄**：`run-web.sh` **不构建 Flutter Web**（那一条仍成立），
  **但它构建 WASM 这一点与 `ignite.sh` 重复**（`ignite.sh` 缺失才构建、它无条件构建）⇒⇒
  **⇒ 两个脚本在 WASM 这一步的策略不同**：**「缺失才建」vs「每次都建」** ⇒⇒ **记为观察**（无用户后果：
  后果只是慢一点；而 `ignite.sh` 更快）。

## 未核
`ignite.sh` 本体其余 ~200 行（`--check` 四项断言 / toml 预检 / URL 打印）·
`ignition-precheck.sh` 本体（402 行只读 10 行）· `verify_edge_alpha.py` 本体（177 行只读 14 行）·
`diag-stale.sh` 后续 32 行 · `adb_ui_tap.py` / `deploy_android.sh` / `setup_linux.sh`。
