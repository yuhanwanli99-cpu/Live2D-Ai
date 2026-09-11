# l2d — Live2D 皮套格式、兼容报告与离屏渲染封装

`l2d` 是 Live2D-Ai Rust 重建（RFC：`docs/plans/RUST-REWRITE-RFC.md`）的皮套层 crate。

## 模块

- `format`：`MocFormat`（`Moc3(MocVersion)` / `Unsupported(版本字节)` / `Unknown`）、
  `MocVersion`（公开版本字节白名单 1–6）、保守 MOC3 头探测（magic + 版本字节 +
  字节序标志，64 字节头；白名单匹配、不推断）。**头版本唯一事实来源是
  `MocFormat::Moc3(v)` 变体**——报告等读取方一律解构格式取版本，不再另存副本。
- `report`：`Analyzer` / `CompatibilityReport`（`#[non_exhaustive]`）+
  类型化诊断 [`CompatibilityIssue`]（`UnknownFormat` /
  `UnrecordedVersionByte(u8)` / `UnanchoredVersion(MocVersion)` /
  `RuntimeVersionMismatch { header, runtime }`）；任何一条 issue 即否决
  `is_supported_v0()`。v0 仅锚定 Bai 实测档位 moc3 raw 字节 4 → Cubism 4.2；
  raw v5/v6 为下一兼容方向，未验证不放行。
- `asset`：model3 皮套包加载（清单/纹理/物理字节 + 兼容报告），面向通用模型路径；
- `model`：moc3 运行时句柄 `ModelHandle`（画布/版本/规模，全部以自有类型暴露；
  版本映射用**穷尽 `match`**，上游新增判别值时编译期即暴露）+ 统一加载路径
  **`LoadedModel::resolve(package)`**：包 → 句柄原子解析 + 兼容报告 + 「头探测 ×
  运行时版本」交叉核对（不一致 → `RuntimeVersionMismatch` 否决 v0 播放）。
  `LoadedModel` 同时持有包与句柄，纹理/物理与 moc3 必然同源——渲染器只接受
  该类型，**model/package 错配在类型层面不可能发生**。
- `pose_stack`：姿态分层栈（纯逻辑、无 GPU 可独立测试）。优先级自低到高：
  | 层 | 写入方 |
  |---|---|
  | base | 模型清单默认值 |
  | idle | 内建待机驱动（呼吸正弦/摆头），每帧整体重算 |
  | input | `set_parameter(id, v)` / `clear_parameter(id)` |
  | physics | 物理引擎输出（消费 idle+input 作为输入信号） |
  | final_override | `override_parameter(id, v)` / `clear_override_parameter(id)` |
  每帧从独立层重新合成最终姿态：idle 不再被历史合并值覆盖，
  `clear_parameter` 的撤销可正确传播。`update(dt)` 拒绝非有限或 <= 0 的 dt
  （`InvalidDt`，拒绝时状态与时钟不变）。
- `renderer`：headless 离屏渲染，三层结构：
  - **`GpuContext`**：共享 GPU 上下文。`GpuContext::headless()` 自建设备
    （vulkan→gl→all 回退），或 `GpuContext::from_parts(device, queue)` 用
    **已有 wgpu 设备**接入；可被多个画布/核心共享。
  - **`ModelRendererCore`**：渲染核心（上游 `ModelRenderer` 封装 + 姿态分层栈），
    与输出目标解耦，`render_to_view(view, format)` 可把一帧渲到任意视图
    （离屏纹理或窗口交换链）。
  - **`OffscreenRenderer`**：便利层（建画布 → 加载 → 固定 dt 更新 → RGBA/PNG）。
    创建/`resize` 均校验尺寸（0 尺寸 → `RenderError::InvalidSize`）；
    非正方形视口自动套**矩形等比变换**（letterbox，不拉伸，见
    `aspect_fit_transform`）。
  - 帧读回为**诚实命名的阻塞语义**：先 `poll(wait)` 等队列排空并触发 GPU 回调，
    再阻塞收取结果——无人为重试循环、无伪造超时预算；回调通道意外关闭时返回
    类型化的 `RenderError::ReadbackClosed`。

## 测试

```text
cargo test -p l2d
```

- 单元测试覆盖格式探测、类型化兼容问题、姿态分层优先级/dt 校验、aspect 变换等；
- `tests/bai_asset.rs` 为 **Bai 真实资产门控集成测试**：加载、清单字段、
  头探测 × 运行时版本一致性、256² 真实出帧（coverage/bbox 容差窗 + 双渲染
  hash 一致性）、resize 与 0 尺寸拒绝。资产缺失或无 headless GPU 时打印
  明确的 SKIP 行后跳过（不静默假装跑过）。

示例：`cargo run -p l2d --example render_model -- [model3.json] [out.png] [size]`
（默认指向仓库内 Bai 皮套，输出 `verification/rust-bakeoff/selected-bai.png`）。

## Runtime/render 底座（第三方来源，已登记）

自 2026-08-26 起（RFC D4 Bakeoff 结论），本 crate 以 **git 依赖固定 rev** 方式引入
[Ayagami](https://github.com/AyagamiDev/ayagami) 的 `ayagami` + `ayagami-render`
两个 crate 作为 runtime/render 底座：

- 固定 rev：`640ae4b10bad8def1adcacdada4f8241b484c169`（禁止浮动分支；升级须重新走验证流程）；
- 上游许可：**MIT OR Apache-2.0**（上游仓库根 `COPYRIGHT` / `LICENSE-MIT` /
  `LICENSE-APACHE`），与本 crate 双许可一致；
- 引入方式为依赖 pin，**未复制/vendored 任何上游源码**；本 crate 公开 API 全部为自有类型，
  第三方类型不越过 crate 边界；
- 选型依据与双报告对照：`docs/verification/rust-bakeoff-decision.md`、根 `SOURCES.md`
  「已引入」一节。

版本表与头布局早期依据纯 Rust runtime [Mocari](https://github.com/Eatgrapes/Mocari)
与 [py-moc3](https://github.com/Ludentes/py-moc3)，并与本仓库 `bai.moc3`
实测头（`4D 4F 43 33 04` → v4.2 小端）交叉验证。Mocari 本身**未被引入**，
仅作渲染行为对照与动画能力参考。

## 许可

本 crate 采用 **双许可：MIT OR Apache-2.0**（SPDX: `MIT OR Apache-2.0`）：

- [LICENSE-MIT](LICENSE-MIT)
- [LICENSE-APACHE](LICENSE-APACHE)

任选其一即可。注意：本 crate 与应用其余部分（AGPL-3.0-only）许可不同，
引用本 crate 时请遵守其 MIT/Apache 双许可条款。
