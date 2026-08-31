# Live2D-Ai 项目说明（AGENTS.md）

> 本文件是项目级通用说明，供任何 CLI agent（pi / Claude Code / Hermes 等）读取。
> 状态：2026-08-31 按 Rust 主线重写（原版描述 Android/Python 双端，已归档）。

## 项目背景

- 文档索引见 `docs/README.md`；核心契约与目录约定见 `docs/architecture/core-contracts.md` 与 `docs/architecture/directory.md`。
- Live2D-Ai 是 **Rust 桌宠**：Live2D + AI + TTS 桌面应用，主仓库为**最小最简核心链路**
  （TTS + LLM + Live2D + Web UI），验证「文本/语音 → LLM → 表情/口型/动作」闭环。
- **唯一主线** = Rust workspace（`crates/`）：`l2d` / `live2d-ai-core` / `live2d-ai-runtime` /
  `live2d-ai-desktop` / `l2d-wasm-demo` / `xtask`（+ 节点 E 新增 `live2d-ai-mod-system`）。
- `Live2D-Ai-pc/`（Python）已归档（tag `py-legacy`）；`Live2D-Ai-Android/` 已归档
  （远端 `android-archive` 分支，见 `ANDROID_ARCHIVE_POINTER.md`）。
- 主仓库保持 **95-99% Rust**（xtask `rust-ratio` 门禁默认 95%）。
- 增强能力通过 **Mod 边界**隔离：`live2d-ai-mod-system` trait 注册中心，
  3 个 Mod（external-input / pet-desktop / director）为 workspace crate，默认不启用。

## 开发约定

- **门禁**（提交前必须）：`cargo test --workspace --all-targets` + `cargo test --doc --workspace`
  + `cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets -- -D warnings`。
- **源码 ≤500 行**（豁免 ≤1000 需头注理由）；测试文件 ≤800 行。
- **Rust 占比** `cargo run -p xtask -- rust-ratio`（门槛 95%）。
- **密钥安全**：密钥不进 GET/日志/WS/导出；loopback-only；mutating 需 `application/json`。
- **Mod 边界**：Mod 必须通过 `live2d-ai-mod-system` 接入，不得绕过 core 仲裁。

## 变更历史

- **2026-08-31**：按 Rust 主线重写（移除 Android/Python 双端描述）。
- **2026-08-09**：重写——移除 pi 专属路径引用，保留项目级通用信息。
