# Live2D 模型资产目录

> **出厂与条约**：渲染白（Bai）模型**必需**的文件已随仓库分发——`MODEL_LICENSE.md`、
> `runtime/bai.model3.json`、`runtime/bai.moc3`、`runtime/bai.cdi3.json`、
> `runtime/bai.physics3.json`、`runtime/bai.16384/texture_00_4096.png`。
> 条约是作者的使用条约（**不是**作者授权再分发），全文见 `bai/MODEL_LICENSE.md`。
> 未被 `model3` 引用的 `runtime/bai.16384/texture_00.png` 与 Cubism/VTube 工程残留
> （`bai.vtube.json`、`items_pinned_to_model.json`）**仍不入库**。

## 当前使用

Rust 主线默认采用 **白 (Bai)** 模型作为 v0 唯一锚定档位（参见 `crates/l2d/tests/bai_asset.rs` 与 `crates/live2d-ai-desktop/src/cli.rs`）。
出厂即带下列结构（\* = 仍不入库，需自行放置）：

```
assets/models/bai/
├── MODEL_LICENSE.md
└── runtime/
    ├── bai.moc3
    ├── bai.model3.json
    ├── bai.cdi3.json
    ├── bai.physics3.json
    ├── bai.vtube.json              *
    ├── items_pinned_to_model.json  *
    └── bai.16384/
        ├── texture_00.png          *
        └── texture_00_4096.png
```

## 获取方式

- 原视频与作者使用条约：<https://www.bilibili.com/video/BV1NqNFejEeE/>
- 使用条约全文见 `bai/MODEL_LICENSE.md`（用户导入时随模型分发）。
- 渲染必需文件已随仓库分发（见上）；`texture_00.png`、`bai.vtube.json`、`items_pinned_to_model.json` 仍不入库，请从作者渠道合法取得后自行放置。

## 历史来源

- 在 `Live2D-Ai-pc/`（Py 版）下，原路径为 `Live2D-Ai-pc/open-llm-vtuber/live2d-models/bai/`
- 2026-08-28 Py 版退役时，Py 端的 `.gitignore` 早已将 `/live2d-models/*` 排除（仅 `mao_pro` / `shizuku` 默认白名单可入库），
  因此本目录从未跟踪过模型二进制，迁移时使用普通 `mv` 而非 `git mv`。

## 门控

- `crates/l2d/tests/bai_asset.rs` 采用「资产缺失则明确 skip」的纪律——本地未放置 Bai 模型时，
  该集成测试打印 `SKIP:` 后直接返回，**不会**因找不到文件而失败。
- 同进程双渲染 hash 比对与 coverage/bbox 容差窗仅在资产存在时执行。
