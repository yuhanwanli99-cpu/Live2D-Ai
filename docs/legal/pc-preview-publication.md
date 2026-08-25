# PC-only Preview publication policy

Release target: `v0.2.0-preview.1`. The supported public scope is `Live2D-Ai-pc/open-llm-vtuber` plus the minimum shared documentation/configuration required by the PC client. Android is not a supported artifact in this Preview.

## Excluded content

The public release tree and release archives must exclude:

- Bai/白 model `.moc3`, texture, and other raw model assets. The source video is https://www.bilibili.com/video/BV1NqNFejEeE/; its current custom terms do not explicitly grant public Git redistribution of raw files.
- `.env`, provider keys, external-input tokens, chat history, cache/audio files, uploaded backgrounds, crash dumps, local absolute-path configuration, virtual environments, and `node_modules`.
- Android application sources and artifacts from the PC-only release archive.

The generic model loader, model adapters, parameter tests, and attribution may remain. Users must obtain models lawfully and import them locally.

## Required gates

1. `uv sync --locked` and the Python test suite pass in CI on Python 3.12.
2. Renderer `npm ci`, tests, type-check, and production build pass.
3. Secret scan passes on the Git history used for publication.
4. The generated archive contains none of the excluded paths or model binaries.
5. README and release notes identify the build as PC-only Preview and list unsupported platforms.
