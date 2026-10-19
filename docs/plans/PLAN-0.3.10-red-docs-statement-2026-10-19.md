# 执行单：0.3.10 标红、声明、皮套分发、清债（2026-10-19）

> 交给执行 agent。一块版本 `0.3.10`，为进入 `0.4.0` 做准备。`0.4.0` 仍只做 Windows，本版不写窗口、不在本机假装验过 Windows。
> `rc.z` 自拆。最后一个 rc 只清债，不加功能。每个 rc：一次本地提交 + 一个本地附注 tag，不 push。
> `Cargo.toml` 仍是 `0.3.1`。不改 `.env`、`mods.json`、`live2d-ai.toml`。不提交 `official_live2d_samples.7z`。

## 0. 这一版四件事

1. **标红修复**：`code-stats --check` 与 `pytest tests/ -q` 恢复全绿。
2. **项目声明改为单一 AGPL-3.0**（本单里的「单一 A3」就是这一份许可证，不是第二套许可证，也不是把第三方代码改许可）。商业付费联系本人 QQ（号码只写在声明文件里，见 §3）。
3. **皮套入库与分发**：仓库继续只随克隆分发白模运行时必需文件；其它皮套只进用户本机模型库，不进 git。
4. **文档**：活文档与上面三条对齐。历史段、发布说明、`AGENTS.md` 沿革一段不改。

## 1. 禁止

- 不把 `l2d` 的 MIT OR Apache-2.0、第三方字体、渲染栈改成 AGPL。不改写 `LICENSE` 里的 GPL 正文。
- 不把白模写成「作者已授权再分发」。不新增大模型、不提交 `official_live2d_samples.7z`。
- 不提高 `DOCS_BUDGET_LINES`，不改 `RATCHET_DART_800`（保持 0）。用拆文件消掉 844 行，不要放宽棘轮。
- 不删导演，不复活 `action_tx`，工厂数保持 7。不把 A2A 搬回主仓。
- 不 push。不要 `git add -A`。

## 2. 标红

`shell/flutter/lib/live2d/live2d_stage.dart` 现在 **844** 行，门禁是单文件 ≤800 且 `RATCHET_DART_800 = 0`。按现有 part / 类边界拆出，行为不变。拆完 `flutter analyze` 与 `flutter test` 仍绿，`code-stats --check` 里这一项为 0。

`tests/melo_speakable_test.py` 仍指向主仓 `crates/live2d-ai-mod-local-tts/melo/serve.py`。文件实际在第二仓：

`/home/skystar/Live2D-Ai-mods/crates/live2d-ai-mod-local-tts/melo/serve.py`

测试改为读这个路径（相对主仓 `../Live2D-Ai-mods/...`）。第二仓不在时失败信息写明路径，不要再报主仓旧路径。`python3 -m pytest tests/ -q` 目标是 0 error。不改 `melo_speakable` 的删下划线行为。

## 3. 声明

项目自有代码的公开声明只有 **AGPL-3.0-only**。商业上想避开 AGPL 的源码义务，不构成另一份公开许可证；联系本人 QQ（号码只写在下列声明文件里），仍要单独书面协议。

改这几处的联系句，不要改 GPL 条文：

- `README.md` 的 License 段
- `COMMERCIAL-LICENSE.md`（现在写的是 “Contact the repository owner through GitHub”）
- `LICENSE-SCOPE.md` 开头那段
- `docs/architecture/CURRENT.md` 加一小节「声明」

QQ 只出现在这些声明里。不要写进日志、接口或 `.env`。

## 4. 皮套入库与分发

入库 = 已有的设置 → 模型库，加载用户本机 `model3`（0.3.5）。分发 = git 里随克隆带走的文件。

- 仓库里继续只跟踪白模渲染必需的那几个文件，条约仍以 `assets/models/bai/MODEL_LICENSE.md` 为准。
- 用户导入的皮套只留在本机模型库，不写入仓库，不做在线皮套商店，不新加下载端点。
- 在 `MODEL_LICENSE.md` 顶部加 2026-10-19 读法：入库与仓库分发是两件事。正文里的作者条约一段不改。

## 5. 活文档

以 `CURRENT.md` §1 为准，改仍把旧话当现状的句子。至少包括：

- `CURRENT.md` 里 0.3.8 那句「路由目前写在主仓 `web_api`，搬家另计」：A2A 已在 0.3.9 进 Mod `a2a`。
- 阶段单 `docs/plans/PROMPT-3.0-motion-first-2026-10-17.md` 里同样的旧句。
- `README.md` 若仍写六工厂、厂商目录、或把商业许可说成与 AGPL 并列的第二份公开许可证，改成与本单一致。

`AGENTS.md` 历史段、`tts-is-core.md` 与 `core-chain-baseline.md` 的旧正文保持不动（顶部读法已指向 CURRENT）。

## 6. 建议拆法

- `v0.3.10-rc.1`：两处标红。
- `v0.3.10-rc.2`：声明 + 皮套入库/分发读法 + 活文档旧句。
- `v0.3.10-rc.3`：只清理。短发布说明 `docs/releases/v0.3.10-rc.3.md`。确认 `code-stats --check` 与 `pytest tests/ -q` 都是 0。不再加功能。

含 Rust 的 rc 仍跑 `cargo test --workspace --all-targets`、`--doc`、`fmt --check`、`clippy -D warnings`、`rust-ratio`。rc.1 与 rc.3 必须带上 `code-stats --check` 和 `pytest tests/ -q`。改了 Dart 就跑 `flutter analyze` 与 `flutter test`。界面文案若没改，不必为声明重编 web。

## 7. 交付

写明每个 tag 的 hash、`code-stats` 与 pytest 的退出码、声明里 QQ 出现的文件、白模文件列表没有增加。`0.4.0` 写「未开始」。
