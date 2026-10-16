# 执行单：换槽从当前姿势接上 + 停止与发送合成一颗圆键（2026-10-15）

> **本单已执行，不要再跑一遍。** 「表情整段增益 1」和「到点 / Revoke 立即清槽」由
> [`PROMPT-preset-release-2026-10-15.md`](PROMPT-preset-release-2026-10-15.md) 取代。同槽接力留着。

> 交给工作 agent。本单就是范围。不要再调研，不要再拆一轮。
> 树：`/home/skystar/Live2D-Ai`，分支 `main`。已有未提交改动留着，不要还原，不要另开分支。
> 不要改 `.env`、`mods.json`、`live2d-ai.toml`。不要改仓外脚本，不要改 `docs/examples/bilibili-sidecar/`。
> 不要 commit、tag、push，不要改版本号，不要提高 `DOCS_BUDGET_LINES`。
> 不要动 `supervisor.rs`（1000 行顶上）、导演 Mod、`presets.json`、`BODY_LAG`、`ATTACK_END`、`HOLD_END`。
> 不要把表情保持段的增益从 1 改掉。不要复活 `action_tx`。不要新增 WS 帧。
> 用户名星号已另行处理，本单不讨论、不改注入格式。

## 0. 两件事

1. 同一槽换到下一个动作时，从换下去那一帧的参数接到新动作，不要先归零。
2. 输入区只留一颗圆键：没东西可停时发送，有东西可停时变成停止。删掉旁边那颗独立「停止」。

做完自己构建前端、wasm 渲染面和桌面二进制。不要把构建命令交回维护者。不要启停 18080。

## 1. 换槽接上

真源：`crates/l2d-wasm-demo/src/preset/mod.rs`（974 行，豁免上限 1000，头注仍写 902，改这个文件时把头注行数改成改完后的实际行数，豁免理由不改写）。

硬切在 `PresetRuntime::handle` 的 `Apply`：目标槽上一条 id 不同时立刻 `clear_spec_overrides`，新预设 `started_ms = now`，进度从 0 再攻击。表情包 `pack_gain` 整段是 1，到点也是直接清。

只改「同一槽、不同 id」的替换。空槽第一次施加、同一 id 再施加、到点撤销、`Revoke`，都保持现在的行为。表情槽和手势槽互不混合。

- 替换前用当时的进度算出旧预设本帧要写的参数，记在新活动预设上。不要在这一步 `clear_spec_overrides`。
- 新预设进度 `< ATTACK_END`（0.22，用现成常量）时，这一帧写插值，不写「从 0 攻击」的原帧。权重用现成 `smootherstep(p / ATTACK_END)`。
  - 两边都有的通道：旧值 → 新预设在这一进度的正常帧值。
  - 只有旧预设有的通道：旧值 → 0；进度到 `ATTACK_END` 时 `clear` 这些不再写的通道。
  - 只有新预设有的通道：0 → 它在这一进度的正常帧值。
- 进度 `>= ATTACK_END` 之后丢掉记下的旧值，帧值和现在完全一样。手势仍走现有攻击 / 保持 / 放回和身体滞后。表情保持段仍是增益 1。
- 表情换到下一个表情时用同一段窗口。表情到期回到待机仍是到点立即撤销，不做放回曲线。
- 不写 `ParamMouthOpenY`。不改 `field_runtime`。待机摆头让开保持原样。

插值函数放新文件（建议 `crates/l2d-wasm-demo/src/preset/handoff.rs`，≤500 行）。`mod.rs` 只调用它，改完仍 ≤1000。回归放 `preset/tests/` 下的新文件，由 `tests/mod.rs` 引入；不要把测试写进 `mod.rs`。

必须有的断言：

- 手势保持段中途换成另一个手势：新预设第一帧，共享通道仍等于换之前的旧值，不是 0。
- 进度到达 `ATTACK_END`：共享通道等于不接旧值时的正常帧；只属于旧预设的通道已被清掉。
- 表情换表情：进度 0 等于旧表情值；进度 ≥ `ATTACK_END` 等于新表情满增益。
- 空槽第一次施加手势：进度 0 的增益仍是 0。
- 到点撤销和 `Revoke` 仍是立即清槽。

`cargo test -p l2d-wasm-demo` 与 `cargo clippy -p l2d-wasm-demo --all-targets -- -D warnings` 必须过。`cargo fmt --all -- --check` 必须过。

渲染面按请求读 `crates/l2d-wasm-demo/dist`。改完后在该目录执行与 `scripts/ignite.sh` 相同的 trunk 构建（需要代理时只把已有的 `https_proxy` 抄进 `CARGO_HTTP_PROXY`，不改 `.env`）。不要因为 dist 已存在就跳过。

## 2. 一颗圆键

现状：`chat_panel.dart` 输入行里，文本框右边是常驻 `_StopPlaybackButton`（「停止」），再右边圆键在思考中 / 说话中变成「停止本轮」。窄栏里两颗挤在一起，生成时也是两个停止。

`canStopPlayback` / `stopPlayback`（`stage_cancel.dart`）不要改语义：永远先取消舞台和待播，仅当这一轮还在飞才 `ChatController.stop()`。

圆键改成：

- `canStopPlayback` 为假：向上箭头，提示「发送（Enter）」，调用 `onSend`。
- 为真：`Icons.stop_rounded`，提示「停止」，调用现有 `onStopPlayback`（`_stopPlayback`）。思考中、说话中、只剩待播或待演，都是这一种停止，不再用「停止本轮」作第二颗。
- 删掉 `_StopPlaybackButton` 和它在输入行的位置。不要另做一颗停止。

Esc 与这颗圆键走同一条 `_stopPlayback`。只有 `canStopPlayback` 为真时 Esc 才调用；为假时 Esc 仍什么都不做。「打断并重发」继续用 `_stopWithCancellation`，不要改成空闲也可按的那条。

`ChatPanel.onStop` 若只被旧圆键使用，从面板去掉，避免再接出第二颗停止。Esc 的接线留在外壳。

测试：改 `shell/flutter/test/stop_playback_button_test.dart`。空闲且 `canStopPlayback == false` 时圆键是发送；`canStopPlayback == true` 时同一颗圆键是「停止」，按下只走 `onStopPlayback`，输入行不再有第二颗「停止」。`visual_language_test.dart` 仍只认圆键 `IconButton`。`stage_cancel_test.dart` 的编排断言保持绿。`flutter analyze` 与 `flutter test` 必须过。

## 3. 交付

仓库根执行。`flutter` 用 `$HOME/flutter/bin`。跑 flutter / `cargo test` / `cargo build` 前先 unset `http_proxy` `https_proxy` `HTTP_PROXY` `HTTPS_PROXY` `ALL_PROXY` `all_proxy`。trunk 要拉依赖时按第 1 节单独处理代理。

```bash
export PATH="$HOME/flutter/bin:$PATH"
(cd crates/l2d-wasm-demo && unset NO_COLOR && trunk build)
(cd shell/flutter && flutter build web --release --base-href /app/ --no-web-resources-cdn)
./scripts/prune_web_artifacts.sh
./scripts/prune_web_artifacts.sh --check
cargo build -p live2d-ai-desktop
```

本单不改桌面 crate 的源码。wasm 和前端都是服务按请求读盘。不要杀、不要重启 18080，不要跑 `ignite.sh`。

完工报告单独写明：trunk、flutter 产物、prune `--check`、`cargo build -p live2d-ai-desktop` 的退出码，以及 18080 未重启。构建失败就贴失败命令和最后几行，不要假装交付完成。
