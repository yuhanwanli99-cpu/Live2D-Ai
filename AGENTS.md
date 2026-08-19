# Live2D-Ai 项目说明（AGENTS.md）

> 本文件是项目级通用说明，供任何 CLI agent（pi / Claude Code / Hermes 等）读取。
> pi 的 SPOQ 编排规则在 pi 专属位置（`~/.pi/agent/spoq-templates/`），不影响本文件。

## 项目背景

- 文档索引见 `docs/README.md`；核心契约与目录约定见 `docs/architecture/core-contracts.md` 与 `docs/architecture/directory.md`。
- Live2D-Ai 是 Live2D + AI + TTS 的 Android/PC 双端最小验证项目，验证「文本/语音 → LLM → 表情/口型/动作」闭环，目标是比 N.E.K.O 等同类产品门槛更低，做到"动动手指即可配置"的全平台零门槛体验。跨平台一致性优先于跨端实时同步（当前不做云端配置同步）。
- 双端结构：`Live2D-Ai-Android/`（Kotlin/Compose）+ `Live2D-Ai-pc/`（Python/open-llm-vtuber）+ `shared/`。
- 项目已有级1/级2/级3 分级体系，定义见 `docs/architecture/Phase-0-architecture.md`（ROMA 2026 标准：每层选最优方案+一个验证过的降级方案+可观测性）。
- 项目内有大量前期研究文档（`docs/research/core-fusion-research.md`、`docs/research/NEURO_LIVE2D_RESEARCH.md`、`docs/research/benchmark-neko-vs-neurosama.md` 等）——调研时先读这些避免重复取数。

## 变更历史

- **2026-08-09**：重写——移除 pi 专属路径引用（SPOQ 规则归 pi 专属位置），本文件只保留项目级通用信息，任何 CLI agent 可安全读取。
