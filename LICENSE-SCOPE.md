# LICENSE-SCOPE — 许可适用范围说明

> 本文解释 Live2D-Ai 仓库中各部分代码与资源分别适用什么许可证。
> 本仓库自有代码默认按 AGPL-3.0-only 发布（见根 [LICENSE](LICENSE)）；
> 第三方代码与资源保留各自许可。商业使用若遵守 AGPL-3.0 义务即被允许；
> 如需在不履行 AGPL 义务的闭源产品/托管服务中使用本项目自有代码，可申请独立的可选商业许可证（见 [COMMERCIAL-LICENSE.md](COMMERCIAL-LICENSE.md)）。

## 1. 代码部分

| 范围 | 许可证 | 说明 |
|---|---|---|
| Live2D-Ai 自研代码（Android 端 `Live2D-Ai-Android/` 中本项目新增/修改部分、PC 端新增/修改部分、`shared/`、根目录工具脚本） | **AGPL-3.0-only**（`LICENSE`） | 本项目的自有代码 |
| 上游 Open-LLM-VTuber 原始代码（`Live2D-Ai-pc/open-llm-vtuber/` 中未修改或仅搬移的上游代码） | **MIT**（`Live2D-Ai-pc/open-llm-vtuber/LICENSE`） | 保留上游 Copyright (c) Yi-Ting Chiu / Open-LLM-VTuber contributors |
| N.E.K.O. | Apache-2.0（design reference only，未抄代码） | 见 `NOTICE`、`CREDITS.md` |
| pymouth / kugutu / Purism Core | MIT | 见 `CREDITS.md` |
| PixiJS / pixi-live2d-display-lipsyncpatch / @soullink-emotion/* | MIT | PC renderer 依赖，见 `CREDITS.md` |
| Android 生态依赖（Jetpack Compose、AndroidX、OkHttp、SnakeYAML 等） | Apache-2.0 / MIT / EPL-2.0 | 二进制依赖，见 `CREDITS.md` |

> **重要**：PC 端 fork 目录同时包含上游 MIT 代码和本项目 AGPL 修改代码。以文件为单位判断：上游文件保留其 MIT 头；本项目新增/修改的文件按 AGPL-3.0-only。提交贡献时默认按 AGPL-3.0-only 授权本项目自有代码。

## 2. Live2D 渲染相关

| 组件 | 许可证 | Preview 是否分发 |
|---|---|---|
| Purism Core（Android 端原生渲染） | MIT（自研干净实现） | — |
| Cubism Java Framework 着色器（Android 端 `assets/shaders/`） | **Live2D Open Software License**（不可改授为 AGPL/MIT） | — |
| **Live2D Cubism Web Core**（PC 端 `live2dcubismcore.min.js`） | **Live2D Proprietary Software License** | ❌ **不分发**；用户自行从 https://www.live2d.com/ 接受许可后下载 |

## 3. 模型与素材

- `bai`（白-免费版）：模型作者自定义条约，来源视频 https://www.bilibili.com/video/BV1NqNFejEeE/。
  作者允许盈利直播、二创、少量舰长礼、改色贴图；禁止公开售卖周边；可对恶意使用收回授权。
  **PC-only Preview 不分发 `.moc3` 与纹理等原文件**，用户需自作者渠道合法取得并导入。
- Niziiro Mao：Live2D Free Material License（Live2D Inc.）。
- 本项目不适用这些模型作者的许可；任何模型都不被 AGPL 覆盖。

## 4. 密钥 / 配置

- 本项目不内置、不分发任何 API Key、共享 Key 或免费兜底 Provider。
- 用户的 `.env`、`conf.yaml`、本地模型、上传背景、聊天历史等均属本机内容，不入库、不进发布树。

## 5. 发布树

- PC-only Preview 公共发布树只包含本项目自有代码、必要的上游 MIT 代码与文档；
  不含 Android 工程、不含专有 Cubism Web Core、不含模型原文件、不含密钥与本机配置。