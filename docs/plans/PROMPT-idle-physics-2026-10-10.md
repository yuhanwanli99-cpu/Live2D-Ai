# 提示词 · 待机开关与物理跟导演

> 立档：2026-10-10。状态：**活**。计划书 `docs/plans/PLAN-idle-physics-2026-10-10.md`。交给有无头浏览器的工作 agent。

```text
你在 /home/skystar/Live2D-Ai，分支 main。只做待机开关和物理跟导演。先读 docs/plans/PLAN-idle-physics-2026-10-10.md。版本已是 0.2.3-rc.2，不要再升，不要打 tag，不要推送。

现状：
- 界面「待机小动作」只让 apply_idle_life 停掉眨眼和微表情。PoseStack::update 每帧仍写 ParamBreath，并把 ParamAngleZ 摆 ±3°，不看这个开关。
- 导演的头/表情/身体在 final_override，合成时压过 input。物理输入是待机层 + input，不含 final_override，所以头发和被物理带动的身体跟着 ±3° 走。
- tick 先 core.update，再 apply_bridge_effects。不要重排这个顺序。

要做：
1. idleEnabled 为 false 时，这一帧的 PoseStack::update 不写呼吸正弦，也不写 ParamAngleZ 的 ±3°。缺省仍为 true。保留 IdleState 和 apply_idle_life，继续用同一个开关。
2. 物理输入改为待机层 + input + final_override，同一参数上 final_override 赢。口型 ParamMouthOpenY 仍走 input。
3. wasm 在写入 bridge.idle_enabled 的那条 sync 上，把同一个布尔交给 PoseStack。
4. pose_stack 原生测试：关着时 ParamAngleZ 保持默认；开着时会离开默认；final_override 上的 ParamAngleZ 进物理输入，不被 ±3° 换掉。

渲染档位不要改代码。验收时在开发者模式切 4K/8K/16K，HUD 应是 x1.00 / x1.50 / x2.00，不保存、不重启。对不上就查 sync 的 tier，不要新做保存流程。

禁止：删 IdleState；接回 action_tx；改 tts.rs 或 melo/start.sh；改 .env、mods.json、live2d-ai.toml；提交 此项目的调研结果.md；升版本；打 tag；commit；push。

动了 l2d 就跑 cargo test -p l2d。动了 wasm 就在 crates/l2d-wasm-demo 里执行 env -u NO_COLOR trunk build。浏览器打开 http://127.0.0.1:18080/app/：待机开关来回两次，头的小摆和眨眼一起停、一起恢复；导演开着发一句带问号的话，歪头在，身体或头发跟着这一下，而不是继续 ±3°。做不到就停在 HUD 和参数读数上，不要报告成功。
```
