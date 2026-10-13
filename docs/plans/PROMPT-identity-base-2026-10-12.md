# 执行单：底座身份收口（2026-10-12）

> **交给另一个 agent 执行。** 照本文件改，不要重新评价产品。
> **权威评价**：`docs/research/评估-底座定位-Grok-2026-10-12.md`
> （Grok 4.7，2026-10-12 22:48 CST）。与本文件冲突时停下来问维护者，不要两边各改一版。
> **并行稿** `docs/research/评估-底座定位与工程质量-2026-10-12.md` 不是任务书。不要按它的 P1–P3 做。
> **版本**：不 bump，不写 release，不打 tag，不提交，除非维护者另说。
> **树**：`/home/skystar/Live2D-Ai`，分支 `main`。开工时先 `git status` 与 `git rev-parse --short HEAD`，写进回报。

## 0. 做完是什么样

活文档里，现行身份只有一句：这是本地优先的 **Live2D × AI 皮套接入**。桌宠、原生窗口、`--chat` 只出现在「已经删除」的记录里。

现行导演的事实只有一句：**驱动舞台的是 `action_cue`；`action_tx` 仍然休眠。** 「零投递 / 导演已删除」只保留在标明了日期的历史句里。

行为不变：不改协议、不改 Mod 注册表、不改测试断言、不新增第二套界面。

## 1. 禁止

做到任何一条就停，不要「顺便」：

- 不改 `.env`、`mods.json`、`live2d-ai.toml`、`Cargo.toml` / `pubspec.yaml` 版本号。
- 不挂回 `action_tx`、`pet-desktop`、`wallpaper`、`local-llm`，不新增 Mod。
- 不合并 `[performance]`、`director` 的 `staging_*`、`SentenceCleaner`。不裁决说话权（Q1）。
- 不删 core 的 `action/`、`performance/`，不改 `lib.rs` 不变量 4 和 6 的条文。
- 不引入第二套 UI，不做第二张 model3，不做参数重映射器。
- 不全库替换「桌宠」或「零投递」。`AGENTS.md` 变更历史、`docs/releases/`、`docs/plans/`、`docs/research/`（除本单点名的那一篇横幅）、`CHANGELOG.md` 保持原句。
- 不改 `crates/live2d-ai-mod-director/src/lib.rs`。它的头注已经写明 `action_cue` 是驱动、`action_tx` 休眠。该文件 969 行，顶在 1000 行豁免上。
- 不改 `action_tx_and_apply_settings_are_never_called` 以及任何把 `action_tx` 调用次数钉在 0 的测试。那些测试里的「零投递」指的是不调用 `action_tx`，这仍然正确。
- 不把注释改成会让 `>500` / `>1000` 棘轮变红的加长。改完跑门禁；变红就缩短注释，不放宽常量。
- 不把密钥、`.env` 的值、请求体写进回报。

## 2. 任务

按编号做。每一条都先读文件再改。改完在回报里写「旧句 → 新句」各一行。

### T1 · 现行身份句（四条，只改这一句）

| 文件 | 只替换这句 | 新句 |
| --- | --- | --- |
| `AGENTS.md` 约 449 行 | `本项目是本地优先的桌宠，不允许依赖 Google CDN` | `本项目是本地优先的皮套接入服务，不允许依赖 Google CDN` |
| `shell/README.md` 约 65 行 | 同上 | 同上 |
| `docs/architecture/font-fallback-offline.md` 约 18 行 | `对一个**本地优先**的桌宠，这条默认值有三个问题：` | `对一个**本地优先**的皮套接入服务，这条默认值有三个问题：` |
| `docs/local-inference-setup.md` 约 279 行 | `system_prompt = "你是桌面上的 Live2D 桌宠。用简短、口语化的中文回答。"` | `system_prompt = "你是一张 Live2D 皮套背后的对话角色。用简短、口语化的中文回答。"` |

`AGENTS.md` 其余「桌宠」不动，包括变更历史。

### T2 · 已删程序还被写成现行步骤

**`docs/local-inference-setup.md`**，`#### local-llm Mod 配置` 标题下、正文之前，插入一段引用：

> **2026-10-12**：`local-llm` 这个 Mod 和 crate 已于 2026-10-01 物理删除，只留在 tag `checkpoint/pre-d1-dormant`，禁止挂回。下面关于探活和端口转发的段落是删除前的记录，不能再当配置步骤。现在只写 `live2d-ai.toml` 的 `[llm]` 与 `[tts]`。

不要删掉后面的 PowerShell 示例，也不要改它去指向一个已不存在的 crate。

**`docs/headless-deploy.md`**，只动这三处，不要重写 Termux 章节：

1. 约 103–106 行，那句「只在 `--pet-mode` / `--model-smoke` / `--window-smoke` / `--chat`(桌宠模式) 被调用」。改成：这些原生模块和 `--chat`、`--pet-mode`、`--window-smoke`、`--model-smoke` 已于 2026-10-01 移出构建。现行入口只有 `--web`。
2. 约 160 行「AGPL 桌宠服务器」改为「本地皮套接入服务，无 Android UI」。
3. 约 215–217 行的表：`桌面窗口壳` 的说明改为「已于 2026-10-01 删除，不是待做项」。`桌宠点击穿透` 这一行改为「窗口点击穿透 | 不在产品路径 | 随原生壳删除」。不要新增穿透实现。

同文件约 200 行写着浏览器用 Web Audio 播放。那是过时的，但**本单不改**。回报里记一笔「未做：headless-deploy 仍写 Web Audio」。

### T3 · 活索引仍写「crate 还在 workspace」或「工厂数是 5」

这些是现行索引，不是发布说明。历史数字可以留，但必须紧跟一句 2026-10-01 / 2026-10-09 的现状。

| 文件 | 改什么 | 现状句 |
| --- | --- | --- |
| `docs/README.md` 约 31–32 行，ARCHIVED 台账那条 | 「crate 暂留 workspace 可编译可测」 | crate 已于 2026-10-01 物理删除，只在 tag `checkpoint/pre-d1-dormant` |
| `docs/README.md` 约 42–44 行，桌宠窗口 Mod 那条 | 同上 | 同上。用户手动的舞台/壳背景（`DisplayPrefs`）仍在，与这个已删 Mod 不是一件事 |
| `docs/architecture/mod-community-license.md` 约 79–85 行 | 三行「crate 暂留仓库」；「注册面现为 5 个工厂（`mod_count_is_five`）」 | 三行改为「crate 已于 2026-10-01 物理删除」。注册面改为 **6**：表中五个加上 `local-tts-melo`（缺省启用，只拉起进程，不写 `[tts]`）。断言名是 `mod_count_is_six`。不要改许可条款正文 |
| `docs/architecture/tts-is-core.md` 约 24–28 行那段 2026-09-14 引用之后 | 不要改引用里的「当时 5 个 / crate 暂留」 | 紧接一段：**2026-10-12**：wallpaper / pet-desktop / local-llm 的 crate 已物理删除。注册表现在是 6 个，多出来的是缺省启用的 `local-tts-melo`（只拉起外部进程）。上面的 5 与「暂留」是 2026-09-14 的原句 |
| `docs/architecture/directory.md` 约 74 行 | `mod_count_is_five`、工厂数恒为 5 | 改为 `mod_count_is_six`，现行 6 个；第六个是 2026-10-09 的 `local-tts-melo`。同段 68 行「零投递」已经标明是 2026-09-14 骨架期，**不要改那段历史句** |

`docs/README.md` 约 52 行标题仍是「零投递」，下一行已经有 2026-09-21 现状更正。只把标题改成「已注册、缺省停用；启用后以 `action_cue` 驱动舞台」。不要删掉现状更正那段。

### T4 · `core-contracts.md` 作废过期节

文件只有约 113 行。不要重写。在标题下、第一段之前插入：

> **2026-10-12 作废**：本文停在 2026-09-10。下面这些已经不在产品路径：winit 窗口、egui 设置面板、cpal 声卡、`SupervisorHandle::trigger_action`、命令必须进 core 仲裁。现行入口是 `--web` + Flutter `/app/` + wasm `/render`。架构真源改看 `core-chain-baseline.md` 与 `directory.md`。下文保留作历史，不要按它改代码。

表格里那一行「窗口（winit+wgpu）、音频（cpal）……egui」留着，因为横幅已经宣布它作废。不要删表。

### T5 · 会被当成不变量来读的注释

**`crates/live2d-ai-core/src/lib.rs`** 约 40 行，这一条：

旧：`动作序列的唯一驱动方 live2d-ai-mod-director 已删除`

改为两条短句：

- rc.2 那个**动作序列** director 已删除（取回命令维持原文）。
- 2026-09-14 起的同名 Mod 还在注册表里：它用 `action_cue` 驱动舞台，不调用 `action_tx`。core 的 `action/` 与 `performance/` 仍然没有驱动方。

不要改不变量 1–6 的列表，不要删 `#[doc(hidden)]`，不要动休眠 sender 那一条。

**`crates/live2d-ai-desktop/src/main.rs`** 约 59–64 行和约 80–81 行。历史句保留。在两处各加一行现状，不要把「零投递」从历史句里删掉：

> 现状（2026-10-12）：「零投递 / 不驱动动作」只描述 2026-09-14 骨架当时。现行舞台驱动是 `ModServices.cues` → WS `action_cue`。`action_tx` 仍然休眠。

80–81 行的行内注释同样加这句，保持一行能读完。改完 `main.rs` 仍须低于 500 行。

### T6 · 研究文档只加横幅

`docs/research/neko-ui-alignment-and-gaps.md` 文首元数据之后、`## 结论先行` 之前插入：

> **口径（2026-10-12）**：下文用桌宠清单（透明窗、点击穿透、托盘）给当时的渲染层打分。那不是本仓库的现行验收口径。现行产品是 Live2D × AI 皮套接入（`--web` + Flutter `/app/`）。没有穿透窗不是身份级缺口。下文原句保留。

不要改第 12 行那句「算不算桌宠」。横幅负责否定它。

### T7 · 对照索引，不新写测试

新建 `docs/architecture/base-conformance.md`，80 行以内。只列已经存在的测试名，并写明它们钉住的事实。不要新加 `#[test]`，不要复制断言。

必须点名：

- `crates/l2d-wasm-demo/src/mouth.rs`：`real_speech_rms_maps_to_visible_mouth_open`，以及非有限值回 0 的那个测试。事实：线性 RMS → dBFS → `ParamMouthOpenY`。
- `crates/l2d/src/pose_stack.rs` 里已有的层优先级测试（`base` < idle < input < physics < `final_override`）。打开文件核对真实函数名再写，不要发明测试名。
- `crates/l2d-wasm-demo/src/preset/tests/fields.rs` 与 `fields_map_tests.rs`：预设与导演 cue 不得写 `ParamMouthOpenY`。
- 句界在 `live2d-ai-runtime` 的对话引擎，不在播放端猜。点一个已有测试名；找不到就写「本索引未点名，不要为了索引新写测试」。

文首写：这是索引，不是第二套实现，也不是新的协议真源。协议仍以 `performance-protocol-v1.md` 与 `core-chain-baseline.md` 为准。

在 `docs/DOC-MAP.md` §1 加一行指向它，备注「索引，非契约真源」。在 `docs/README.md` 架构节加一条同样的话。

### T8 · 地图

`docs/DOC-MAP.md` §1 已有权威评价那一行。不要改它的「以 Grok 稿为准」。若你新增了 conformance 索引，只加行，不改优先级说明。

## 3. 做完怎么验

1. `git diff --stat`。预期只有本单点名的文件，加上 `base-conformance.md`。没有 `shell/flutter/lib`，没有 `Cargo.toml`。
2. 下面两条检索的结果写进回报：

```bash
# 现行身份句必须已经不在这些文件里
grep -n '本地优先的桌宠\|你是桌面上的 Live2D 桌宠' \
  AGENTS.md shell/README.md \
  docs/architecture/font-fallback-offline.md \
  docs/local-inference-setup.md

# 全库仍会命中。回报里说明每一条是历史句，而不是漏改的现行身份
grep -n '桌宠' AGENTS.md shell/README.md docs/architecture/font-fallback-offline.md
```

第一条应当没有输出。第二条允许 `AGENTS.md` 变更历史里的命中；若 `shell/README.md` 或字体文档仍命中，就是没做完。

3. 注释改动过 Rust 的话：

```bash
cargo fmt --all -- --check
cargo test -p live2d-ai-core --lib
cargo test -p live2d-ai-desktop --bin live2d-ai-desktop mod_count_is_six
```

`mod_count_is_six` 这条如果测试名过滤失败，改跑该二进制里名字含 `mod_count` 的测试，不要改断言去凑命令。

4. 文档门禁：

```bash
cargo run -p xtask -- code-stats --check --only docs
```

失败就缩短 `base-conformance.md` 或本轮新加的横幅。不要提高 `DOCS_BUDGET_LINES`，不要把文件挪进 `docs/audit/`。

5. 不跑 `flutter test`（没有 Dart 改动）。不跑全量 `cargo test --workspace`，除非上面三条里有失败而且你怀疑是自己引起的。不点火，不开浏览器。

## 4. 回报格式

用这个顺序，短：

1. HEAD、改了哪些文件。
2. T1–T8 每条：完成 / 跳过（原因）。
3. 第 3 节两条 grep 的输出。
4. fmt、core 测试、`mod_count`、docs 门禁的退出码。
5. 明确没做的：Q1、第二模型、`action_tx`、版本号、`director/src/lib.rs`、headless 文档里的 Web Audio 句。

## 5. 本单故意不做（下一张单，须维护者先说话）

> **2026-10-13 停止执行。** 后续只看 [`PLAN-0.2-closeout-2026-10-13.md`](PLAN-0.2-closeout-2026-10-13.md)。

- 裁决 Q1，以及让 `[performance]` 与 `director` 只留一个 cue 写入者。
- `SentenceCleaner` 保持只做 `display` / `speech`。不要把它并进表演。
- 第二张 model3、缺参数时的重映射。
- 生产 `unwrap` 清点、拆 `supervisor.rs` / `director/lib.rs` / `engine.rs`、给 `AGENTS.md` 减肥、CI 真 runner。
