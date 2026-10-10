# 待机开关停住摆头，物理跟着导演

> 立档：2026-10-10。状态：**活**。真源：本文件。配套提示词 `docs/plans/PROMPT-idle-physics-2026-10-10.md`。
> 出声接线已在工作区的 `melo/start.sh` 上跑通，本份不动它。版本停在刚打的 `0.2.3-rc.2`，不升版本，不打 tag。

## 现状

「待机小动作」只接到渲染面的 `apply_idle_life`（眨眼、微表情、另一套呼吸）。关掉之后 HUD 上的眨眼会停。

另一套待机不看这个开关。`PoseStack::update`（`crates/l2d/src/pose_stack.rs`）每帧重写 `ParamBreath`，并把 `ParamAngleZ` 摆 ±3°。`ModelRendererCore::update` 每帧都调用它。所以开关关掉，头还在轻轻摆。

导演的头、表情、身体写在 `final_override`，合成时压过 input，表情参数本身不会被眨眼盖掉。物理的输入是「内建待机 + input」，不含 `final_override`。头发和被物理带动的身体跟着那 ±3° 走，不跟着导演写上的角度走。

渲染档位是本机 `DisplayPrefs`。同一条 `sync` 带 `tier`，渲染面当条消息调用 `sync_canvas_size`。这一页没有「保存并应用」，也不该重启 18080。本份不改档位。

时序保持现状：`tick` 先 `core.update`，再 `apply_bridge_effects` 写下这一帧的 cue。`final_override` 里已经躺着的值（上一帧写下、还没撤销的）可以进物理。新 cue 晚一帧进物理可以接受。不要为了消掉这一帧去重排 `tick`。

## 要改的

1. `idleEnabled == false` 时，`PoseStack::update` 这一帧不写呼吸正弦，也不写 `ParamAngleZ` 的 ±3°。缺省仍是开，和现在的界面默认一样。`IdleState` / `apply_idle_life` 保留，继续由同一个开关控制。
2. 物理的输入信号改为「待机层 + input + `final_override`」。同一参数上 `final_override` 赢。导演写在 `ParamAngleZ` 上的歪头因此带动物理。口型仍走 input 的 `ParamMouthOpenY`，不要改。
3. wasm 侧在已经写入 `bridge.idle_enabled` 的那条 `sync` 上，把同一个布尔传给 `PoseStack`（经 `ModelRendererCore`）。缺省 true。

回归放在 `pose_stack` 的原生测试里：关掉时连续 `update`，`ParamAngleZ` 保持默认；打开时它会离开默认；`final_override` 上的 `ParamAngleZ` 出现在物理输入里，而不是被 ±3° 换掉。不要靠截图像素当唯一证据。

## 不做

- 不删 `IdleState`，不接回 `action_tx`，不改口型、分句、TTS、`start.sh`。
- 不改渲染档位的算法，不加保存按钮，不把档位写进 `live2d-ai.toml`。
- 不改 `.env`、`mods.json`、`live2d-ai.toml`。不提交 `此项目的调研结果.md`。不升版本，不打 tag，不推送。

## 验收

1. 浏览器打开 `http://127.0.0.1:18080/app/`。待机开：头有轻微摆动，眨眼会动。待机关：摆动停、眨眼停，口型仍跟声音。来回拨两次都生效，不用重启服务。
2. 导演开着时发一句带问号的话。歪头和思考表情在，头发或被物理带动的身体跟着这一下走，而不是继续那条 ±3° 的小摆。待机关掉再发一句，歪头仍在，小摆不在。
3. 开发者模式下切 4K / 8K / 16K，HUD 为 `x1.00` / `x1.50` / `x2.00`，不点「保存并应用」、不重启。这一条只核对，档位代码不动；对不上再查 `sync` 里有没有 `tier`，不要另做一套保存。
4. 动了 `l2d` 跑 `cargo test -p l2d`。动了 wasm 就要 `env -u NO_COLOR trunk build`（在 `crates/l2d-wasm-demo`）。`trunk` 带 `NO_COLOR=1` 会失败。
