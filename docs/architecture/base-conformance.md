# 底座一致性索引（已有测试点名）

> **性质**：这是**索引**，不是第二套实现，也不是新的协议真源。它只把**已经存在**的测试
> 与其钉住的事实列出来。协议仍以 [`performance-protocol-v1.md`](performance-protocol-v1.md)
> 与 [`core-chain-baseline.md`](core-chain-baseline.md) 为准。
> **纪律**：本文件不新增 `#[test]`、不复制断言；测试名以源码为准（改名请回来同步）。
> 立档：**2026-10-12**。

## 1. 口型：线性 RMS → dBFS → `ParamMouthOpenY`

- 文件：`crates/l2d-wasm-demo/src/mouth.rs`（纯函数，原生可测，不被 wasm `cfg` 挡掉）。
- `real_speech_rms_maps_to_visible_mouth_open`：实测语音的 p90 电平映射到肉眼可见的开合。
- `defensive_against_non_finite_input`：非有限 / 非正输入返回 `0.0`，不 panic。
- 事实：口型只由 TTS 音频包络驱动；静音不改口型。

## 2. 姿态分层：`base` < `idle` < `input` < `physics` < `final_override`

- 文件：`crates/l2d/src/pose_stack.rs`（纯逻辑，无 GPU）。
- `base_defaults_then_input_overrides`：`input` 覆盖 `base`。
- `physics_layer_beats_input_but_final_override_beats_physics`：物理压 `input`、`final_override` 压物理。
- `override_layer_beats_everything_and_clear_restores`：`final_override` 最高且可撤销。
- `idle_drives_breath_and_is_not_stuck_after_clear`：`idle` 每帧重算，撤销能传播。
- 事实：各层每帧重新合成，不是把一个持久姿态反复覆盖。

## 3. 预设 / 导演 cue 不得写口型

- 文件：`crates/l2d-wasm-demo/src/preset/tests/fields.rs`
  - `whitelist_stays_the_documented_thirteen`：参数白名单里没有 `ParamMouthOpenY`。
  - `expressions_only_write_facial_channels`：表情只写五官通道。
- 文件：`crates/l2d-wasm-demo/src/preset/tests/fields_map_tests.rs`
  - `thinking_expression_writes_facial_channels_only`：导演 cue 的生效参数里没有 `ParamMouthOpenY`（注释：口型只归 TTS）。
- 事实：预设表与导演 cue 的测试明确禁止写 `ParamMouthOpenY`。

## 4. 切句只按句读，不按字符位置

- 文件：`crates/live2d-ai-runtime/src/dialogue/tests.rs`
  - `long_sentence_is_never_split_at_the_old_48_char_bound`：禁止按字符位置硬切。
  - `arbitrary_delta_splitting_is_consistent`（经 `assert_splitting_invariant`）：任意增量切分保持不变量。
  - `chinese_sentences_keep_punctuation`：中文句读保留。
- 文件：`crates/live2d-ai-runtime/src/dialogue/clean.rs`
  - `sentence_terminators_are_never_added_or_removed`：清洗不增删句读。
- 事实：分句只按真实句读边界；`SentenceAssembler` 的 48 字位置切分已退役。

## 5. 音频句界：一句一片 `start`、一片 `end`

- 文件：`crates/live2d-ai-desktop/src/web_api/ws/audio_tests.rs`
  - `sentence_boundaries_are_exactly_one_start_and_one_end`。
- 事实：一句音频只有一片 `start`、一片 `end`，由引擎的 `first_chunk` / `final_chunk` 给出，
  不在播放端按 epoch 或按块去猜。

## 6. 本索引未点名

- `live2d-ai-core` reducer 的 epoch / 闩锁：本索引**未点名**，不要为了索引新写测试；
  现有回归在 `crates/live2d-ai-core/` 与 `crates/live2d-ai-desktop/src/supervisor/`。
