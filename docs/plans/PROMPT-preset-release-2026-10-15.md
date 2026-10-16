# 执行单：表情进出有起落，撤销从屏幕上的姿势收回去（2026-10-15）

> **本单已执行，不要再跑一遍。** 2026-10-16 验收通过。`clear_all_overrides` 删除、`handle` 的 sink 改名 `_sink` 是接受的偏差。

> 交给工作 agent。本单就是范围。不要再调研，不要扩大。
> 树：`/home/skystar/Live2D-Ai`，分支 `main`。已有未提交改动留着，不要还原，不要另开分支。
> 不要改 `.env`、`mods.json`、`live2d-ai.toml`。不要改仓外脚本，不要改 `docs/examples/bilibili-sidecar/`。
> 不要 commit、tag、push，不要改版本号，不要提高 `DOCS_BUDGET_LINES`。
> 不要动 `supervisor.rs`、导演 Mod、`presets.json`、`field_runtime`、待机呼吸 / 眨眼、`BODY_LAG`、`ATTACK_END`、`HOLD_END`。
> 不要改口型，不要写 `ParamMouthOpenY`。不要复活 `action_tx`。不要新增 WS 帧。
> 不要给空 cue 补点头，不要改规则表，不要改按句清洗。
> 同槽换预设的接力（`handoff.rs`）留着。用户名星号不讨论。
> [`PROMPT-preset-handoff-and-one-round-button-2026-10-15.md`](PROMPT-preset-handoff-and-one-round-button-2026-10-15.md) 不要再执行。

直播可能还在。编完 wasm 即可。不要杀、不要重启 18080，不要跑 `ignite.sh`，不要刷新维护者的页面。

## 0. 现在断在哪

按句清洗发的是 `preset_id`，舞台走 `PresetRuntime`，不走 `field_runtime`。

同槽换**不同 id** 已经在攻击段里从旧帧混到新帧。手势自己有攻击、保持、放回，进度 1 的增益是 0，到点清槽时值已经是 0。

还是一刀切的有两处：

- 表情 `pack_gain` 整段是 1。空槽第一次出现就是满的，时间到的那一帧整批清掉。
- `preset_id == "none"`（`Revoke`）在这一帧 `clear_all_overrides`，两个槽立刻归零。接力窗口内再换一次时，快照用的是旧预设自己的曲线，不是屏幕上正在混合的值。

## 1. 做完是什么样

1. 表情的增益改用现成 `motion_curve::curve`。进度 0 和进度 1 恰好是 0，`ATTACK_END` 到 `HOLD_END` 恰好是 1。进场、保持、离场和手势同一条曲线。到点仍立即清槽：那一帧值已经是 0，不要在到期后再加一段放回。
2. `Revoke` 不在当帧清参数。每个还有姿势的槽，从**这一帧屏幕上的值**收到 0。时长 = 该槽 `ttl_ms * (1.0 - HOLD_END)`，权重用现成 `smootherstep`。到点再清槽。
3. 槽上还有姿势时（含放回途中、含接力途中），换到另一个 id 的快照必须是屏幕上这一帧的值。窗口内再换一次，新预设第一帧等于换之前屏幕上的混合值，不是旧预设未混合的曲线。

空槽第一次出手势：进度 0 仍是 0。同一 id 再施加：仍从 0 重开，不接力。两个槽互不混合。手势曲线、身体滞后不改。

## 2. 放在哪

`crates/l2d-wasm-demo/src/preset/mod.rs` 头注写 995 行，上限 1000。`pack_gain` 里表情那一支从 `1.0` 改成 `motion_curve::curve(progress) as f32`，行数不要涨。必须改的注释改短，不要加段落。改完把头注行数改成实际行数，仍 ≤1000。

放回状态机放新文件 `preset/release.rs`（≤500）。`Revoke` 和每帧放回只在 `mod.rs` 里调用。测试放 `preset/tests/`，由 `tests/mod.rs` 引入，不要写进 `mod.rs`。

`Handoff::capture` 记下屏幕值：正在接力就用这一帧的混合结果，正在放回就用放回写到一半的值，否则才用 `frame_params_scaled_by`。

`peak_gain` 里表情的上确界仍是 1（保持段）。注释不要再写「整段恒 1」。`motion_curve.rs` 头注里「表情恒 1」改成一句「表情增益走同一条 curve」。

## 3. 必须有的断言

- 表情进度 0：写出的参数全是 0。进度在保持段内：等于不乘曲线时的满值。进度 1：全是 0。到点清槽后槽为空。
- 手势进度 0 仍是 0。保持段中途换成另一个手势：共享通道第一帧仍等于换之前的值。
- 保持段中途 `Revoke`：当帧参数还在，等于撤销前屏幕上的值。经过 `ttl_ms * (1.0 - HOLD_END)` 之后槽为空、override 已清。
- 放回到一半时换另一个 id：新预设第一帧等于这一半的屏幕值，不是 0，也不是旧预设未混合的曲线。
- 接力窗口内再换第三次：第一帧等于第二次混合到一半的屏幕值。
- 同一 id 再施加：不接力，进度 0 是 0。

`handoff.rs` 里「到点 / Revoke 立即清」那条测试拆开：到点仍立即清；`Revoke` 改成上面的放回。

## 4. 验收和交付

代理先卸掉：`http_proxy` `https_proxy` `HTTP_PROXY` `HTTPS_PROXY` `ALL_PROXY` `all_proxy`。trunk 要拉依赖时，只把已经存在的 `https_proxy` 抄进 `CARGO_HTTP_PROXY`，不改 `.env`。

```bash
env -u http_proxy -u https_proxy -u HTTP_PROXY -u HTTPS_PROXY -u ALL_PROXY -u all_proxy \
  cargo test -p l2d-wasm-demo
env -u http_proxy -u https_proxy -u HTTP_PROXY -u HTTPS_PROXY -u ALL_PROXY -u all_proxy \
  cargo clippy -p l2d-wasm-demo --all-targets -- -D warnings
cargo fmt --all -- --check
(cd crates/l2d-wasm-demo && unset NO_COLOR && trunk build)
```

不要编 Flutter，不要 `cargo build -p live2d-ai-desktop`，不要全仓测试。不要点火。

回报写明：改了哪些文件、`mod.rs` 改完的行数、上面每条命令的退出码、18080 未重启。构建失败就贴命令和最后几行。

## 5. 明确不做

不改 `field_runtime` 的到点移除。不改 cue 的 2000ms，不改预设表的时长。不把放回做成第二次到期后的尾巴。不改桌面二进制。
