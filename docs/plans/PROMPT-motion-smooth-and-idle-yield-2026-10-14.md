# 执行单：动作顺、待机让开头（2026-10-14）

> 交给工作 agent。本单就是范围。不要再调研，不要扩大。
> 树：`/home/skystar/Live2D-Ai`，分支 `main`。工作区里已有未提交改动，留着，不要还原，不要另开分支。
> 不要改 `.env`、`mods.json`、`live2d-ai.toml`。不要点火，不要启停 18080。
> 不要 commit、不要 tag、不要 push、不要改版本号。不要提高 `DOCS_BUDGET_LINES`。
> 不要改 `CHANGELOG.md`、`docs/releases/**`、`docs/audit/**`、`docs/legacy/**`、`docs/architecture/CURRENT.md`。
> 不要改 Flutter、导演 Mod、`supervisor.rs`、`presets.json` 的 id / 幅值 / 时长。

## 0. 维护者当轮裁决

动画发僵。表情不用处理。这一单只做两件事：手势（`kind=motion`）的起落改顺；待机小动作里那条摆头，在手势占槽时让开，结束后再缓回来。

不要新做情绪模型，不要随机表情，不要跨轮常驻，不要把 `action_tx` 接回去，不要加预设，不要改表演倾向，不要改 cue 的 `ttl_ms`（仍是 2000）。

## 1. 现在为什么僵、为什么抢

两处都在源码里，按这个改，不要另找原因。

- 手势包络是 `sin(πp)`，头和身用同一个 `p`。`motion_gain(Single)` 被断言成与 `envelope` 逐点相等（`preset/tests/mod.rs` 约 385 行）。看起来就是一下戳出去再收回，身没有落后。
- 待机层每帧写 `ParamAngleZ = sin * 3°`（`crates/l2d/src/pose_stack.rs` 的 `update`）。`nod` / `look_up` / `look_down` **不写** Z，摆头就叠在点头下面。`shake` / `look_*` / `tilt_*` 用 `final_override` 盖住 Z，但槽一撤，摆头按自己的相位立刻回来。
- 表情包 `pack_gain` 恒为 1，到点整段撤销。这一单保持这样。（2026-10-15 已由 [`PROMPT-preset-release-2026-10-15.md`](PROMPT-preset-release-2026-10-15.md) 取代，不要按这句改回去。）
- 帧序不要换：`render.rs` 先 `core.update`，再 `apply_bridge_effects`。新 cue 晚一帧进物理是既有行为。

呼吸、眨眼、微表情（`apply_idle_life`）不改。`field_runtime` 不改。导演仍只发 `preset_id`。

## 2. 手势曲线

新建 `crates/l2d-wasm-demo/src/preset/motion_curve.rs`（≤200 行）。`preset/mod.rs` 现在 959 行，**不得超过 1000**。曲线放新文件，`mod.rs` 只改调用。

只用于 `PresetKind::Motion`：

- 进度 `p ∈ [0,1]`。攻击到 `0.22`，保持到 `0.62`，再到 `1` 放回。用 smootherstep `t³(t(6t-15)+10)`。`p=0` 与 `p=1` 必须仍是 0，两端斜率是 0。保持段的值是 1。
- `Single` 的幅值乘子就是这条曲线，不再等于 `envelope(p)`。`envelope()` 函数留着，旧的两端为 0 的测试继续测它。
- `Oscillate` 仍是「这条曲线 × `sin(2π·cycles·p)`」，首末仍是 0，`|gain| ≤ 1`。NaN cycles 仍回落缺省。
- 身体滞后：`ParamBodyAngleX/Y/Z` 用 `body_p = ((p - 0.12) / 0.88).clamp(0, 1)` 再套同一条曲线（摇头再乘身体自己的正弦）。头和其它通道用 `p`。`p=0` 与 `p=1` 时身体也是 0，避免槽到期时把尾巴剪断。
- 表内幅值、`head_scale` / `body_scale`、通道钳位都不动。表情通道仍乘 1。

测试改判据，不要放宽成「差不多」：

- `expression_holds_while_motion_follows_the_envelope`：表情两个进度仍同值。`nod` 在 0 和 1 仍是 0；在 `[0.22, 0.62]` 内 `|ParamAngleY|` 为峰值。
- `motion_gain_falls_back_and_stays_bounded`：删掉「Single 逐点等于 `envelope`」。改成 Single 在保持段为 1、两端为 0、全程 ≥ 0。
- `shake_oscillates_both_ways_with_body_in_phase`：左右变号仍 ≥ 2，峰值仍 ≥ 10。身体不再要求每一采样都同相。峰值附近（头的绝对值 > 峰值的一半）仍同号，身/头仍在 `0.25..=0.55`。靠近头的过零点允许身体短暂反号。
- `nod_and_look_carry_body_follow` 看的是表内符号和比例，表不改，这条应仍绿。

## 3. 待机摆头让开

只让 `PoseStack` 的 `ParamAngleZ` 摆头。呼吸正弦照写。`idle_enabled=false` 时整层仍停，现有回归保持。

- `PoseStack` 增加「目标抑制」`0` 或 `1`。`update` 里用本帧 `dt` 把当前抑制追向目标：`0→1` 用 200ms，`1→0` 用 350ms。写入的摆头 = 原来的 `sin * 3°` × `(1 - 当前抑制)`。不要重置 `elapsed`，松手后相位接着走。
- `PresetRuntime` 增加 `gesture_occupied() -> bool`：手势槽里有活动预设才为真。表情槽不算。
- `apply_bridge_effects` 在 `apply_frame` 之后，把这个布尔写成目标抑制（`core.stack()` 已能拿到栈）。本帧的 `update` 已经跑过，抑制从下一帧开始追。不要为了「当帧生效」去调换 `update` 和 `apply_bridge_effects`。
- `final_override` 仍压过 idle。`final_override_angle_z_reaches_physics_and_outranks_idle` 保持绿。
- 新回归两条，放在 `pose_stack.rs`（现 649 行，改完 ≤1000）：抑制追到 1 之后，连续几帧 `ParamAngleZ` 保持默认，呼吸仍离开默认；目标回到 0 后，摆头在 350ms 内离开默认，且与「从未抑制、同一 `elapsed`」的正弦同相。

## 4. 验收

```bash
cargo test -p l2d --lib pose_stack
cargo test -p l2d-wasm-demo --lib
cargo fmt --all -- --check
cargo clippy -p l2d -p l2d-wasm-demo --all-targets -- -D warnings
cargo check --target wasm32-unknown-unknown -p l2d-wasm-demo
cargo run -p xtask -- code-stats --check --only lines
test $(wc -l < crates/l2d-wasm-demo/src/preset/mod.rs) -le 1000
test $(wc -l < crates/l2d-wasm-demo/src/preset/motion_curve.rs) -le 200
test $(wc -l < crates/l2d/src/pose_stack.rs) -le 1000
(cd crates/l2d-wasm-demo && trunk build)
```

`code-stats --only lines` 的 `>500` 仍是 44、`>1000` 仍是 0。新文件算进 `<500` 就不会把 44 顶上去。

`trunk build` 只更新 `dist/`（gitignore）。不要起服务。做完告诉维护者刷新页面后再看手势：点头时头不应再被 ±3° 的摆头带着晃，身比头晚一点；表情包的硬切保持原样。

## 5. 明确不做

不改 `apply_idle_life`，不改眨眼和微表情，不改表情包的恒定增益，不改 `assets/actions/presets.json`，不改 `field_runtime.rs`，不改导演选包，不改 Flutter，不提交。
