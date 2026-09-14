# 壁纸 Mod 策略 v0（`live2d-ai-mod-wallpaper`）

> **状态**：2026-09-14 Wave 1（分支 `mod/wallpaper` @ `2d492447`）起草。
> **未注册**进 `AVAILABLE_MOD_FACTORIES`——注册与数量断言由集成 PR 按
> [`../plans/parallel-mods/REGISTER-wallpaper.md`](../plans/parallel-mods/REGISTER-wallpaper.md) 统一做。
> 上层协议：[`../plans/parallel-mods/PARALLEL-PROTOCOL-2026-09-14.md`](../plans/parallel-mods/PARALLEL-PROTOCOL-2026-09-14.md)；
> Mod 通用契约：[`mod-product-chain.md`](mod-product-chain.md)。

## 1. 定位

本 Mod 只回答一个问题：**何时换壁纸 / 要不要和舞台同步**。

它**不画图**。壁纸的像素路径在 `0.1.0-rc.5` 已经定案（背景预通道 +
`LoadOp::Load` 叠模型，见 [`../releases/v0.1.0-rc.5.md`](../releases/v0.1.0-rc.5.md) §⑦），
本 crate **一行不碰**。因此本版交付的是**可脱机单测的策略状态机** +
**静态配置 schema**，加上一个**明文标注的接线占位**（§5）。

范围真源：`docs/plans/parallel-mods/PLAN-wallpaper.md`（Must / Forbidden / Gate）。

## 2. 不碰什么（红线）

| 对象 | 处置 |
| --- | --- |
| `crates/l2d-wasm-demo/`（含 `stage_bg.rs`） | **不动**。舞台背景写进 framebuffer 是 rc.5 的定案，重做会推翻已验证的合成语义 |
| framebuffer / 预通道 / `LoadOp` | **不动**（「do NOT redo framebuffer」是 PLAN 原文） |
| `AVAILABLE_MOD_FACTORIES` / `mod_count_*` / 缺省 manifest | **不动**（PARALLEL-PROTOCOL §3）；只加 `members` 一行 + desktop path 依赖 |
| 大轮播 / 图库 / 在线拉图 / 轮播 UI | **不做**。`interval` 只按**调用方给的列表长度**推进一个游标 |

## 3. 配置契约（`settings_spec` v1）

`WallpaperFactory::settings_spec()` 是**静态**的（未启用也拿得到，rc.4 M2 语义），
`start` 注册同一份（单测钉住「两处不分叉」）。

| key | kind | 语义 | 缺省 |
| --- | --- | --- | --- |
| `mode` | select | `off` / `follow_stage` / `interval` | `off` |
| `interval_secs` | number | `interval` 模式的切换间隔（秒），钳在 `5..=86400` | `300` |

**宽容口径**（配置写错不打死行为）：

- `mode` 未知值 / 非字符串 / 缺失 → `off`（fail-safe 到「不接管」）；
- `interval_secs` 接受整数与整数形态浮点；负数 / `NaN` / 无穷 / 非数字 / 缺失 → `300`；
- 最终值**钳位**，**永不返回 0**——于是 `interval_ms()` 恒非 0，策略里没有除零路径。

schema 里**没有**第二个 `enabled`：启停唯一真源是 Mod manifest 的 `enabled`
（与 `external-input` / `voice-input` 同口径，见 [`mod-product-chain.md`](mod-product-chain.md) §4）。

`mode = off`（缺省）时本 Mod **完全不接管**——产品既有的
`DisplayPrefs.syncShellStageBg = true`（rc.5 缺省）保持不变，本 Mod 不覆盖它。

## 4. 纯策略状态机

实现：`crates/live2d-ai-mod-wallpaper/src/strategy.rs`（**不依赖 host、不读时钟、不碰 IO**）。
时间由调用方以**增量毫秒**喂入 `WallpaperStrategy::tick(delta_ms)`，因此单测不需要 sleep，
也不需要假时钟。

```text
tick(delta_ms) -> WallpaperDecision

off           -> None（恒不动作）
follow_stage  -> 首次 SyncStage，之后恒 None（同步一次即可）
interval      -> 首帧  Advance { index: 0 }
                 之后每满 interval_ms 前进一格并取模
                 列表为空 -> None（保持不动）
```

三条刻意设计：

1. **不追帧**：一次 `tick` 至多换一张，换后计时归零；进程长挂起后不会「补播」一大串。
   这与「不做大轮播」是同一条纪律。
2. **不持有图片**：只持有列表**长度**与游标；图从哪来由集成方（现有 `DisplayPrefs` /
   stage-bg 通道）决定。本 crate 不读文件、不存 dataURL。
3. **重配置即复位**：`reconfigure` 把计时 / 同步标志 / 游标全部复位——`interval` 换间隔后
   不沿用旧累计，`follow_stage` 换进来后**重新发一次** `SyncStage`（配置改了就重新同步）。

决策类型：

```rust
pub enum WallpaperDecision {
    None,                       // 不动作
    SyncStage,                  // 壳跟随舞台（syncShellStageBg = true）
    Advance { index: usize },   // 切到播放列表第 index 张
}
```

## 5. 决策 → 显示层落点（**v0 占位，明文**）

`DisplayPrefs`（`shell/flutter/lib/settings/display_prefs.dart`）与 stage-bg 通道
（`Live2DStage.sendStageBg` → wasm `stage-bg`）**都在 Flutter / wasm 侧**，
Mod API 目前**没有壁纸写入口**。所以本 crate 产出决策后：

- `WallpaperRuntime::tick` 记一条 info 日志并把决策**返回给集成方**；
- `WallpaperRuntime::apply_decision` 对 `None` 返回 `true`（幂等），
  对其余决策**记一条 warn 并返回 `false`**——这是**刻意的占位，不是遗忘**。

集成方要接的线（**复用既有通道，不得新增 wasm / framebuffer 路径**）：

| 决策 | 落点 |
| --- | --- |
| `SyncStage` | `DisplayPrefs.copyWith(syncShellStageBg: true)`（`effectiveShellImage` 随即等于 `stageImage`） |
| `Advance { index }` | 取播放列表第 `index` 张 → `prefs.copyWith(stageImage: dataUrl)` → 经既有 `sendStageBg` 下发 |

节拍（谁来驱动 `tick`）同样由集成方决定：Mod 事件主题里**没有时钟**
（`ModEventTopic` 只有 turn / text / voice / model 六项），v0 不在 Mod 内起线程。
可选做法：主循环 timer、或在 `VoiceEnded` / `TurnStarted` 事件上做粗粒度节拍。

## 6. 门禁与测试

- `cargo test -p live2d-ai-mod-wallpaper` → **22 passed / 0 failed**（0 doc-test，纯逻辑无需示例）；
- `cargo fmt -p live2d-ai-mod-wallpaper -- --check` clean；
- `cargo clippy -p live2d-ai-mod-wallpaper --all-targets -- -D warnings` → 0 warning。

覆盖：模式映射与缺省、间隔缺省/钳位/非数字、`off` 恒不动作、`follow_stage` 只同步一次且
重配置后重新同步、`interval` 首帧/到点才换/取模回绕/空列表保持/长挂起不追帧/`u64::MAX` 饱和、
`set_playlist_len` 钳位与清空、`reconfigure` 复位、schema 字段与选项顺序且无 `enabled`、
`start` 只注册 settings 不订阅事件、runtime 日志与占位 `apply_decision`。

涉及文件：

- `crates/live2d-ai-mod-wallpaper/src/lib.rs`（Mod 集成：descriptor / schema / runtime / factory）
- `crates/live2d-ai-mod-wallpaper/src/strategy.rs`（纯策略）
- 根 `Cargo.toml`（members 一行）、`crates/live2d-ai-desktop/Cargo.toml`（path 依赖一行）

## 7. 已知缺口 / 下一步

1. **注册未做**（本分支刻意不做）：`FACTORIES` / `mod_count_*` / id 断言 / 缺省 manifest /
   本文档与 AGENTS 的 Mod 一行——步骤见 [`REGISTER-wallpaper.md`](../plans/parallel-mods/REGISTER-wallpaper.md)。
2. **落点未接线**（§5）：需要一条 Mod → 显示层的通道；在不新增 wasm 路径的前提下，
   建议由 web_api 把决策投影成一条 WS 帧 / 前端设置字段，由既有 `DisplayPrefs` 消费。
3. **播放列表来源未定**：v0 只接受长度；真正多图管理（图库、导入、删除）属后续 rc。

## 8. 非目标

- 动态壁纸（视频 / shader / 桌面级壁纸）——本 Mod 只管**背景图**；
- 与舞台**各自独立**的壁纸轮播（rc.5 已把壳与舞台收敛为「一份真相」）；
- 在线图源 / 拉图 / 缓存；
- 把壁纸 Mod 变成动作 / 编舞的驱动方（动作在产品路径上不存在，见
  [`core-chain-baseline.md`](core-chain-baseline.md) §3.1）。
