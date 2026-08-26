# SOURCES.md — 来源纪律与许可边界

> 用途：任何第三方代码进入本仓库源码树**之前**，必须先在本文件登记
> （来源、许可证、版本/commit、目的），经确认后再引入（RFC `docs/plans/RUST-REWRITE-RFC.md` §5 / D4）。
> 引入动作发生时，在「已引入」一节补记 fork/pin 的固定 commit 与引入位置；
> 未登记即引入视为违规。

---

## 当前状态：Ayagami 已 pin 引入；Mocari 仅登记未引入

**2026-08-26 更新**：Bakeoff 双报告（`docs/verification/rust-bakeoff-ayagami.md` /
`docs/verification/rust-bakeoff-mocari.md`）已完成，选型决策见
`docs/verification/rust-bakeoff-decision.md`：**Ayagami 胜出，作为 runtime/render 底座
以固定 rev 引入 `crates/l2d`**（git 依赖 pin，未复制/vendored 源码）；
Mocari 保持「仅登记、未引入」，作为未引入参考与动画补充候选。

### Runtime 候选（Bakeoff 结论已出，RFC D4）

| # | 名称 | 上游 | 许可证 | 语言 | 状态 | 登记目的 |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | **Mocari** | <https://github.com/Eatgrapes/Mocari> | MIT | Rust + WGSL（纯 Rust Cubism/Live2D runtime 实验） | **未引入**（参考/动画补充候选）；观察快照 `13c7ed3` @ main（2026-08-26 查询） | Bakeoff 落选方：发布过 crates.io 版本、无 unsafe、内建 motion/expression/pose；保留作渲染行为对照与未来动画能力参考 |
| 2 | **Ayagami** | <https://github.com/AyagamiDev/ayagami> | **MIT OR Apache-2.0**（双许可） | Rust + WGSL（兼容 Live2D 的 2D 皮套渲染器） | **已引入**（见下节「已引入」）；观察快照 `640ae4b` @ 默认分支（2026-08-26 查询） | Live2D runtime/render 底座：Bakeoff 胜者 |

- **Ayagami 许可证更正**：早前误记为 Apache-2.0 单许可。以上游仓库根实际文件为准：
  `COPYRIGHT`（自述 "dual-licensed under Apache 2.0 and MIT terms"）、`LICENSE-MIT`、
  `LICENSE-APACHE` 三份文件并存 → SPDX 表达为 **MIT OR Apache-2.0**，与本 crate
  （`crates/l2d`）的双许可一致，无边界冲突。
- 两候选许可证均与本项目 AGPL-3.0-only 应用侧兼容。
- Bakeoff 基准皮套为 **Bai（白）**（v0 只锚定 Bai 实测档位 moc3 raw 字节 4 → Cubism 4.2；
  注意 RFC D5 已于 2026-08-26 修订格式优先级，见该文档修订记录）。

### 渲染行为 Oracle（对照基准，不引入）

| 名称 | 来源 | 许可证 | 说明 |
| --- | --- | --- | --- |
| **PurismCore** | 本仓库既有资产：`Live2D-Ai-Android` 内 vendored 的 C99 原生层（`cpp/purism/`，25 个 `.c/.h`，全部带 `SPDX-License-Identifier: MIT`，见 `docs/research/license-report.md` §1） | MIT | 作为 Rust 渲染实现的**行为 Oracle 对照**：同一输入下核对顶点/裁剪/动画输出是否一致 |

- PurismCore 属于旧 Android 项目保留资产（RFC 范围外，不动）；Rust workspace
  （`crates/l2d` 等）**未复制、未编译链接其任何代码**，仅以其运行行为为正确性参照。
- 对照结论（后续批次）记录于此处及 `docs/verification/`。

---

## 已引入

| 名称 | 引入方式与位置 | 固定 rev | 许可证 | 引入日期 | 决策依据 |
| --- | --- | --- | --- | --- | --- |
| **Ayagami**（`ayagami` + `ayagami-render`） | `crates/l2d/Cargo.toml` git 依赖（未复制/vendored 源码；crate 对外仅暴露自有封装 API，第三方类型不越过 crate 边界） | [`640ae4b10bad8def1adcacdada4f8241b484c169`](https://github.com/AyagamiDev/ayagami)（**禁止浮动分支**；升级 rev 须重新验证并同步本文件） | MIT OR Apache-2.0（上游根 `COPYRIGHT`/`LICENSE-MIT`/`LICENSE-APACHE`） | 2026-08-26 | RFC D4 Bakeoff：`docs/verification/rust-bakeoff-decision.md`；验证报告 `docs/verification/rust-bakeoff-ayagami.md` |

- Mocari 未引入（无 crates/l2d 依赖、无源码复制），仅作参考。

---

## 纪律要点

1. **先登记、后引入**：来源、许可证、版本/commit、目的一并写明；
2. 引入一律 **fork/pin 固定 commit**，禁止跟踪浮动分支；
3. 引入位置只允许出现在对应 crate 目录内并在 crate README/LICENSE 注明出处；
4. 与 AGPL-3.0-only（应用侧）/ MIT OR Apache-2.0（`crates/l2d`，RFC D11）许可边界冲突的
   来源不得引入；
5. 每次引入或 Bakeoff 决策后更新本文件，并在 `CHANGELOG.md` 记一笔。
