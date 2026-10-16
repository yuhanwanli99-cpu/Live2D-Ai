# 计划：0.2.6 之后只清债务（2026-10-16）

> 功能冻结在 `v0.2.6-rc.1`（`767a286`）。不发布 `0.3.0`，不升版本。
> **2026-10-16 维护者改口**：从下一片起，做完要本地 commit + 附注 tag，仍然不推送。
> 下一片：[PROMPT-w1-lock-poison-2026-10-16.md](PROMPT-w1-lock-poison-2026-10-16.md)。
> 本文件是范围。现在不改产品代码。一片验收完，再写下一片的执行单。
> 不改 `.env`、`mods.json`、`live2d-ai.toml`。不提高 `DOCS_BUDGET_LINES`。
> 不复活 `action_tx`、已删 Mod、第二张 model3。不改 `AGENTS.md` 历史句的用字。
> 快捷键帮助、仓内 B 站示例、主动点头，不在本阶段。

## 0. 划掉，不要再做

表情调试 / 动作调试已经在 `v0.2.3-rc.2` 搬到导演卡片（`director_panel.dart` 里的 `DebugPanels`）。收口计划那句「不在本时代搬家」是不要再搬。本阶段也不搬。

「CI 从没在真 runner 上跑」已经过时。`v0.2.6-rc.1` 的 Release Build、Secret Scan、Build & Upload 都是绿的。还红的是夜间全量测试，见 D1。

## 1. 顺序

前一片没验收，不开始下一片。每片只动自己点名的文件。

### D1 夜间测试缺 ALSA

2026-10-15 的 Nightly「Rust Full Test」失败：<https://github.com/yuhanwanli99-cpu/Live2D-Ai/actions/runs/38697610847>。`alsa-sys` 要 `alsa.pc`，这个 job 在 `cargo` 之前没装 `libasound2-dev`。`pr-checks.yml` 的构建 job 同样没装。装了的是 `nightly.yml` 里后面那个 Web 托管 job，所以它是绿的。

做完：这两个构建步在编译前用和那个 Web job 相同的探测，缺了才装 `libasound2-dev` 和 `pkg-config`。不删 `cpal`。不改产品代码。

验收：`workflow_dispatch` 跑 Nightly，Rust Full Test 为绿，回报里写 run URL。没跑到不算做完。

### D2 S-000 和 S-002

台账 `docs/audit/2026-10-09-silent-errors/FINDINGS.md`。里面的行号按 2026-10-09 的树，先对现在的源码。台账正文不改。

- **S-000**：`wait()` 失败时把子进程句柄放回，和 `stop()` 失败同一条。句柄已经不在时，下一次停用不得回 200「已停用」。
- **S-002**：`PUT /api/v1/env` 把现有的 `ApplyStatus` 放进响应。只有 `Applied` 时界面才写「立即生效」。密钥写进 `.env` 成功、引擎没装上，两件事分开说。

验收：各一条会失败的测试。改了哪个包就测哪个包。S-002 若改了 Dart，再跑 `flutter analyze` 和点名的测试。

### D3 先拆 `supervisor.rs`，再修 S-001 / S-007

`supervisor.rs` 现在 1000 行，顶在豁免上。`>500` 棘轮是 44，不能涨。

先只搬家：拆成的每个文件 ≤500，`supervisor.rs` 自己也 ≤500。棘轮同一次收紧到新的个数。不改行为。

然后：热重载失败时，界面不得写「已保存并生效」。文件已经写上、引擎仍用旧配置，用现有文案分成两句。成功路径不改。S-007 的「手改 toml 语法错、界面一个字都没有」走同一条失败可见，不新做通知系统。

### D4 其余四条，一条一片

- **S-003**：`mods.json` 写盘失败时不得 HTTP 200，也不得 toast「已启用 / 已保存」。
- **S-004**：人设写回失败时不得报「已停用」，不得把 `registered` 清掉。
- **S-005**：某一句音频解码或播放失败，不得把界面改成「启用声音」。这一句仍可跳过。不新做错误页。
- **S-006**：丢掉的 Mod 事件要让现有的 `dropped` 计数增加，并出现在现有 `state_json`。不新增 WS 帧。

### D5 拆 `engine.rs`

现在 778 行。同样拆到每个文件 ≤500，棘轮再收紧。不改分句、TTS、口型。

### D6 `AGENTS.md` 减肥

现在 1218 行。从 `## 变更历史` 起整段**逐字**移出工作树，原处只留一行取回命令（`git show` 这版 tag 的路径）。不要改任何历史句，不要删「桌宠」。不要在 `docs/` 再留一份（预算不够）。文件开头的现行口径留在原处。

### D7 `unwrap` 只清点

生产代码里的 `unwrap` / `expect` 记到 `docs/audit/`（不算 45,000 行预算）。不批量改。清点里如果有一条会在请求路径上 panic，单独再开一片，不顺手改旁边的。

### D8 wasm 体积

先看本机有没有 `wasm-opt`。没有就停：不改 4.2 MiB 这个接受值，不为了体积改预设或功能。有的话再单独立片，回报里写减掉的字节数。

## 2. 明确不做

不升版本，不打 tag，不推送，除非维护者点名。不点火，不刷新页面。`target/debug` 已在 2026-10-16 清掉，下一轮编译会重新生成，不要再删一遍。

不修 FINDINGS 附录 1–10。不改「保存并重载」的成功语义。不把 `channels = 2` 改成单声道。不讨论用户名星号。

## 3. 文档

本文件落盘后 docs 预算是 44,903 / 45,000。执行片不要再写长计划。`code-stats --check --only docs` 失败就缩短，不提高预算。
