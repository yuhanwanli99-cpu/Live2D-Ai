# 执行单：0.3.9 核心边界复位（2026-10-19）

> 交给执行 agent。一块功能，编号 `0.3.9`。不并进 `0.4.0` / `0.4.1`。
> `rc.z` 按代码拆，最后一个 `rc.(z+1)` 只清理。每个 rc：一次本地提交 + 一个本地附注 tag，不 push。
> `Cargo.toml` 仍是 `0.3.1`。不改 `.env`、`mods.json`、`live2d-ai.toml`。

## 0. 产品边界

核心只有三块：一路表演 LLM、二路导演（清洗 → 上屏 / TTS / 动作）、一个前端（渲染、动作、设置、对话）。

`local-tts-melo` 只拉起本机 TTS 进程。出声端点是 `[tts]`，可以换成云端 API。云端时表演不依赖这个 Mod。

多厂商 TTS/LLM 选择器是**核心增强**，留在主仓。现状（厂商目录 + `vendors.*` 快照）前后端都不满意。0.3.9 把它收成两层：

1. **云端 / 本地**二选一。LLM 与 TTS 各一个，缺省沿用今天：LLM `cloud`，TTS `local`（本机 Melo）。
2. **二级设置**，挂在当前这一档下面。LLM：地址、模型名、密钥变量名、思考开关。TTS：地址、音色、格式、采样率、声道、云端时的密钥变量名。不再有第三级「厂商名单 / 快照来回复制」。

拿掉仍能完成上面三块表演的，才算混进核心的扩展，迁出主仓。导演、热换、动作包、模型库、设置草稿属于表演闭环，留下。

## 1. 禁止

- 不删第二路导演，不关规则表，不复活 `action_tx`，不挂回已删 Mod。
- 不把 `[llm]` / `[tts]` 当前端点搬进 Mod。不新增第二套对话模型，不内置推理运行时。
- 皮套回合仍不调工具。A2A 迁走之后也不往句子、TTS、口型、`action_cue` 里插调用。
- 不手改本机 toml。旧文件里的 `vendor` / `vendors` 要能读进来；写回时不再写出这两键。
- 不提高 `DOCS_BUDGET_LINES`。不改 `AGENTS.md` 历史段、`tts-is-core.md` 历史段。
- 不 push。不提交 `official_live2d_samples.7z`。不要 `git add -A`。

## 2. 设置收成两层（核心）

后端真源是 `[llm].mode` 与 `[tts].mode`（`cloud | local`）。二级字段仍在这两个表里，仍走「保存并重载」。

删掉作为切换手段的 `vendor` 标签和 `vendors` 快照表（含虚拟 `melo` 快照、`vendor_modified`、一次 PATCH 把快照复制到当前段）。本地 TTS 的缺省地址仍由 `mode=local` 写入今天的 Melo 缺省（`127.0.0.1:8091` 这一组），不要另做一份厂商表。

旧 toml：`deny_unknown_fields` 不得因旧键启动失败。读入时忽略 `vendor` / `vendors`。下一次正常保存不再写回它们。不在本单里跑一遍把用户磁盘上的 toml 改掉。

前端：`llm_section.dart` / `tts_section.dart` 只留模式开关和该模式下面的字段。去掉厂商下拉、快照脏标记、`Tri` 厂商三态。`settings_controller.dart` 里对应草稿一起删。界面文案只用已有子集里的字。

`local-tts-melo`：`[tts].mode=local` 时仍可按今天的缺省拉起进程。`mode=cloud` 时核心只打 `[tts].base_url`，不要求这个 Mod 在跑，点火也不要等 `127.0.0.1:8091/models`。

## 3. 迁出主仓的扩展

**A2A** 从 `web_api/a2a_routes.rs` 迁到第二仓的新 Mod。新 id 用 `a2a`，crate `live2d-ai-mod-a2a`。缺省停用。不要复用已删除的 id。

宿主只留 loopback、Origin、`Content-Type` 这道门，以及「Mod 停用则不调用」。协议、任务表、上游收集放在 Mod 里。停用时 `GET /.well-known/agent-card.json` 与 `POST /api/v1/a2a` 不打 LLM（403 `mod_disabled` 或 404，两处同一套，写进测试）。启用后的线格式与现在一致：`message/send`、`tasks/get`，取消仍拒绝。密钥仍只在宿主按 `[llm].api_key_env` 查找，Mod 拿不到密钥值，响应里也不出现。

工厂数 6 → 7，`mod_count_is_six` 改成 7，并改断言名。`local-engine` 仍注册、界面仍隐藏。第二仓一次本地提交，不 push。

已经在第二仓的 Mod（外部事件、角色卡、语音输入、记忆、Melo 启动器、本地引擎）不要搬回主仓。外部事件的忙时合并留在宿主胶水里，行为不变。

## 4. 建议拆法

中间步可以合并，最后一步只清理。

- `v0.3.9-rc.1`：设置模型去掉 vendor 快照；旧 toml 仍能加载；云端 TTS 不依赖 Melo 进程。
- `v0.3.9-rc.2`：前端收成模式 + 二级字段。`flutter analyze` 与 `flutter test`。要让 `/app/` 看到新界面时才重编 web（`--base-href /app/ --no-web-resources-cdn`，再 prune）。
- `v0.3.9-rc.3`：A2A 进新 Mod，宿主只留门。工厂数改为 7。
- `v0.3.9-rc.4`：只清理。CURRENT §7 的「多厂商」段改成上面两层；短发布说明。不加功能。

每个含 Rust 的 rc 跑仓库门禁：`cargo test --workspace --all-targets`、`--doc`、`fmt --check`、`clippy -D warnings`、`rust-ratio`、`code-stats --check`。测试前 `unset` 代理变量。`code-stats` 若仍只红在既有的 `live2d_stage.dart` 844 行，写明即可，不要为它改棘轮，也不要顺手拆这个文件。

## 5. 交付

写明每个 rc 的 hash 与 tag、工厂数、旧 toml 夹具是否仍能启动、云端 TTS 是否不再等 8091、界面上已无厂商下拉。A2A 停用时确认没有打到上游。真 Ollama 仍是 3.0 遗留，本版不拉模型。
