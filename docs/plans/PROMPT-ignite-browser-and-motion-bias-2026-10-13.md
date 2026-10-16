# 执行单：等本地 TTS 再给地址，并给导演一个表演倾向（2026-10-13）

> 交给工作 agent 执行。本单就是范围。不要再调研，不要扩大。
> 树：`/home/skystar/Live2D-Ai`，分支 `main`。写单时点火脚本无未提交改动。
> 不要改 `.env`、`mods.json`、`live2d-ai.toml`。不要启动或杀掉 `ignite.sh` / `live2d-ai-desktop`（用户机器上可能有进程）。
> 不要 commit、不要 tag、不要 push、不要改版本号。

## 0. 做完是什么样

1. `[tts].base_url` 的主机是 `127.0.0.1`、`localhost` 或 `::1` 时：桌面进程拉起本地 TTS 之后，`GET {base}/models` 返回 200，脚本才打印 `http://127.0.0.1:18080/app/`，并用默认浏览器打开它。在这之前终端只有「正在等」。
2. TTS 不是本机，或没配 TTS：本机页面 `GET /app/` 返回 200 后就打印地址并打开浏览器。
3. 全部就绪前不打开浏览器。TTS 超时或页面没起来：浏览器不打开。
4. 导演卡片上多一个可见下拉「表演倾向」，三档：克制 / 平常 / 主动。缺省是平常。保存后要重新点火才进清洗请求。
5. 克制 = 今天的提示词，模型交回 `cues:[]` 仍是成功、不回退规则表。平常只把提示词写得更愿意选一个。主动才在模型交回空数组、且规则表对**这一句原文**有动作时，用规则表。

## 1. 禁止

- 不新增 LLM 调用。不改 `CLEAN_SYSTEM_PROMPT`（动作门关着的两字段提示）。不关思考的请求体不要动（`thinking.type = disabled`，`temperature` 仍是 0）。
- 不改嘴型、待机、`[performance].enabled` 缺省、`action_tx`、`mod_count_is_six`。
- 不把 `motion_bias` 放进 `hiddenKeys`。不在 `DirectorPanel.build` 再画一块重复控件。
- `crates/live2d-ai-mod-director/src/lib.rs` 现 969 行，改完必须 ≤1000。`supervisor.rs` 现 996 行，改完必须 ≤1000，能不改就一行不加。
- 新的 `crates/*/src/**/*.rs` ≤500 行。不要让 `code-stats` 的 `>500` 计数从 44 变成 45，不要出现 `>1000`。
- 不提高 `DOCS_BUDGET_LINES`。不改 `CHANGELOG.md`、发布说明、`docs/audit/**`。
- 界面文案只用字体子集里已有的字。`font_subset_test` 变红时改文案，不要重新生成字体。

## 2. 任务

### T1. `scripts/ignite.sh`

现状：约 259 行 `exec ./target/debug/live2d-ai-desktop --web ...`，在这之前就打印点火入口。`--help` 是 `sed -n "2,18p"`。

改成：

- 读 toml 时用三行输出：`llm_base`、`tts_base`、`tts_local`（`1`/`0`）。主机名用 `urllib.parse.urlparse`。`127.0.0.1` / `localhost` / `::1` 为 `1`。不要用 `print(a, b, c)`，空 TTS 会被 `read` 吃掉。
- 本机 TTS：预检不要报「未就绪，继续」。印「由本次点火拉起；GET /models = 200 之后才给页面地址」。云端 TTS 仍走现有 `probe`。
- 服务放后台，脚本前台 `wait`。`INT`/`TERM` 时若 pid 还在就 `kill`。不要 `exec`。不要在 `EXIT` 里再杀一次已经退出的进程。
- 先等 `GET http://127.0.0.1:${PORT}/app/` = 200，超时 90 秒。进程先死就停。
- 仅当 `tts_local=1`：再等 `GET ${TTS_BASE%/}/models` = 200，超时 600 秒。每 15 秒一行「仍在等待」。
- 两步都成功：打印现在这块横幅（入口、渲染面、Mod 状态、自测清单）；本机 TTS 再加一行 `本地 TTS：`。然后打开浏览器。
- 打开方式：`LIVE2D_AI_NO_BROWSER=1` 只打印。WSL（`/proc/version` 含 microsoft）且有 `powershell.exe`：`Start-Process` 那个 URL，放后台。否则 `xdg-open`。都没有就告诉用户手动打开。
- 页面没好，或本机 TTS 超时：不打印成功横幅，不打开浏览器。TTS 超时的错误行可以带上 URL，供用户自己开。
- 文件头改成这五步。`--help` 的行号要盖住用法和那句「前台等待，Ctrl+C 停服务」。
- `bash -n scripts/ignite.sh` 必须过。不要真的执行点火。

### T2. 活文档里「exec 进服务、用户自己开浏览器」

只改现在进行时的句子，不改变更历史：

- `README.md`、`README.zh-CN.md` 的快速开始和点火节。
- `AGENTS.md`「工作区与点火纪律」里那句「它会 exec 进服务」和 Windows「只负责开浏览器」。改成：脚本等到本机 TTS 的 `/models` 为 200（云端或未配置则等到 `/app/` 为 200）后打印地址，并用默认浏览器打开；脚本不退出，体检仍另开终端 `--check`。

### T3. 表演倾向

现状锚点，先读再改：

| 事实 | 位置 |
| --- | --- |
| 动作门提示词写「选 0~2 个，选不出就给空数组」 | `cleaning.rs` `motion_system_prompt` |
| `Some([])` = 成功不动；只有 `None` 才规则回退 | `conversation/engine.rs` `sentence_texts` 约 728 行 |
| 规则表看的是传入的文本；问号只有 `？` `?`；中性 = 空 | `director/src/presets.rs` `rule_cues_for_sentence` |
| 导演配置在启动和 toml 热重载时读一次 | `performance_wiring.rs` `DirectorTakeover::from_manifest` |
| 产品面隐藏 8 个键；下拉由通用表单画 | `director_panel.dart` `hiddenKeys`；`ModSettingField::Select` |
| 卡片测试把 spec 手写成 8 字段 | `shell/flutter/test/director_panel_test.dart` |

做法：

- 新文件 `crates/live2d-ai-mod-director/src/motion_bias.rs`。枚举 `restrained` / `normal` / `eager`。未知或空 = `normal`。三个方法：`as_str`、`cue_clause(&self, allowed: &str) -> String`、`fallback_on_empty(&self) -> bool`（只有 `eager` 为 true）。
- `cue_clause` 文案锁死，不要改写：
  - 克制：`只根据用户给你的这一句原文，从下面这组 id 里选 0~2 个动作预设，不要根据别的句子或上下文，选不出就给空数组 []：{allowed}`
  - 平常：`只根据这一句原文，从下面这组 id 里选 0~2 个。这一句是问句（含？或?），或能看出高兴、难过、惊讶、摇头、点头、朝左看、朝右看时，必须选 1 个最贴切的 id。看不出就给空数组 []，不要为了有动作而硬选：{allowed}`
  - 主动：`只根据这一句原文，从下面这组 id 里选 1~2 个。只有整句是平铺直叙、没有情绪也没有问号时才允许空数组 []：{allowed}`
- `motion_system_prompt` 增加一个 bias 参数，只把 `cues：` 那一句换成 `cue_clause`。`display` / `speech` 两句保持原样。动作门关着仍用 `CLEAN_SYSTEM_PROMPT`。
- `SentenceCleaner` 记住 bias。`with_motion` 增加这个参数。`empty_cues_fall_back()` 转给 `fallback_on_empty`。
- `sentence_texts`：`Some(ids)` 且 ids 为空、并且 `cleaner.empty_cues_fall_back()` 为真时，改走已有的 `fallback(raw)`。其它成功解析仍用模型的 ids，包括空数组。失败仍是 `None` → 规则表。不要打原文。主动档真的改用规则表时，`tracing::info` 只打 `sentence_seq` 和 cue 条数。
- `DirectorTakeover` 加 `motion_bias`。`from_manifest` 读 `mods.director.config.motion_bias`。缺键 = `normal`。
- `build_sentence_cleaner` 在现有 `with_motion(...)` **同一行**传入 bias。`supervisor.rs` 行数不得上升。现有那条 `二路按句清洗已装配` 可在同一 `info!` 上加 `bias` 字段。
- `director_settings_spec` 增加一个 `Select`，不要写在 `lib.rs` 里展开。`motion_bias.rs` 提供 `setting_field() -> ModSettingField`，`lib.rs` 只 `push` 一次。键 `motion_bias`，label `表演倾向（重新点火后生效）`，选项值 `restrained`/`normal`/`eager`，显示 `克制`/`平常`/`主动`，`default: Some("normal")`。`DirectorConfig::from_value` 同样解析，缺省 `normal`。`lib.rs` 改完 `wc -l` ≤1000，否则把新增行再挪出去。
- 前端：`hiddenKeys` 仍是原来那 8 个，不要加入 `motion_bias`。通用表单会画出这个下拉，不要在面板里再写一个。更新 `director_panel_test.dart` 里手写的 8 字段 spec，加上这一字段；隐藏键断言仍是 8。若有测试说卡片上除那句话以外没有任何控件，改成允许这个下拉。`mods_section_test` 里「隐藏键原样保留」不要被新字段冲掉。

## 3. 验收

跑这些，不要跑全仓，不要点火：

```bash
bash -n scripts/ignite.sh
env -u http_proxy -u https_proxy -u HTTP_PROXY -u HTTPS_PROXY -u ALL_PROXY -u all_proxy \
  cargo test -p live2d-ai-mod-director --lib motion_bias
env -u http_proxy -u https_proxy -u HTTP_PROXY -u HTTPS_PROXY -u ALL_PROXY -u all_proxy \
  cargo test -p live2d-ai-runtime --lib motion_system_prompt
env -u http_proxy -u https_proxy -u HTTP_PROXY -u HTTPS_PROXY -u ALL_PROXY -u all_proxy \
  cargo test -p live2d-ai-desktop --bin live2d-ai-desktop motion_bias
cd shell/flutter && flutter test test/director_panel_test.dart test/font_subset_test.dart
cargo run -p xtask -- code-stats --check --only lines
```

测试名可以对上你新加的名字；过滤词用 `motion_bias` 和 `motion_system_prompt`。断言至少包括：缺键 = 平常；垃圾字符串 = 平常；克制和空数组不回退；平常和空数组不回退；主动 + 含 `？` 的原文回退到规则表（非空）；主动 + 没有问号也没有情绪词的原文仍为空。

回报：改了哪些文件、`wc -l`（`lib.rs`、`supervisor.rs`、`cleaning.rs`、`performance_wiring.rs`、`engine.rs`、`motion_bias.rs`）、上面每条命令的退出码。没跑的写明没跑。

## 4. 明确不做

不改提示词以外的选角逻辑，不把用户输入拿去打五类情绪，不让表情跨轮常驻，不改动作幅度滑条，不改 cue 的 2000ms，不打开 `[performance]`。
