# Live2D 模型资产目录

> **出厂与条约**：渲染白（Bai）模型**必需**的文件已随仓库分发——`MODEL_LICENSE.md`、
> `runtime/bai.model3.json`、`runtime/bai.moc3`、`runtime/bai.cdi3.json`、
> `runtime/bai.physics3.json`、`runtime/bai.16384/texture_00_4096.png`。
> 条约是作者的使用条约（**不是**作者授权再分发），全文见 `bai/MODEL_LICENSE.md`。
> 未被 `model3` 引用的 `runtime/bai.16384/texture_00.png` 与 Cubism/VTube 工程残留
> （`bai.vtube.json`、`items_pinned_to_model.json`）**仍不入库**。

## 当前使用

Rust 主线默认采用 **白 (Bai)** 模型作为出厂模型（参见 `crates/l2d/tests/bai_asset.rs` 与
`crates/live2d-ai-desktop/src/cli.rs`）。**0.3.5 起可在「设置 → 模型库」切换本机其它
`model3`，白模仍是出厂与回退模型**；registry（`model_registry.json`）是运行时数据，
不进仓库。
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

## 多模型（0.3.5）与授权

- **用户自持授权**：本仓库不附带、也不再分发任何非 Bai 模型。用户导入的模型必须
  由用户自己取得授权/遵守作者使用条约；仓库不做运行期 license gate，也**不做假
  license 检查**。
- **未授权模型不提交仓库**：`assets/models/*` 的 git 白名单只有 `README.md`、
  `.gitkeep` 与 `bai`（见根 `.gitignore`）。**不要**扩大白名单，也不要提交用户导入的
  模型目录或 `model_registry.json`。
- 导入只在**本机**生效：把模型目录放到 `assets/models/<id>/`，在「设置 → 模型库」
  填目录名导入，再点「激活」；真正换皮由舞台 iframe 热换并等渲染面 `loaded`。
- **参数名不匹配的动作继续丢弃**，并沿用既有 ack 原因（缺参名 /
  `expression_unknown_id`）；0.3.5 **不新增 remap 表**，也不给单个模型硬编码
  `field_map` 段。
- **未验**：第二张合法 `model3`、浏览器肉眼、真实服务 HTTP 换模、公开远端 / CI；
  换模 / 回滚 / 字段映射仅 fixture 单测。不以「已真机 / 已浏览器」表述。

## 历史来源

- 在 `Live2D-Ai-pc/`（Py 版）下，原路径为 `Live2D-Ai-pc/open-llm-vtuber/live2d-models/bai/`
- 2026-08-28 Py 版退役时，Py 端的 `.gitignore` 早已将 `/live2d-models/*` 排除（仅 `mao_pro` / `shizuku` 默认白名单可入库），
  因此本目录从未跟踪过模型二进制，迁移时使用普通 `mv` 而非 `git mv`。

## 门控

- `crates/l2d/tests/bai_asset.rs` 采用「资产缺失则明确 skip」的纪律——本地未放置 Bai 模型时，
  该集成测试打印 `SKIP:` 后直接返回，**不会**因找不到文件而失败。
- 同进程双渲染 hash 比对与 coverage/bbox 容差窗仅在资产存在时执行。
