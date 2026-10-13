# 执行单：0.2 时代收口（2026-10-13）

> **交给工作 agent 执行。** 范围真源是 [`PLAN-0.2-closeout-2026-10-13.md`](PLAN-0.2-closeout-2026-10-13.md)。
> 本单把那份计划拆成可以逐条做完的任务。与本单冲突时以维护者原话为准：
> 导演开着时，二路 LLM 清洗一路原文送给 TTS 和嘴，并按一路原文出动作；
> 0.2 时代的结尾是这条路径做完，并留下一份干净底座。
> **树**：`/home/skystar/Live2D-Ai`，分支 `main`，写单时 HEAD = `f5d4184`。
> 已验收的身份改动留着，不要还原。不要改 `.env`、`mods.json`、`live2d-ai.toml`。

## 0. 做完是什么样

1. 导演 Mod **关**：按句清洗仍在，不出导演 `action_cue`。待机还在。
2. 导演 Mod **开**，且 `takeover_of` 为 `ReuseConversation` 或 `Own`：每一句仍只有 **一次** 清洗请求。`speech` 送 TTS。cues 按**这一句原文**选预设。用户那句话不参与选预设。
3. 清洗超时、非 2xx、坏 JSON、`display` 不是原文切片：`display = speech = 原文`，再用规则表看**这一句原文**。合法 JSON 里 `cues` 为空 = 这句不动，不回退规则表。
4. `[performance].enabled` 仍缺省 false。动作门开着时，它不另写一份 cues。`staging_enabled` 不是总闸。
5. 出厂 `system_prompt` 为空。`live2d-ai-mod-persona` 还在，工厂数仍是 6。
6. 版本变为 `0.2.5-rc.1`，附注 tag `v0.2.5-rc.1`。不打 `v0.2.5`。不复活 `docs/releases/v0.3.0.md`。

## 1. 禁止

做到就停：

- 不新增第四次 LLM 调用。不删 `SentenceCleaner`。不把 `[performance].enabled` 当成导演开关。
- 不挂回 `action_tx`。不删 core `action/`、`performance/`。不改 `live2d-ai-core` 不变量 4 和 6。
- 不改 `crates/live2d-ai-mod-director/src/lib.rs`，除非 T16 证明 `turn.rs` 做不到。改了也必须 ≤1000 行。现为 969 行。
- 新的 `crates/*/src/**/*.rs` 必须 ≤500 行。`>500` 文件数棘轮是 44，已经满。不要让计数变成 45。
- `engine.rs` 现为 666 行，已经在 >500 名单里。只留发射调用。解析放 `cleaning.rs`（现 325 行）或新的 ≤500 行文件。`cleaning.rs` 要越过 500 就拆出去。
- 不做第二张 model3，不引入 soullink，不写参数重映射器，不改 `V0_SUPPORTED_FORMATS`。
- 不改 `CHANGELOG.md` 的历史句，不删 `local-inference-setup.md` 里的 PowerShell 示例。
- 不提高 `DOCS_BUDGET_LINES`，不把文件挪进 `docs/audit/`。
- 不提交仓库根 `此项目的调研结果.md`。禁止 `git add -A`。禁止 force-push。禁止移动已有 tag。
- 清洗请求继续关思考（`thinking.type = disabled`）。不追 token 延迟。

## 2. 现状锚点（先读，再改）

| 事实 | 位置 |
| --- | --- |
| 产品默认按句清洗，返回 `display` / `speech` | `crates/live2d-ai-runtime/src/cleaning.rs` 的 `CLEAN_SYSTEM_PROMPT`、`CleanedSentence` |
| 流式主链调用点 | `conversation/engine.rs` 的 `sentence_texts`（约 648 行）。表演层整段 JSON 是另一条，只在 `performance.is_some()` 时走 |
| 导演规则表现在看的是用户句 | `director/src/lib.rs` `handle_sentence_ready`：第一句用 `ledger.latest()` 的 preset。`ledger` 在 `TurnPrompt` 写入。`SentenceReady.text` 只喂异步二路 |
| 规则纯函数 | `presets.rs` `rule_cues_for_text`。`sentence_seq` 写死 1。问号只认 `？` / `?`，空槽补 `tilt_left` + `thinking` |
| 接管判定 | `staging.rs` `takeover_of`：两项都空 = `ReuseConversation`；都有 = `Own`；只填一项 = `Incomplete` |
| 表演层装配在 `enabled == false` 时直接 `None` | `supervisor/performance_wiring.rs` `build_performance_runtime`。所以「导演已启用」今天**并没有**把整段表演层打开 |
| 两份 cue 的现有互斥 | `supervisor/turn.rs`：`forward_sentence_ready_to_mods = !engine.performance_enabled()` |
| 预设 id | `smile` `unhappy` `surprised` `thinking` `nod` `shake` `look_left` `look_right` `look_up` `look_down` `tilt_left` `tilt_right` `none` |
| 嘴 | cue 不写 `ParamMouthOpenY`。待机层序 `base < idle < input < physics < final_override` |

动作门（本单的唯一新开关）：

```text
motion_on = director.enabled && takeover_of(staging_base_url, staging_model).active()
```

`Incomplete`（只填了一项）保持今天的行为：清洗仍走主 LLM、不带 cues，Mod 规则层仍按用户句出第一句 cue。在发布说明里写成已知边界，不要顺手改掉。

`motion_on` 时：

- `ReuseConversation`：这一次清洗仍用主 LLM。
- `Own`：这一次清洗改用 `staging_base_url` / `staging_model`。`staging_api_key_env` 空则仍用主 LLM 的密钥变量名。
- 不转发 `SentenceReady` 给 director Mod。
- 不装配 `[performance]` 整段路径。

## 3. 任务

按编号做。前一条的提交或测试没过，不要跳到行为任务。

### 提交 A · 只登记文档

**T1。** `git status --short` 对照第 4 节的文件清单。多出来的已跟踪改动先查是不是身份单。对不上就停，把名单写进回报。

**T2。** 提交 A，消息 `docs: 登记 0.2 时代收口计划`。只加入：

- `docs/plans/PLAN-0.2-closeout-2026-10-13.md`
- `docs/plans/PROMPT-0.2-closeout-2026-10-13.md`
- `docs/plans/PROMPT-identity-base-2026-10-12.md`
- `docs/plans/PROMPT-base-followup-2026-10-13.md`
- `docs/DOC-MAP.md`
- `docs/README.md`

不升版本。不打 tag。

### 提交 B · 已验收身份

**T3。** 提交 B，消息 `docs: 收口皮套接入身份与余项注释`。只加入下面这些（身份单 + 余项已经改过的内容）：

- `.github/workflows/pr-checks.yml`
- `AGENTS.md`
- `crates/live2d-ai-core/src/lib.rs`
- `crates/live2d-ai-desktop/src/main.rs`
- `crates/live2d-ai-desktop/src/web_api/flutter_app.rs`
- `docs/architecture/core-contracts.md`
- `docs/architecture/directory.md`
- `docs/architecture/font-fallback-offline.md`
- `docs/architecture/mod-community-license.md`
- `docs/architecture/tts-is-core.md`
- `docs/architecture/base-conformance.md`
- `docs/headless-deploy.md`
- `docs/local-inference-setup.md`
- `docs/research/neko-ui-alignment-and-gaps.md`
- `docs/research/评估-底座定位-Grok-2026-10-12.md`
- `docs/research/评估-底座定位与工程质量-2026-10-12.md`
- `docs/verification/ignition-core-loop.md`
- `shell/README.md`
- `shell/flutter/lib/ui/theme.dart`

不加入 `此项目的调研结果.md`。不升版本。这一步之后再改人设，避免和身份提交混在一起。

### 人设

**T4。** 三处测试夹具的 `system_prompt` 改成空串。键和 `max_history_pairs` 留着。

- `shell/flutter/test/settings_api_test.dart` 约 17、27 行
- `shell/flutter/test/settings_controller_test.dart` 约 18 行

不要换成另一句角色介绍。

**T5。** `docs/local-inference-setup.md` 示例里的 `system_prompt` 改成 `""`。只改这一行。PowerShell 示例留着。

**T6。** 再 grep `persona.yaml`（`crates/`、`shell/flutter/`、`scripts/`）。仍没有运行时代码读取，就删除 `shared/persona.yaml`。历史文档里的路径不要改。有运行时读取就停，把命中写进回报。

**T7。** `AGENTS.md`「产品口径」块（约 382 行，2026-09-21 那段）**原文不动**。在 2026-10-07 追加段后面加一段，日期写 2026-10-13：

- 导演 Mod 开启且接管生效时，动作按一路原文的这一句，不按用户输入。
- 导演关闭时不出导演动作。
- 出厂不设人设，`system_prompt` 为空。角色卡仍由 persona Mod 在用户自己启用后导入。
- 不整篇情绪分类，不把标签写进台词，不随机表情，不跨轮常驻，不照 N.E.K.O 重做情绪模型。

同文件首段的现行版本先不要改，留给 T21。

**T8。** `export PATH="$HOME/flutter/bin:$PATH"` 后：

```bash
cd shell/flutter && flutter test test/settings_api_test.dart test/settings_controller_test.dart
```

**T9。** 提交 C，消息 `fix: 出厂不设人设`。只含 T4–T7 的文件。不升版本。`mod_count_is_six` 不用改；若你改到了工厂表，还原。

### 导演行为

**T10。** 在 host 能看见 `DirectorTakeover` 的地方做纯函数 `motion_on`（第 2 节的公式）。单测三条：`enabled=false` → 关；两项都空且 enabled → 开；只填 `staging_base_url` → 关。放在已有 `performance_wiring.rs` 或它旁边的新文件。不要写进 `director/src/lib.rs`。

**T11。** `SentenceCleaner` 增加动作开关，缺省关。关着时 `CLEAN_SYSTEM_PROMPT` 与解析保持现在的两字段，现有 `cleaning.rs` 测试保持绿。开着时**同一次**请求的 JSON 多一个可选 `cues`。提示词写明：cues 只根据用户消息里的这一句原文选预设 id，不根据别的句子；`display` / `speech` 的旧约束不变；不要在台词里写表情标签。

**T12。** 解析 `cues`：元素只收 `preset_id` 字符串。必须属于第 2 节的 id 表，否则丢掉该条。host 填 `intensity = 1`、`ttl_ms = 2000`、`sentence_seq = 当前句`、`field = None`、`hold` 不设。`none` 保留，表示这句撤销。空数组是成功，表示这句不动。不要让模型直接写 `Param*`。

**T13。** 失败回退新函数放在 `presets.rs`：`rule_cues_for_sentence(text, sentence_seq)`。内部仍调现在的 `rule_cues_for_text`，再把每条 `sentence_seq` 改成参数。`rule_cues_for_text` 保持「seq = 1」的旧测试。回退的 `text` 参数只能是这一句一路原文。函数签名里不要出现用户句。

**T14。** `sentence_texts` 在 `motion_on` 时除了 `(display, speech)` 还交出 cues。成功解析用模型 cues。失败用 T13。发射沿用已有 `EngineEvent::ActionCue`（`engine.rs` 表演层那段约 497 行的形状）：在这句 `SentenceReady` 之前发出，`covers_upto_seq` 至少盖住这句。不新增 WS 帧类型。`speech` 仍是送 TTS 的那份。

**T15。** `Takeover::Own` 且 `motion_on`：这一次清洗的 base_url / model / 密钥变量名改用 staging。`ReuseConversation`：仍是主 LLM。密钥只走 `secrets::lookup` 的现有路径，日志不写值。`Incomplete`：清洗客户端不变，不解析 cues。

**T16。** `motion_on` 时 `forward_sentence_ready_to_mods = false`。实现放在 `supervisor/turn.rs` 已有的那个布尔旁边，条件是「表演层开着 **或** 动作门开着」。不要为了互斥去把 `[performance].enabled` 设成 true。这样 `director/src/lib.rs` 可以不改。若发现不改 lib 就会在动作门开着时仍发出用户句规则 cue，先停，说明原因，再考虑 lib 里的最小跳过，并守住 1000 行。

**T17。** 测试先红后绿。名字按这个意图，文件放在对应包的现有测试模块或新的 ≤500 行测试文件：

1. `cleaner_off_keeps_two_field_json`：动作关，旧提示词，无 cues 字段也能解析。
2. `motion_cues_follow_the_assistant_sentence`：原文「今天天气不错。」、模型返回 `nod` → cue 是 `nod`，seq 等于这句。测试数据里不要把「开心」传进解析函数。
3. `user_text_is_not_a_cue_input`：签名或调用点证明用户句到不了 T13 / T12。用源码断言也可以：`rule_cues_for_sentence` 的调用实参不是 `last_user_text`。
4. `bad_json_falls_back_to_original_sentence_rules`：原文含 `？`，坏 JSON → `speech` 为原文，cues 含 `tilt_left` 与 `thinking`，seq 是这句而不是写死的别的句。
5. `empty_cues_mean_no_motion`：合法 JSON `cues: []` → 不调用规则表。
6. `unknown_preset_id_is_dropped`。
7. `cue_preset_ids_are_not_mouth`：结果 id 全在 allowlist 里。不新增嘴部预设。
8. `own_endpoint_is_used_only_when_both_staging_fields_are_set`：不发真实网络，用现有假客户端。
9. `performance_default_off_does_not_emit_a_second_cue_set`：`[performance].enabled` 缺省仍是 false；动作门开着时 `forward_sentence_ready_to_mods` 为 false。
10. 旧测试保持绿：`sentence_cleaner_is_wired_only_when_clean_tts_is_on`、`takeover_of` 已有测试、`action_request_is_dormant_not_delivered`、`mod_count_is_six`。

**T18。** 待机。先读 `crates/l2d/src/pose_stack.rs` 里 `idle_drives_breath_and_is_not_stuck_after_clear` 与 `override_layer_beats_everything_and_clear_restores`。跑 `cargo test -p l2d pose_stack -- --test-threads=8`。两条都在且通过：不改 `pose_stack.rs`，不改 wasm。发布说明写：覆盖层在 clear 之后会还呼吸；本轮没有新的粘滞复现。T12 已经禁止 `hold`。若这两条测试不存在或失败，停下来写进回报，不要改层序。

**T19。** `docs/architecture/performance-protocol-v1.md` §1 在 Q1 那句下面加 2026-10-13 短段：动作门开着时，按句清洗的同一次响应带 cues，依据是这一句原文；`[performance].enabled` 缺省关，不与这条路径双写。V1–V12 其余句子不动。这段写长了就压缩，文档门禁变红就再压。

**T20。** 点名测试：

```bash
cargo fmt --all -- --check
cargo test -p live2d-ai-runtime --lib cleaning
cargo test -p live2d-ai-mod-director --lib rule_cues
cargo test -p live2d-ai-desktop --lib motion_on
cargo test -p live2d-ai-desktop --bin live2d-ai-desktop mod_count_is_six
cargo test -p live2d-ai-core --lib
cargo clippy -p live2d-ai-runtime -p live2d-ai-mod-director -p live2d-ai-desktop --all-targets -- -D warnings
cargo run -p xtask -- code-stats --check --only docs
cargo run -p xtask -- code-stats --check --only lines
```

包名或测试过滤器对不上时，改成能选中 T17 新测试的过滤器，并在回报里写实际命令。clippy 若因无关旧警告失败，只修你改过的文件。全量 `cargo test --workspace` 与 `cargo clippy --workspace` 有时间就跑，没跑就在回报写没跑。不点火，不开浏览器。

### 版本

**T21。** T20 通过之后把现行版本改成 `0.2.5-rc.1`。只改这些：

| 文件 | 改什么 |
| --- | --- |
| `Cargo.toml` | `version = "0.2.5-rc.1"` |
| `Cargo.lock` | 仅 workspace 包版本跟着变。不要重排无关依赖 |
| `shell/flutter/pubspec.yaml` | `version: 0.2.5-rc.1+19`，注释里的版本号一起改 |
| `README.md`、`README.zh-CN.md` | 首屏版本 |
| `AGENTS.md` | 首段「现行版本」那句。历史段落不动 |
| `docs/README.md`、`docs/DOC-MAP.md` | 当前发布说明改指向新文件 |

新增 `docs/releases/v0.2.5-rc.1.md`，短。必须包含：两态行为、人设为空、`Incomplete` 仍是旧规则层、白模以外不证明、`action_tx` 仍休眠、浏览器未点、不打 `v0.2.5`。第 6 节已知边界从收口计划抄要点，不要扩写。

**T22。** 再跑一次 `cargo run -p xtask -- code-stats --check --only docs`。然后提交 D，消息 `feat: 导演开启时按句清洗并按原文出动作`。含 T10–T21 的代码、测试、协议段、版本文件、发布说明。

**T23。** 附注 tag，不要移动旧 tag：

```bash
git tag -a v0.2.5-rc.1 -m "0.2.5-rc.1：导演按句清洗并按原文出动作"
```

不打 `v0.2.5`。可以 `git push origin main` 和 `git push origin v0.2.5-rc.1`。认证失败就停，回报里贴 remote 的原句。不要改 remote，不要 force。

## 4. 提交时的工作树提醒

写单时尚未提交的身份改动都在 HEAD `f5d4184` 之上。T3 的清单若和 `git status` 不一致，以「这些路径上的身份/余项改动」为准，不要把 `target/`、`shell/flutter/build/`、`.env`、`mods.json`、`live2d-ai.toml` 加进去。

## 5. 回报

1. 四个提交的哈希。tag 是否打上。push 结果。
2. T1–T23：完成 / 跳过（原因）。T16 有没有改 `director/src/lib.rs`，改完多少行。
3. T17 十条测试的最终名字。
4. T18 两条待机测试的结果。
5. T20 每条命令的退出码。全量 workspace 测试写跑了或没跑。
6. 明确没做的：第二张 model3、`action_tx`、`v0.2.5`、浏览器点火。
