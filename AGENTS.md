# Live2Dai 项目 AGENTS.md（仅项目专属差异）

> 全局权威规则在 `~/.pi/agent/spoq-templates/AGENTS.md` + `SOP.md`，本文件只含 Live2Dai 项目专属差异。冲突时以本文件为准。

## 项目专属背景（供主代理参考，非规则）

- Live2Dai 是 Live2D + AI 语音/形象皮套人主播的 Android/PC 双端项目，目标是比 N.E.K.O 等同类产品门槛更低，做到"动动手指即可配置"的全平台零门槛体验。跨平台一致性优先于跨端实时同步（当前不做云端配置同步）。
- 双端结构：`Live2Dai-Android/`（Kotlin/Compose）+ `Live2Dai-pc/`（Python/open-llm-vtuber）+ `shared/` + `tts/`。
- 项目已有级1/级2/级3 分级体系，定义见 `docs/Phase-0-architecture.md`（ROMA 2026 标准：每层选最优方案+一个验证过的降级方案+可观测性）。
- 项目内有大量前期研究文档（`docs/core-fusion-research.md`、`docs/NEURO_LIVE2D_RESEARCH.md`、`docs/benchmark-neko-vs-neurosama.md` 等）——子代理调研时先读这些避免重复取数。

## 项目专属覆盖（目前为空）

<!-- 未来 Live2Dai 需要与全局不同的行为时在此追加 -->

## 变更历史

- **2026-08-08**：v10 重写——删除旧状态机引用（agent-loops/spoq-state.schema.md 已废），对齐全局三级路由 + LARGE 完整状态机。
