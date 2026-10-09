# 问句要有歪头和思考 · 交给下一个 agent

> 2026-10-07。管理员已定口径。下一个 agent 只做 T9。
> 不要提交、不要打 tag、不要推送、不要升版本。做完由管理员审查。
> 先读本文件，再读 `docs/plans/PROMPT-director-tts-2026-10-07.md`（T8 已收，不要重做）。
> 调研只作边界：`docs/research/调研结果-导演层同类对照-2026-10-07.md`。

## 口径

台词仍然是主模型原文。问句补动作时不改字、不删句、不加标签、不做内容审查。`segments.concat() == source` 保持整份失败。

问句只认原文里的 `？`（U+FF1F）和 `?`（U+003F）。不把「吗 / 呢 / 什么」当成问句，那些词会误伤陈述句。一轮里只补**第一处**问句所在的那一段。没有问号就不补。

补的是两样，都锚在该段音频开始（`seg:N`，N 从 1 起）：

| 条 | 字段 | 值 |
| --- | --- | --- |
| 歪头 | `head` | `x=0`，`y=0.12`，`z=0.45`，`intensity=2`，`hold=false`，`ttl_ms=1800` |
| 思考 | `expression` | `id=thinking`，`intensity=2`，`hold=false`，`ttl_ms=2600` |

`z` 固定为正，与现成 `tilt_left` 同向。不随机，不 50% 重播，不跨轮常驻。

幅度是算过的：头通道满幅 30°，再乘出厂 `head_scale` 0.75，所以 `0.45 × 2 × 30 × 0.75 ≈ 20°`。比现成手势包大约 9° 明显，也不会顶到 30° 钳位。不要改滑条默认值。

问句这一段的歪头和思考**以词典为准**。模型在这两项上交了别的值，也换成上表。别的段上模型交来的合法 cue 可以留下。`cues` 为空加上合法分段仍然是成功。补丁加在成功结果上，不把这一轮打成失败，也不退回直送。

嘴只归 TTS。导演 cue 不得写 `ParamMouthOpenY`。现有字段白名单已经排除口型，本轮保持这条，并留一条测试钉住。

## 谁来填动作

第二路没有导演词典，通用大模型也没有「填写 Live2D cue 数组」的训练。0.3 之前不要求它当导演。它继续只切分原文。动作由本地词典产生：问号 → 上表；规则层原有的问候点头、高兴微笑、不悦、惊讶保持。不要把这张表写进 system 提示。`prompt.rs` 本轮不改。

六级参数仲裁（VTube Studio 的 P0–P5）**不在 T9，也不在 0.3**。本轮只保证三路不抢：待机写呼吸和眨眼，词典写头和表情，TTS 写嘴。

另一条限制：接管成功时走的是字段通道（`field=head|expression`），不是 `presets.json` 里的手势包。`head` 是保持一个角度，不是点头那种包络。`nod` / `tilt_left` 写进表情 `id` 会在渲染面被丢掉，因为表情表只有脸。新的手势包给规则层和 `preset_id` 通道用；问句补丁本身用上面的字段值。

## 新增三个包

只加这三个，用现有参数，不加手臂、不加新参数名。

`thinking`（expression，时长 2600）：

- 五官：`ParamMouthForm=0`，`ParamEyeLOpen=0.55`，`ParamEyeROpen=0.55`，`ParamBrowLY=-0.45`，`ParamBrowRY=-0.45`
- 包内小幅头：`ParamAngleZ=6`，`ParamBodyAngleZ=2`（身/头 = 0.333，落在 [0.30, 0.50]）
- 字段表里的 expression **只抄五官五行**。头角留在包里，给 `preset_id` 通道；字段通道的歪头由补丁的 `head.z` 负责，避免两路叠加。

`look_up` / `look_down`（motion，`wave=single`，时长 1100）。主轴用 Y，和 `nod`（Y 为负、900ms）分开：

| id | ParamAngleY | ParamBodyAngleY | ParamAngleX | ParamBodyAngleX |
| --- | ---: | ---: | ---: | ---: |
| `look_up` | 12 | 3.9 | 2.7 | 0.8 |
| `look_down` | -12 | -3.9 | -2.7 | -0.8 |

身/头主轴 = 3.9/12 = 0.325。解析期头 ≤30、身 ≤10。intensity 走满时允许钳位，与现成手势包同一条口径。

中文展示名：思考、向上看、向下看。字符必须已在 `assets/fonts/*.ranges.txt` 里；缺字就改 label，本轮不重做字体子集。

## 要改的文件

- `crates/live2d-ai-runtime/src/performance/plan.rs`：`parse_v1` 成功之后调用纯函数补问句。legacy 预设路径不走这个函数。schema 的 expression `enum` 只留 `smile`、`unhappy`、`surprised`、`thinking`、`none`。手势 id 出现在 expression 上：丢该条并 warn，整份计划仍成功。
- `crates/live2d-ai-runtime/src/performance/prompt.rs`：**不改**。不要把词典写进 system 提示。
- `crates/live2d-ai-mod-director/src/presets.rs`：`PRESET_IDS` 加上三个 id。`thinking` 归表情槽，`look_up` / `look_down` 归手势槽。`rule_cues_for_text`：正文含 `？` 或 `?` 时，手势槽还空就加 `tilt_left`，表情槽还空就加 `thinking`。已有的问候点头、情绪脸优先，不要被问号挤掉。不要为此新增设置项。
- `assets/actions/presets.json`、`assets/actions/preset_labels.json`。
- `crates/l2d-wasm-demo/src/preset/mod.rs` 的 `PRESET_IDS` 与内建 `PRESETS` 与外置 JSON 逐条一致。`field_map.rs` 的内建表情加上 `thinking` 的五官五行。
- 数死了「9 个包」或「3 张表情」的测试跟着改为 12 与 4。包括 `preset/tests/assets.rs`、`preset/tests/packs.rs`。补丁测试至少三条：有问号且 cues 为空 → 得到上表两值，分段不变；该段模型已交了别的表情或别的 z → 仍换成上表两值；没有问号 → 不补。删掉补丁调用必须使第一条变红。另有一条：字段白名单不含 `ParamMouthOpenY`。
- `AGENTS.md` 动作表现状表里「共 9 条包」补上这三个 id。不要改 2026-09-21 那段历史裁决原文；T9 的追加段已经写在它后面。

## 禁止

- 把 `[emotion]` 一类标签写进主模型回复，或放宽拼接必须等于原文。
- 整篇回复打 happy/sad/angry/surprised/neutral，随机挑表情，表情跨轮不撤。
- 再请求一个独立情绪模型。改 `staging_*` 的开关语义。
- 新的 WS 帧。接回 `action_tx`。改口型、待机、拖动、动作幅度滑条、舞台背景、语音。
- 改 `mods.json`、`.env`、`live2d-ai.toml`。升版本、提交、推送。
- 改 `docs/architecture/performance-layer-v0.md` 与 `performance-protocol-v1.md`（仍留到单独一轮）。
- 在仓库根目录跑 `flutter analyze`。把测试改成恒真。

语音仍封存，不在 T9。

## 验收

```bash
cargo test -p live2d-ai-runtime --lib performance
cargo test -p live2d-ai-mod-director
cargo test -p l2d-wasm-demo --lib
cargo check --target wasm32-unknown-unknown -p l2d-wasm-demo
cd crates/l2d-wasm-demo && trunk build
cd /home/skystar/Live2D-Ai/shell/flutter
/home/skystar/flutter/bin/flutter analyze
/home/skystar/flutter/bin/flutter test test/font_subset_test.dart test/director_panel_test.dart
```

都要 exit 0。贴出各自最后 30 行。`trunk` 不在 PATH 就写明，不要假装渲染面产物已更新。桌面全量测试若因 `HTTP_PROXY` 把传输失败用例如成 `llm_upstream_502`，先去掉代理再跑，不要改断言，也不要改 reqwest 客户端。

## 提示词（整段复制给下一个 agent）

```text
你在 /home/skystar/Live2D-Ai，分支 main。先读 docs/plans/PROMPT-director-expression-2026-10-07.md。只做 T9。不要提交，不要推送，不要升版本。T8 已收，不要重做装配。

口径：台词仍是主模型原文。第二路只切分，不负责当导演，也不要改它的 system 提示。问句（只认 ？ 和 ?）所在的第一段，在表演计划校验成功之后由本地词典写入：head x=0 y=0.12 z=0.45 intensity=2 hold=false ttl_ms=1800；expression id=thinking intensity=2 hold=false ttl_ms=2600。这两项以词典为准，模型交了别的值也换掉。没有问号不补。分段一个字不改。空 cues 仍算成功。不随机，不跨轮保持。嘴只归 TTS，不得写 ParamMouthOpenY。六级参数仲裁不做。

新增三个包，只用现有参数：thinking（五官 MouthForm 0、双眼 Open 0.55、双眉 Y -0.45，另加 AngleZ 6、BodyAngleZ 2）；look_up / look_down（single，1100ms，AngleY ±12、BodyAngleY ±3.9、AngleX ±2.7、BodyAngleX ±0.8）。字段表情表只收 thinking 的五官。expression 的 schema 只允许 smile、unhappy、surprised、thinking、none；手势 id 填进表情就丢该条并 warn。规则层在正文有问号时补 tilt_left 与 thinking；问句这两项以词典为准。不要改 prompt.rs。

中文名用思考、向上看、向下看。缺字就改名，不重做字体。AGENTS.md 里「共 9 条包」补上这三个 id。不要改 2026-09-21 的历史裁决原文。

禁止：标签写进台词；五类情绪分类；随机表情；独立情绪模型；新 WS 帧；接回 action_tx；改口型、待机、滑条、背景、语音、mods.json、.env、toml；改两份表演架构文档；仓库根目录 flutter analyze；恒真测试。

验收并贴出最后 30 行：
cargo test -p live2d-ai-runtime --lib performance
cargo test -p live2d-ai-mod-director
cargo test -p l2d-wasm-demo --lib
cargo check --target wasm32-unknown-unknown -p l2d-wasm-demo
cd crates/l2d-wasm-demo && trunk build
cd shell/flutter && /home/skystar/flutter/bin/flutter analyze && /home/skystar/flutter/bin/flutter test test/font_subset_test.dart test/director_panel_test.dart
trunk 不在 PATH 就如实写明。HTTP_PROXY 导致桌面传输用例变 502 时，去掉代理再跑，不要改断言。
```
