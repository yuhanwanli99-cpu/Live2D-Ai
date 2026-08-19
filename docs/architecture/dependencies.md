# Live2D-Ai 依赖与许可清单（地基）

> 目标：把关键依赖、版本、许可、用途固定下来，方便后续开源/审计/升级。
> 详细许可分析见 `docs/research/license-report.md` 与根 `CREDITS.md`。

## Android 端

| 依赖 | 版本/来源 | 用途 | 许可 |
| --- | --- | --- | --- |
| Kotlin | 2.1.0 | Android 主语言 | Apache-2.0 |
| Compose BOM | 2024.12.01 | UI | Apache-2.0 |
| OkHttp | 4.12.0 | LLM SSE / HTTP | Apache-2.0 |
| kotlinx-serialization | 1.7.3 | JSON | Apache-2.0 |
| kotlinx-coroutines | 1.7.3 | 异步 | Apache-2.0 |
| SnakeYAML | 2.0 | persona.yaml 解析 | Apache-2.0 |
| Robolectric | 4.14.1 | JVM 单测 | MIT/Apache-2.0 |
| Purism Core | vendored `cpp/purism/` | Live2D Core 替代 | MIT |
| CubismNativeFramework | vendored `cpp/framework/` | 官方 C++ 渲染框架 | Live2D 开源许可（需确认） |
| minimp3 | vendored `cpp/minimp3/` | MP3 解码 | CC0 |
| sherpa-onnx | 1.13.4 AAR（可选） | 离线 TTS/ASR | Apache-2.0 |

## PC 端

| 依赖 | 用途 | 许可 |
| --- | --- | --- |
| Python 3.10+ | 后端运行时 | PSF |
| FastAPI / Uvicorn | HTTP/WS 服务 | MIT |
| Edge TTS | 云端 TTS（可选） | MIT/GPL 需确认 |
| CosyVoice Cloud SDK | 阿里云 TTS | 阿里云服务条款 |
| sherpa-onnx | 离线 TTS/ASR | Apache-2.0 |
| Faster-Whisper | ASR（可选） | MIT |
| Silero VAD | VAD | MIT |
| React / Vite / ChakraUI | 前端 | MIT |

## 版本锁定建议

- Android：已通过 Gradle 依赖版本锁定，建议在 `gradle/libs.versions.toml` 集中管理（当前未使用）。
- PC：建议用 `pyproject.toml` / `uv.lock` 锁定（当前 `uv sync` 已可用）。
- Native：`cpp/purism/`、`cpp/framework/` 是 vendored 代码，建议记录上游 commit，方便升级审计。
