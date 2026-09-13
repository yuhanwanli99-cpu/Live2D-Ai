# Contributing to Live2D-Ai

> **状态：2026-09-13（rc.3）重写。** 旧版描述的是 Python/Android 双端时代
> （`Live2D-Ai-pc/open-llm-vtuber`、`uv`、`npm`）——**那些目录早已归档**
> （tag `py-legacy` / 分支 `android-archive`），不要再按旧版操作。
> 现行主线、红线与休眠台账见 [`AGENTS.md`](./AGENTS.md)。

## 项目主线（一句话）

**Rust 核心（`crates/`）+ Flutter Web 前端（`shell/flutter/`）** 双主导；
唯一产品入口是 `--web` + `scripts/ignite.sh`（`/` 302 → `/app/`）。
增强能力只经 Mod 边界（`live2d-ai-mod-system`）接入。

## 开发环境与门禁

分工（WSL2 开发 / Windows 只开浏览器）与**本地必跑 vs CI 必跑**的完整表在
`AGENTS.md`。最小集：

```bash
# Rust 核心
cargo test --workspace --all-targets
cargo test --doc --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p xtask -- rust-ratio        # 门槛 95%

# Flutter 前端（前端/接口层豁免 rust-ratio）
cd shell/flutter && flutter analyze && flutter test

# 起服务 + 体检（WSL2 是唯一进程宿主）
./scripts/ignite.sh
./scripts/ignite.sh --check
```

## 提交前

- 一个里程碑一个提交；提交信息写清「**删除了什么**」「**为什么**」，并带上门禁数字。
- 改动 `shell/flutter/**` 后**必须重新构建**才生效：
  `flutter build web --release --base-href /app/ --no-web-resources-cdn`
  （`--no-web-resources-cdn` 不是可选项——缺省的 CDN 指向让断网即白屏）。
- 涉及 UI / wasm 的改动：重建 + 在浏览器里**肉眼**验一遍。**测试全绿 ≠ 界面是对的**。
- 发布顺序：本地提交 + tag → Windows 浏览器点火通过 → **再** `git push`。

## 不要提交

- 密钥（真源是 `.env`，**永不入库**）、聊天记录、上传的背景图、生成的音频、
  无再分发许可的模型文件、本地环境文件。
- 前端构建产物（`shell/flutter/build/`、`.dart_tool/` 已被嵌套 `.gitignore` 排除）。

## 红线（细节与理由见 `AGENTS.md`）

- 前端**不得**复制核心逻辑（状态机 / 动作仲裁 / LLM-TTS 协议），不得直接持有密钥。
- 链路错误必须**同时**进 `tracing` 与 WS `error` 帧，且两处的 `code` 是同一个字符串。
- 界面文案不得出现字体子集外的字符（门禁 `test/font_subset_test.dart`）。
- **动作系统不在产品路径上**：不得恢复 `RootEvent::Action` / director / 渲染面编舞；
  待机生命体征（`IdleState`）必须保留。
- **第二壳非主线**（egui 原生壳 / `--chat`）：不要往里加与 Flutter 重复的产品字段。
- 新能力必须经 `live2d-ai-mod-system` 接入，不得绕过 core 仲裁。

Contributions to project-owned code are accepted under AGPL-3.0-only. By submitting a
contribution, you confirm that you have the right to provide it under that license.
