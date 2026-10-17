# 计划：0.2 时代收口（2026-10-13）

> **2026-10-16**：收口候选 `0.2.5-rc.1` 已打过。其后已验收的动作衔接与本句续播收进 `0.2.6-rc.1`。功能冻结后的债务已收，稳定版是 `v0.2.7`。3.0 顺序 1、6、2、4、3、7，第一片见 [`PROMPT-3.0-motion-first-2026-10-17.md`](PROMPT-3.0-motion-first-2026-10-17.md)。债务阶段见 [`PLAN-debt-after-0.2.6-2026-10-16.md`](PLAN-debt-after-0.2.6-2026-10-16.md)。
> **交给下一个 agent 执行。** 本文件是 0.2.x 的收口计划。
> 维护者原话：「目前 0.2.0 时代的结尾就是完善导演系统并留下一个技术/代码/工程上一个干净无债务的基础底座版本。」
> 上一单 [`PROMPT-base-followup-2026-10-13.md`](PROMPT-base-followup-2026-10-13.md) 已验收。
> 它的 §5 和 [`PROMPT-identity-base-2026-10-12.md`](PROMPT-identity-base-2026-10-12.md) 的 §5
> **停止执行**。权威评估稿 [`docs/research/评估-底座定位-Grok-2026-10-12.md`](../research/评估-底座定位-Grok-2026-10-12.md)
> §6 第 5 条（Q1 仍未决、默认按用户输入出 cue）同样停止执行。
> **树**：`/home/skystar/Live2D-Ai`，分支 `main`，HEAD 写作时为 `f5d4184`。已验收的身份改动留着，不要还原。
> **执行单**：[`PROMPT-0.2-closeout-2026-10-13.md`](PROMPT-0.2-closeout-2026-10-13.md)。工作 agent 按那一单的 T1–T23 做。本计划是范围真源。

## 0. 做完是什么样

导演 Mod **关**着：主链仍是文本 → 一路 LLM → 按句清洗 → TTS → 嘴型 → Live2D。待机呼吸 / 眨眼还在。没有导演动作。

导演 Mod **开**着：同一句上的二路 LLM 做两件事。它把一路原文清洗后送给 TTS，播放和嘴型只用这份清洗文本。它按**一路原文**（不是用户那句话，也不是清洗后的台词）给出头、颈、表情、手势。嘴仍只由音量包络写 `ParamMouthOpenY`。

出厂不设人设。`system_prompt` 空着。

版本停在 0.2.x。收口候选是 `0.2.5-rc.1`。不发布 `0.3.0`，不复活 [`docs/releases/v0.3.0.md`](../releases/v0.3.0.md)。

「干净、无债务」在本时代的意思写在第 6 节：本时代清单里的项做完，其余项写进发布说明的「已知边界」。不把仓库里每一条 TODO 都当成这轮的债。

## 1. 维护者已裁定

1. **导演是半核心，放在 Mod 里实验。** 开启后，核心链的这一段换成二路 LLM：清洗一路原文，送给 TTS 做播放和嘴型；同时按一路原文驱动 Live2D 动作。现状不好，可能和一路 LLM、默认待机抢同一批参数。
2. **多皮套 / 第二张 model3 是没做完的证明。** 材料已经在仓库里（第 5 节）。本时代不新做重映射器，不再引进一张 model3。
3. **版本、tag、提交由本计划规定。** 见第 4 节。
4. **人设不设，删除。** 删的是出厂人设字符串和已无人读取的历史人设文件。`live2d-ai-mod-persona` 留着，缺省停用。它是用户自己导入角色卡的入口。要连这个 Mod 一起删，停下来问。

2026-09-21「表演决策的输入是用户输入」只对**导演关闭**的路径继续有效（关闭时本来就不出导演动作）。导演开启后，动作的输入改为一路原文。这是 2026-10-13 的新裁定，写进 `AGENTS.md` 那一段产品口径时加日期，不要改历史段落。

## 2. 现有三条路，收成导演开 / 关两态

不要新增第四次 LLM 调用。不要删 `SentenceCleaner`。不要把 `[performance].enabled` 当成导演开关。

| 现有机制 | 现在做什么 | 收口后 |
| --- | --- | --- |
| `SentenceCleaner`（`crates/live2d-ai-runtime/src/cleaning.rs`） | 产品默认 `clean_tts=true`。同一 `[llm]` 端点，一句一次，返回 `display` / `speech`。失败则两边都退回原文 | **保留，并且是唯一的按句二路调用。** 导演开启时，同一次响应再带这一句的 cues。cues 的依据是这一句的原文（`display` 对应的原文切片），不是 `speech`，不是用户输入 |
| 导演规则层 `rule_cues_for_text`（`crates/live2d-ai-mod-director/src/presets.rs`） | 常按用户输入的关键词表出 cue。问句补丁只认原文里的 `？` / `?` | 导演开启且二路 cues 有效时，**这张表不出 cue**。二路失败、超时、坏 JSON 时，才用这张表，输入改成**这一句的一路原文** |
| `[performance]`（`performance_wiring.rs` 的 `build_performance_runtime`） | `enabled` 缺省 false。打开后等整段回复，交回不改写原文的 JSON，并产 cues。`staging_base_url` 与 `staging_model` 都空 = 复用主 LLM；都有 = 自有端点；只填一项 = 不接管 | **继续缺省关。** 导演按句路径开着时，这一层不再额外写 cues。`staging_enabled` 仍不是总闸（`staging.rs` 的 `takeover_of`） |
| 待机（`PoseStack`，`base < idle < input < physics < final_override`） | 呼吸 / 眨眼在 idle。预设写 `final_override`，同键压过 idle。嘴在 input 的 `ParamMouthOpenY` | 动作持续期内，同键仍由 `final_override` 压过 idle。到点必须撤掉。撤掉之后呼吸、眨眼回来。cue 不写 `ParamMouthOpenY`，不把 `ParamBreath` 定死 |

`takeover_of` 继续只管二路用哪个端点：两项都空就复用主 LLM；两项都有就用导演端点；只填一项则导演**动作**不接管，按句清洗仍走主 LLM。不要改 `mods.json`、`.env`、`live2d-ai.toml` 去打开导演。

`crates/live2d-ai-mod-director/src/lib.rs` 有 969 行，顶在 1000 行豁免上。优先改 `presets.rs`、`staging.rs`，或新增一个短文件。必须动 `lib.rs` 时，先数行，越过 1000 就停，把新增逻辑拆出去，不放宽棘轮。

## 3. 切片

按顺序做。前一片没过第 7 节，不开始下一片。

### S1 · 登记与已验收文档

本文件、`docs/DOC-MAP.md`、`docs/README.md` 的规划链接、两份旧执行单 §5 的停止横幅，随执行时的第一次提交进入仓库。不要把仓库根的 `此项目的调研结果.md` 加进去。不要提交 `.env`、`mods.json`、`live2d-ai.toml`。

已验收、仍未提交的身份收口和余项，单独一次提交，消息用 `docs: 收口皮套接入身份与余项注释`。不升版本。

### S2 · 人设不设

出厂 `system_prompt` 改为空串。不要换成另一句角色介绍。

| 位置 | 做法 |
| --- | --- |
| `shell/flutter/test/settings_api_test.dart` 约 17、27 行 | `"system_prompt":""`。键和 `max_history_pairs` 留着 |
| `shell/flutter/test/settings_controller_test.dart` 约 18 行 | 同上 |
| `docs/local-inference-setup.md` 约 281 行的示例 | `system_prompt = ""` |
| `shared/persona.yaml` | Rust `crates/` 与 Flutter `shell/flutter/` 已零引用。执行前再 grep `scripts/` 里会跑的代码。仍零引用就删文件。历史文档里的路径留着 |

不改本机 `live2d-ai.toml`（那里已经是空串）。不改 `AVAILABLE_MOD_FACTORIES`。不改 `mod_count_is_six`。

`AGENTS.md` 只在「产品口径」那一段末尾加一段 2026-10-13：导演开启后动作按一路原文；出厂不设人设。历史段落不动。

### S3 · 导演开 / 关

行为以第 0 节和第 2 节为准。测试先写失败再改实现。至少这些：

1. 导演关：一轮对话不出导演 `action_cue`。按句清洗仍在。`sentence_cleaner_is_wired_only_when_clean_tts_is_on` 继续绿。
2. 导演开：用户句含「开心」、这一句一路原文是中性句时，**不**因为用户句出 `smile`。cue 的输入是原文。
3. 导演开：送给 TTS 的是清洗后的 `speech`。cue 的参数列表里没有 `ParamMouthOpenY`。
4. 二路超时或坏 JSON：`speech` 退回这一句原文；规则表只看这句原文。原文里有 `？` 或 `?` 时，仍是歪头 + `thinking`，只补这一次。
5. `[performance].enabled` 保持缺省 false。把它临时打开时，同一轮不得再叠加第二份 cues。测完恢复缺省。
6. cue 到期后 `ParamAngleZ` 不被留在覆盖层；`ParamBreath` 仍由 idle 驱动。现有 `pose_stack` 测试继续绿。

`performance-protocol-v1.md` §1 的 Q1 句改成 2026-10-13 裁定的两态，并写明 `[performance]` 缺省关、不与按句路径双写。V1–V12 的其余冻结句不动。文件变长就压缩这一段，不要新开长文。

### S4 · 版本与已知边界

S3 的测试与第 7 节通过之后：

- 版本三处加上两份 README 首屏、`AGENTS.md` 首段现行版本、`docs/README.md` 与 `docs/DOC-MAP.md` 的当前发布行，改成 `0.2.5-rc.1`。
- 新增短发布说明 `docs/releases/v0.2.5-rc.1.md`：两态行为、人设为空、第 6 节的已知边界。浏览器没点过就在第一句写明。
- 打附注 tag `v0.2.5-rc.1`。不移动 `v0.2.4-rc.1` 及更早的 tag。
- **不打 `v0.2.5`。** 那个 tag 要等一次真点火：导演关一轮、导演开一轮，嘴在动、动作到期后待机还在。本计划不把没看过的浏览器写成已验收。

## 4. Git

执行 agent 按这个顺序提交。每次只加本片点名的文件。禁止 `git add -A`。禁止 force-push。禁止改写已有 tag。

| 提交 | 何时 | 版本 |
| --- | --- | --- |
| `docs: 登记 0.2 时代收口计划` | S1 的新文档与横幅 | 仍是 `0.2.4-rc.1` |
| `docs: 收口皮套接入身份与余项注释` | 已验收身份 + 余项。可含 `docs/research/评估-底座定位-Grok-2026-10-12.md` 与并行稿、`docs/architecture/base-conformance.md`，因为地图已经指向它们 | 仍是 `0.2.4-rc.1` |
| `fix: 出厂不设人设` | S2 | 仍是 `0.2.4-rc.1` |
| `feat: 导演开启时按句清洗并按原文出动作` | S3 + S4 | `0.2.5-rc.1`，并打同名附注 tag |

推送：本机曾经因无 token 失败。执行时可以 `git push origin main` 与 `git push origin v0.2.5-rc.1`。认证失败就停下并报告，不要改 remote，不要 force。

## 5. 多皮套：登记，本时代不做

这些是已有材料和尝试，下一时代从这里接着做，不从零发明重映射器：

- 协议已写「换皮只换表」：`docs/architecture/performance-protocol-v1.md` 约 299–310 行，可选覆盖 `assets/actions/field_map.json`（仓库里可以没有这份文件，渲染面回落内建表）。
- 模型库「激活即换皮」是 `docs/releases/v0.1.0-rc.2.md` §3.2，只证明换 URL，不证明第二套参数名。
- 旧双端适配仍在 `shared/model-adapter/bai.adapter.json` 与 `ADAPTER.md`。Rust `crates/` 不读它。`niziiro_mao.adapter.json` 今日不在这个目录。
- 半身 / 跨皮套调研：`docs/research/live2d-halfbody-motion-research.md`、`docs/research/live2d-halfbody-presetfree-control.md`。
- 白模试验：`verification/soullink-p0-report.md`（2026-08-22，分支 `refactor/elegance`，生成过 bai profile）。`shared/model-profiles/` 今天不在树上。
- 现行渲染只认白模 moc3：`crates/l2d/src/lib.rs` 的 `V0_SUPPORTED_FORMATS`。缺参数的 cue 丢弃（`dropped_for_bad_cue`）。
- `docs/releases/v0.3.0.md` 是作废草案。里面「导入第二个模型」是当时的外部验收项，不是这套调研本身。

本时代禁止：引入 soullink 依赖、提交第二张 model3、写新的参数重映射器、把 `V0_SUPPORTED_FORMATS` 扩成第二种格式。

## 6. 本时代的债，和明确留下的边界

做完才称得上这轮干净：

- 第 0 节的两态有测试。
- 出厂人设字符串没了。
- 活文档里的现行身份句保持「本地优先的皮套接入服务」（身份单已做，不要重做）。
- 发布说明写出下面这些边界。

这些**当时**留着，写进 `v0.2.5-rc.1` 的已知边界，本时代不改；2026-10-16/17 债务片后的状态逐条标注：

- `action_tx` 继续休眠。`action_request_is_dormant_not_delivered` 继续守。不删 core 的 `action/`、`performance/`，不改不变量 4 和 6。
- 只证明白模。缺参数丢 cue。
- `CHANGELOG.md` 约 1955 行的「桌宠」历史句。
- `docs/local-inference-setup.md` 里 local-llm 删除横幅之下的 PowerShell `netsh` 示例。不把它当步骤执行。
- `AGENTS.md` 变更历史里的「桌宠」。
- 拆 `supervisor.rs` / `engine.rs`、给 `AGENTS.md` 减肥、生产 `unwrap` 清点 —— **2026-10-16 `74b1ecb` 已做**（`supervisor.rs` 492 / `engine.rs` 236，>500 棘轮 42；清点见 `docs/audit/2026-10-16-unwrap-inventory.md`）。
- 静默错误 S-000–S-007 —— **2026-10-16 `74b1ecb` 已做**；S-007 的无 supervisor 分支 2026-10-17 补齐。
- CI 真 runner（D1）—— **2026-10-17 已做**：Nightly `39025484355`（headSha `b957775`）success；wasm 体积 —— **仍开**：本机无 `wasm-opt`，按 D8 停。
- 核心开发者页上的表情 / 动作调试面板。不在本时代搬家。
- 不挂回 `pet-desktop`、`wallpaper`、`local-llm`。

## 7. 每片怎么验

1. `git diff --stat` 只含这一片点名的文件。
2. 改过 Rust：`cargo fmt --all -- --check`，以及这一片点名的 `cargo test -p …`。S3 至少跑 `live2d-ai-runtime`、`live2d-ai-mod-director`、`live2d-ai-desktop` 里新测试所在的包，加上 `cargo test -p live2d-ai-core --lib`。不把没跑的全量 workspace 测试写成已绿。
3. 改过 Dart：`export PATH="$HOME/flutter/bin:$PATH"` 后 `cd shell/flutter && flutter analyze`，并跑点名的测试文件。
4. 改过文档：`cargo run -p xtask -- code-stats --check --only docs`。失败就缩短新文。不提高 `DOCS_BUDGET_LINES`（45000），不把文件挪进 `docs/audit/`。
5. S3 不点火、不开浏览器，除非执行人真的按第 4 节去为 `v0.2.5` 取证。取证结果写进发布说明，没看就写没看。

## 8. 回报

1. HEAD、提交哈希（若本片允许提交）、改了哪些文件。
2. S1–S4：完成 / 没做（原因）。
3. 第 7 节每条命令的退出码。
4. 导演开、导演关各用哪一个测试名证明。
5. 明确没做的：第二张 model3、`action_tx`、`v0.2.5` 正式 tag、推送（若认证失败）。
