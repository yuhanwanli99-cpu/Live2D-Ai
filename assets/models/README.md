# Live2D 模型资产目录

> **不入库声明**：本目录下的 Live2D 模型二进制资源（`.moc3`、`.cdi3.json`、`.model3.json`、纹理 `.png` 等）
> **未获原作者明确授予公开 Git 仓库再分发的权利**，因此本仓库的 `.gitignore` 已将 `assets/models/*` 排除在版本控制之外。
> 本目录仅作为本地运行的占位说明，请按需自行获取并放置模型文件。

## 当前使用

Rust 主线默认采用 **白 (Bai)** 模型作为 v0 唯一锚定档位（参见 `crates/l2d/tests/bai_asset.rs` 与 `crates/live2d-ai-desktop/src/cli.rs`）。
请将 Bai 模型按下列结构放置到本目录：

```
assets/models/bai/
├── MODEL_LICENSE.md
└── runtime/
    ├── bai.moc3
    ├── bai.model3.json
    ├── bai.cdi3.json
    ├── bai.physics3.json
    ├── bai.vtube.json
    ├── items_pinned_to_model.json
    └── bai.16384/
        ├── texture_00.png
        └── texture_00_4096.png
```

## 获取方式

- 原视频与作者使用条约：<https://www.bilibili.com/video/BV1NqNFejEeE/>
- 使用条约全文见 `bai/MODEL_LICENSE.md`（用户导入时随模型分发）。
- **仓库内不捆绑模型文件**，请从作者渠道合法取得并自行导入。

## 历史来源

- 在 `Live2D-Ai-pc/`（Py 版）下，原路径为 `Live2D-Ai-pc/open-llm-vtuber/live2d-models/bai/`
- 2026-08-28 Py 版退役时，Py 端的 `.gitignore` 早已将 `/live2d-models/*` 排除（仅 `mao_pro` / `shizuku` 默认白名单可入库），
  因此本目录从未跟踪过模型二进制，迁移时使用普通 `mv` 而非 `git mv`。

## 门控

- `crates/l2d/tests/bai_asset.rs` 采用「资产缺失则明确 skip」的纪律——本地未放置 Bai 模型时，
  该集成测试打印 `SKIP:` 后直接返回，**不会**因找不到文件而失败。
- 同进程双渲染 hash 比对与 coverage/bbox 容差窗仅在资产存在时执行。
