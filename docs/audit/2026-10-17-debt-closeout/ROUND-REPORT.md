# 2026-10-17 0.2.6 后债务收口轮 · 轮次报告

> 起点：b957775（origin 767a286）。本轮结束：44dc417（+ 本报告提交）。
> 功能冻结：不升版本、不打新 tag、不复活 0.3.0；不碰已删 Mod、action_tx、3.0 功能。
> 口径：绿灯都给原始命令与数字；没跑 / 没验的单独列，不写成已绿。

## 0. 一句话

五项收完：D1 真 runner 绿（维护者手动 dispatch）、根 pytest 16 红清零、无 supervisor 时手改坏 live2d-ai.toml 会在界面出错误横幅、wasm 体积按 D8 停、发布说明与收口计划 §6 不再把已做完写成未做。裁定留下的 6–11 一行未动。

## 1. 范围与起点

- 起点 HEAD b957775，origin/main 767a286（本地领先 2 个提交）。
- 范围：维护者点名的 5 项；3.0 功能、已删 Mod、action_tx 不在本轮。
- 四条规定：tests_reload 棘轮、只改 StatusContext::refresh_from_disk、错误帧条件、core-contracts 两处 + 不动验收文档/FINDINGS.md。

## 2. 分工（2 名 teammate + Lead，共享同一工作树）

| 成员 | 任务 | 写入面 | 结果 |
| --- | --- | --- | --- |
| pytest-cleaner | task-1 | tests/、docs/architecture/core-contracts.md | 删过期 persona 测试；pytest 9 passed / 0 skipped |
| config-error-dev | task-2 | live2d-ai-desktop 7 文件 | S-007 无 supervisor 分支 + 3 条新回归 |
| lead | task-3 + 终局 | 发布说明 / 收口计划 / 提交 / 门禁 | D1 推送与 Nightly、文档、全量门禁 |

所有 teammate 均未做 git 写操作；提交、推送、tag 由 Lead 独占。团队任务 task-1 / task-2 / task-3 均 completed。

## 3. 逐项交付

### 3.1 D1：推送 + Nightly

- `git push origin main`：767a286..b957775（快进），origin/main = b957775。
- `gh workflow run nightly.yml --ref main` 被 fine-grained PAT 挡住：HTTP 403 `Resource not accessible by personal access token`。
- 维护者手动在 Actions 页面 Run workflow（main，b957775）。
- Run：<https://github.com/yuhanwanli99-cpu/Live2D-Ai/actions/runs/39025484355>

| Job | 结果 |
| --- | --- |
| Rust Full Test（D1） | **success**（Ensure ALSA dev headers → Build / Test / doc / fmt / clippy / rust-ratio 全过） |
| Web Offline Serve Check | success |
| Rust Release Build | success |
| Core Chain Verify | skipped（仓库变量未配，按设计跳过） |

- headSha = b957775，event = workflow_dispatch，10:35:32Z → 10:40:13Z，约 4m41s，conclusion = success。

### 3.2 根 pytest 16 红

- 删除 `tests/test_persona_shared.py`（186 行，整份只测已删除的 `shared/persona.yaml`）。
- `tests/test_emotion_protocol.py`：删 C-p2 / `persona_path` / `import yaml`；C22a/C22b 保留。
- `docs/architecture/core-contracts.md`：§6 资产表 persona.yaml 行改为「已删除（d69d45f / 2026-10-13 出厂不设人设）」；§7 计数改为 9 passed / 0 skipped，并写明 16 条红 + 1 条 skip 随文件删除。
- 验证：`python3 -m pytest tests/ -q` → **9 passed**；`grep -rn 'shared/persona.yaml' tests/` 无命中。
- 清理了被 gitignore 的 `tests/__pycache__`（否则 grep 会命中旧 docstring 的 .pyc）；未入库。

### 3.3 S-007：无 supervisor 时手改坏 toml 有界面提示

**根因**：`file_watcher` 的 `PostReloadHook` 是 `Fn()`；`StatusContext::refresh_from_disk` 解析失败只 warn + false；只有 supervisor 的 `apply_reload` 失败会 emit error 帧。无 supervisor 时这条失败没有任何出口。

**实现（7 文件，未新增 WS 帧类型、未改 Dart）**：
- `StatusContext::refresh_from_disk` → `Result<(), String>`；失败保留旧快照并返回解析原文。
- `PostReloadHook = Box<dyn Fn(bool) + Send>`，bool = 本次事件时是否有 supervisor；`on_config_changed` 传真实值。
- `cli_entry::config_watch_post_reload`：**仅当** toml 解析失败且 had_supervisor=false 时，用既有 `app_event_to_ws_frame` 投影并 broadcast；有 supervisor 只留 debug，失败仍由 `apply_reload` 发。
- `AppErrorEvent::config_reload_failed(message)` 作为唯一五字段构造点，`supervisor/reload.rs` 复用。
- `secrets::refresh_from_disk` / `secrets.rs` 一行未动；`.env` 不扩进本片。

**测试**：
- `config_watch_without_supervisor_broadcasts_one_error_frame`：坏 toml + 无 supervisor → 恰好一条 error 帧，code=config_reload_failed / stage=config / epoch=0 / fatal=false / hint 与 supervisor 分支逐字一致；旧快照保留。
- `config_watch_with_supervisor_does_not_broadcast`：有 supervisor 时钩子不广播（去重）。
- `config_watch_with_valid_toml_broadcasts_nothing`：合法 toml 不广播。
- `live2d-ai-desktop`：**545 passed, 0 failed**。

**真服务端到端**：临时目录起服务（无 supervisor、Mod 全关），裸 WebSocket 带 Origin 订阅 `/ws/runtime`，手改坏 `live2d-ai.toml`，收到：

~~~
{"type":"error","data":{"code":"config_reload_failed","stage":"config","epoch":0,"fatal":false}}
~~~

- Flutter 定向测试 `ws_frame_test.dart` + `ui_state_tracker_test.dart`：**78 passed**（error 帧解析 + 错误态）。
- **未做浏览器肉眼截图**：本机没有 Chrome/Chromium；只有服务端 WS + Flutter 单测两层证据，不写成已经看过界面。

### 3.4 wasm 体积

- `wasm-opt`：**不存在**。
- `crates/l2d-wasm-demo/dist`：wasm **5,495,511 B = 5.24 MiB** > 接受值 **4.2 MiB**（`WASM_DIST_BUDGET_MIB = Mib(4.2)`）。
- 按 D8 停：不改 4.2、不改预设/功能、不引入 wasm 工具链。

### 3.5 发布说明与收口计划 §6

- `docs/releases/v0.2.6-rc.1.md`：保留发布时债务清单，加 2026-10-17 补记——拆文件 / AGENTS.md 减肥 / unwrap 清点 / S-000–S-007 已清（S-007 无 supervisor 分支同日补齐）；仍开：CI 真 runner、wasm 体积、开发者调试面板。
- `docs/plans/PLAN-0.2-closeout-2026-10-13.md` §6：已完成项逐条标注并给 `74b1ecb` / 清点文件证据；保留历史条目，不改 v0.2.5 历史句。
- docs 预算：**44,957 / 45,000 PASS**（余 43 行）。

## 4. 提交与推送

| 提交 | 内容 | 状态 |
| --- | --- | --- |
| 74b1ecb | 清 0.2.6 后静默错误 S-000–S-007 + 拆 supervisor/engine（棘轮 42）+ ALSA 探测 + unwrap 清点 | 已推送 |
| b957775 | 共享锁毒化不再打死请求路径（W1，7 处生产锁） | 已推送 |
| 3af8e05 | test: 清掉 shared/persona.yaml 删除后的 16 条红 | 已推送 |
| 359f786 | fix: 无 supervisor 时手改坏 toml 也把错误送上界面（S-007 补齐） | 已推送 |
| 44dc417 | docs: 收口文档更新已完成的债务项 | 已推送 |
| 本报告提交 | docs/audit/2026-10-17-debt-closeout/ROUND-REPORT.md | 随本轮推送 |

- 推送 `b957775..44dc417` 后：`Build & Upload Artifact` **success**（<https://github.com/yuhanwanli99-cpu/Live2D-Ai/actions/runs/39025878506>）；`Secret Scan` **success**（<https://github.com/yuhanwanli99-cpu/Live2D-Ai/actions/runs/39025878505>）。
- 本报告提交只改 `docs/audit/**`；Secret Scan 没有 paths 过滤会再跑一次，Build & Upload 因 `docs/**` paths-ignore 不会重跑。
- 未升版本、未打新 tag、未移动 v0.2.6-rc.1 / v0.2.5-rc.1。

## 5. 门禁（在 HEAD 44dc417 上真实跑过）

| 命令 | 结果 |
| --- | --- |
| `cargo test --workspace --all-targets` | exit 0（live2d-ai-desktop 545 passed） |
| `cargo test --doc --workspace` | exit 0（3 doctest） |
| `cargo fmt --all -- --check` | exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0 |
| `cargo run -p xtask -- rust-ratio` | PASS |
| `cargo run -p xtask -- code-stats --check --only lines` | PASS；>500 = 42（棘轮未动） |
| `cargo run -p xtask -- code-stats --check --only docs` | PASS；44,957 / 45,000 |
| `python3 scripts/check_public_secrets.py` | ok（2129 files） |
| `python3 -m pytest tests/ -q` | 9 passed |
| Flutter 定向测试（ws_frame + ui_state_tracker） | 78 passed |

## 6. 裁定留下（本轮未动）

| # | 位置 | 处理 |
| --- | --- | --- |
| 6 | `shell/flutter/lib/app/app_shortcuts.dart` 快捷键帮助仍写「停止本轮」 | 留 |
| 7 | `docs/examples/bilibili-sidecar/` 不追仓外脚本 | 留 |
| 8 | 主动点头 | 不做 |
| 9 | 本机 `live2d-ai.toml` 的 `[tts].channels = 2`（example 仍 1） | 不改 |
| 10 | `docs/design/web-ui-spec-v3.md` 旧枚举表不同步 | 留 |
| 11 | `docs/audit/2026-10-09-silent-errors/FINDINGS.md` 正文 | 不改 |

## 7. 未做 / 边界 / 需要知道

- **浏览器肉眼截图没做**：本机无 Chrome/Chromium；item 3 的证据是真实 WS 帧 + Flutter 现有错误帧测试，不冒充「看过界面」。
- **Nightly 只跑了 b957775（D1）**；44dc417 只跑了 push 触发的 Build & Upload + Secret Scan，两者均绿。若要对新 HEAD 再跑 Nightly，需要再手动 dispatch 一次（PAT 仍无 Actions: Write）。
- `docs/architecture/core-contracts.md:115` 还有一条历史摘要写「待裁决：shared/persona.yaml 的权威地位」；按维护者「只改第 91 行 + 计数句」的口径未动。
- 工作树全程未改 `live2d-ai.toml`、`.env`、`mods.json`；未复活 action_tx / 已删 Mod。

## 8. 真源

- 本轮报告：本文件。
- 债务范围：`docs/plans/PLAN-debt-after-0.2.6-2026-10-16.md`。
- 收口边界：`docs/plans/PLAN-0.2-closeout-2026-10-13.md` §6。
- unwrap 清点：`docs/audit/2026-10-16-unwrap-inventory.md`。
- S-000–S-007 原台账：`docs/audit/2026-10-09-silent-errors/FINDINGS.md`（正文未改）。

