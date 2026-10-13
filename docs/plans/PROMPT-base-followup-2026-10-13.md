# 执行单：身份收口余项（2026-10-13）

> **交给下一个 agent 执行。** 上一单 [`PROMPT-identity-base-2026-10-12.md`](PROMPT-identity-base-2026-10-12.md)
> 已验收通过（HEAD 仍是 `f5d4184`，改动未提交）。本单只收那次验收点名的漏，
> 加上上一单写明「未做」的 Web Audio 句。
> **权威评价**：[`docs/research/评估-底座定位-Grok-2026-10-12.md`](../research/评估-底座定位-Grok-2026-10-12.md)。
> 与本文件冲突时停下来问维护者。
> **版本**：不 bump，不写 release，不打 tag，不提交，除非维护者另说。
> **树**：`/home/skystar/Live2D-Ai`，分支 `main`。上一单的未提交改动留着，不要还原。

## 0. 做完是什么样

活文档和活注释里，播放路径写成「浏览器 `<audio>` + WAV」，不再写成 Web Audio，
也不再把已删的桌面窗口壳说成将来要做的 P3。一致性索引里，切句测试和音频句界测试各说各的事实。

行为不变：不改协议、不改 Mod 注册表、不改测试断言、不新增测试。

## 1. 禁止

做到任何一条就停：

- 不改 `.env`、`mods.json`、`live2d-ai.toml`、`Cargo.toml` / `pubspec.yaml` 版本号。
- 不挂回 `action_tx`、`pet-desktop`、`wallpaper`、`local-llm`，不新增 Mod。
- 不裁决 Q1，不合并 `[performance]` 与 `director`，不改 `SentenceCleaner`。
- 不删 core 的 `action/`、`performance/`，不改 `lib.rs` 不变量 4 和 6。
- 不改 `crates/live2d-ai-mod-director/src/lib.rs`（969 行，顶在 1000 行豁免上）。
- 不全库替换「桌宠」。`AGENTS.md` 变更历史、`docs/releases/`、`docs/research/`、`CHANGELOG.md` 保持原句。
- 不改测试夹具里的人设字符串（`settings_api_test.dart`、`settings_controller_test.dart`）。那是往返 JSON，不是界面文案。
- 不把 `local-inference-setup.md` 里 `local-llm` 的 PowerShell 示例删掉。上一单的引用横幅已经声明它是删除前记录。
- 不把注释改到让 `>500` / `>1000` 棘轮变红。变红就缩短，不放宽常量。
- 不提高 `DOCS_BUDGET_LINES`，不把文件挪进 `docs/audit/`。

## 2. 本单任务（上一单漏点 + 它写明未做的一句）

按编号做。每一条先读文件再改。回报里写「旧句 → 新句」。

### T1 · 索引把两件事实写成了一件

`docs/architecture/base-conformance.md` §4 标题是「句界由引擎给出，不在播放端猜」，
点名的却是切句器测试（48 字硬切、标点、清洗不改句读）。那些测试是真的，标题过大。

把 §4 拆成两小节，不要新写 `#[test]`：

1. **切句**：保留现有四个测试名。标题改成「切句只按句读，不按字符位置」。事实句保留「48 字位置切分已退役」。
2. **音频句界**：点名已有测试
   `crates/live2d-ai-desktop/src/web_api/ws/audio_tests.rs` 的
   `sentence_boundaries_are_exactly_one_start_and_one_end`。
   事实：一句音频只有一片 `start`、一片 `end`，由引擎的 `first_chunk` / `final_chunk` 给出，
   不在播放端按 epoch 或按块去猜。

§5「reducer 未点名」留着。全文仍须低于 80 行。

### T2 · 窗口壳被说成还要做

`docs/headless-deploy.md` 约 219–220 行，表下这段：

> 桌面窗口壳 (F4/G1) 仍需图形环境，属于 P3 阶段。

改成：桌面窗口壳已于 2026-10-01 删除，不是后续阶段。见 `docs/architecture/ARCHIVED-native-shell.md`。
不要重写 Termux 章节。表内「已于 2026-10-01 删除，不是待做项」那一行不要再改。

### T3 · 播放路径仍写 Web Audio 和已删的 JS 前端

同一文件，只改这三处的播放描述，不要改环境变量名：

| 约行 | 现在 | 改成 |
| --- | --- | --- |
| 81 | 浏览器使用 Web Audio API 播放 | 浏览器把一句 PCM 封成 WAV，用 `<audio>` 播放 |
| 84 | 由前端 `/static/app.js` 中的播放器消费 | 由 Flutter `shell/flutter/lib/audio/audio_player.dart` 消费。`/static/app.js` 已于 2026-09-11 删除 |
| 200、213 | Web Audio | `<audio>` + WAV |

`LIVE2D_AI_WS_AUDIO=1` 这个变量名保留。不要把服务端 `cpal` 声卡链路写成已经删除；headless 上它仍然是 dry-run。

### T4 · 活注释里的现行身份句

上一单的检索范围没盖到这两处。各只替换这一句，离线约束留下：

| 文件 | 旧句 | 新句 |
| --- | --- | --- |
| `shell/flutter/lib/ui/theme.dart` 约 29 行 | `对一个**本地优先的桌宠**，那等于「断网即豆腐块」。` | `对一个**本地优先的皮套接入服务**，那等于「断网即豆腐块」。` |
| `crates/live2d-ai-desktop/src/web_api/flutter_app.rs` 约 9 行 | `本项目是本地优先的桌宠，不允许依赖 Google CDN。` | `本项目是本地优先的皮套接入服务，不允许依赖 Google CDN。` |

`flutter_app.rs` 现为 384 行，改完仍须低于 500。`theme.dart` 现为 460 行，只改这一句。

### T5 · CI 步骤标题还是旧棘轮

`.github/workflows/pr-checks.yml` 约 82 行，步骤名仍写
`live2d-ai-desktop ≤ 34；PLAN 目标 22`。
上面注释里的「立档时 58/4/34/7；现棘轮 >500=48」也旧了。

只改注释和步骤名，命令保持 `code-stats --check --only deps`：

- 步骤名改为：`Cargo dependency budget (live2d-ai-desktop ≤ 23；PLAN 目标 22)`
- 注释里的「现棘轮」改为与 `xtask` 一致：`>500` = 44、`>1000` = 0、Dart `>800` = 0、desktop deps = 23。
  改之前打开 `xtask/src/code_stats/mod.rs` 核对 `RATCHET_*`，不要抄本文件里的数字如果它们已经又变了。

### T6 · 点火清单仍像现行步骤

`docs/verification/ignition-core-loop.md` 是 2026-09-10 的清单。文首仍写 CosyVoice `127.0.0.1:8080`、
「动作/表演本轮不做」，§4 把「桌宠窗口」列在后续。不要重写正文。在标题下插入：

> **2026-10-13 作废**：本文是 2026-09-10 的点火记录。出厂 TTS 现为 Melo `127.0.0.1:8091`；播放走 `<audio>` + WAV；桌宠窗口已删除。现行点火看 `scripts/ignite.sh` 与 `docs/releases/v0.2.4-rc.1.md`。下文保留作当时记录。

## 3. 做完怎么验

1. `git diff --stat`。预期只有本单点名的文件。没有 `Cargo.toml`。没有 `director/src/lib.rs`。
2. 检索，第一条应当没有输出：

```bash
grep -n '本地优先的桌宠\|你是桌面上的 Live2D 桌宠' \
  shell/flutter/lib/ui/theme.dart \
  crates/live2d-ai-desktop/src/web_api/flutter_app.rs

grep -n 'Web Audio\|/static/app.js' docs/headless-deploy.md
```

第二条允许零命中。若仍命中，回报里贴出那一行，并说明是没改到还是另有一处本单没点名。

3. 改过 Rust 注释或 workflow 注释时：

```bash
cargo fmt --all -- --check
cargo run -p xtask -- code-stats --check --only docs
```

4. 改过 `theme.dart` 时：`cd shell/flutter && flutter analyze`。
   不跑 `flutter test`（没有断言变化）。不跑全量 `cargo test --workspace`。不点火。

## 4. 回报格式

1. HEAD、改了哪些文件。
2. T1–T6：完成 / 跳过（原因），每条旧句 → 新句一行。
3. 第 3 节两条 grep 的输出。
4. fmt、docs 门禁、`flutter analyze` 的退出码。
5. 明确没做的：Q1、第二张 model3、`action_tx`、版本号、测试夹具里的「桌宠」人设、`CHANGELOG.md`。

## 5. 接下来的计划（本单不做，须维护者先说话）

> **2026-10-13 停止执行。** 维护者已裁定导演两态、人设删除、版本由计划控制。
> 下一轮只看 [`PLAN-0.2-closeout-2026-10-13.md`](PLAN-0.2-closeout-2026-10-13.md)。下表留作当时记录。

上一单 §5 原样留下。权威稿 §6 的建议只登记，不执行。

| 顺序 | 内容 | 为什么现在不动 |
| --- | --- | --- |
| 1 | 一句 Q1：默认谁写 cue | `[performance]` 与 `director` 都能写。权威稿的建议是：默认仍由导演规则层对**用户输入**出 cue；`[performance]` 保持默认关，且不要和导演同时写。这句话没落下之前，不合并路径，不新增表情 |
| 2 | `SentenceCleaner` 继续只做 `display` / `speech` | 它管哪几个字上屏、哪几个字送 TTS，不产 `body` / `head` / `expression`。不要把它算进「第二表演 LLM」 |
| 3 | core 的 `action/`、`performance/` 继续休眠 | 删除要改不变量 4 和 6，单独开一轮。现有「`action_tx` 不被投递」的测试继续守 |
| 4 | Q1 收成一个写入者之后，再用第二张 model3 证明换皮 | 现在缺参数即丢弃该条 cue。通用皮套还没证明到表情和头颈。不在 Q1 之前做重映射器 |
| 5 | 再往后，不当「是不是底座」的门槛 | 生产 `unwrap` 清点、拆 `supervisor.rs` / `director/lib.rs` / `engine.rs`、给 `AGENTS.md` 减肥、CI 真 runner、wasm 体积 |

研究文档和发布说明里的「桌宠」是历史用词。不整篇改写。`neko-ui-alignment-and-gaps.md` 的口径横幅已经加上，正文第 12 行保留。
