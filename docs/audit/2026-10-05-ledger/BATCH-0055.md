# BATCH-0055 · ⭐ 红线 N 正向验证（结清最后一条红线）

Phase 1 · 域覆盖 → 渲染面

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/l2d-wasm-demo/src/main.rs` — 962（定点 318-379：init 尾部时序 + load_summary + install_stage_bridge 头注）
2. 跨端对照（已核）：`flutter/lib/live2d/live2d_stage.dart:120-123`（B0044 已读）·
   `flutter/lib/app/app_shell.dart:1207`（B0030 已读）

## 跑过的命令（全部只读）
```
grep -rn "emit_event(\s*\"loaded\"|\"loaded\"|stage-ack|first_frame|首帧" crates/l2d-wasm-demo/src/
sed -n '318,379p' crates/l2d-wasm-demo/src/main.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## ★ 本批产出
**0 条新发现**；**红线 N（舞台保活）正向验证通过**，两条时序保证都核到源码：
1. 渲染面：`ready` **发在 listener 装好之后**（:338→:341），rAF 循环最后才开（:343）
   ⇒ 宿主见 `ready` 即可下发，无「消息早于监听器」的竞态
2. 宿主：`_attach` **每次挂桥无条件重发** `stageImage`（B0030 已核）· `onReady → _applyPrefs`
3. 换模成功/失败分别发 `loaded` / `error`（:670 / :674），失败**不**伪装成成功
⇒ **13 条红线全部结清**（台账见 STATE §7b/7d/7e）。

## 未核实项
1. `web/surface/render.rs`(551) / `idle.rs`(270) **仍未读** —— `surface::tick` 的实体
2. `crates/l2d/**` 其余 15 文件（`model.rs` / `renderer/mod.rs` / `asset/mod.rs` …）未读
3. `l2d-wasm-demo/src/preset/*`(2800) 未读
4. **待验的红线 K 附加问题**：`idle.rs` 的待机驱动与 `pose_stack` 的 idle 层
   是否真是 AGENTS.md 说的「两套机制」（那条「删动作时绝不要连带删它」的边界）——本批未核
5. 红线 N 只做了**时序与重发**的核验；**重挂时的纹理/GPU 资源释放**未核（需读 `l2d` 的
   `ModelHandle`/drop 路径）

## 本批新增
**0 条**（净产出：红线 N 结清，13/13 全部判定）
