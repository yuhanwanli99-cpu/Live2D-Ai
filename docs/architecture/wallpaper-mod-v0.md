# 壁纸 Mod 策略 v0（`live2d-ai-mod-wallpaper`）

> **状态**：2026-09-14 Wave 1（分支 `mod/wallpaper` @ `2d492447`）起草；
> `0.2.0-rc.2` 集成时按
> [`../plans/parallel-mods/REGISTER-wallpaper.md`](../plans/parallel-mods/REGISTER-wallpaper.md)
> **已注册**进 `AVAILABLE_MOD_FACTORIES`（`mod_count_is_five`，**缺省停用**）。
> **2026-09-14 Wave 2（分支 `mod/wallpaper-wire` @ 基座 `429609f2`）§5 落点已接线**：
> Mod 决策 → `state_json` 的 `prefs_patch` → Flutter 纯函数 `applyWallpaperPatch`
> → 既有 `DisplayPrefs` / `Live2DStage.sendStageBg`；播放列表来源 = Flutter 自己的偏好
> `DisplayPrefs.stagePlaylist`。接线清单见
> [`../plans/parallel-mods/REGISTER-wallpaper-wire.md`](../plans/parallel-mods/REGISTER-wallpaper-wire.md)。
> 上层协议：[`../plans/parallel-mods/PARALLEL-PROTOCOL-2026-09-14.md`](../plans/parallel-mods/PARALLEL-PROTOCOL-2026-09-14.md)、
> [`../plans/parallel-mods/PARALLEL-WAVE2-2026-09-14.md`](../plans/parallel-mods/PARALLEL-WAVE2-2026-09-14.md) §3B；
> Mod 通用契约：[`mod-product-chain.md`](mod-product-chain.md)。

## 1. 定位

本 Mod 只回答一个问题：**何时换壁纸 / 要不要和舞台同步**。

它**不画图**。壁纸的像素路径在 `0.1.0-rc.5` 已经定案（背景预通道 +
`LoadOp::Load` 叠模型，见 [`../releases/v0.1.0-rc.5.md`](../releases/v0.1.0-rc.5.md) §⑦），
本 crate **一行不碰**。因此本版交付的是**可脱机单测的策略状态机** +
**静态配置 schema** + **决策 → `DisplayPrefs` 的真投影**（§5 已接线）。

范围真源：`docs/plans/parallel-mods/PLAN-wallpaper.md`（Must / Forbidden / Gate）。

## 2. 不碰什么（红线）

| 对象 | 处置 |
| --- | --- |
| `crates/l2d-wasm-demo/`（含 `stage_bg.rs`） | **不动**。舞台背景写进 framebuffer 是 rc.5 的定案，重做会推翻已验证的合成语义 |
| framebuffer / 预通道 / `LoadOp` | **不动**（「do NOT redo framebuffer」是 PLAN 原文） |
| `AVAILABLE_MOD_FACTORIES` / `mod_count_*` / 缺省 manifest | **不动**（PARALLEL-PROTOCOL §3）；只加 `members` 一行 + desktop path 依赖 |
| 大轮播 / 图库 / 在线拉图 / 轮播 UI | **不做**。`interval` 只按**调用方给的列表长度**推进一个游标 |

## 3. 配置契约（`settings_spec` v1 + Wave 2 的 `playlist_len`）

`WallpaperFactory::settings_spec()` 是**静态**的（未启用也拿得到，rc.4 M2 语义），
`start` 注册同一份（单测钉住「两处不分叉」）。

| key | kind | 语义 | 缺省 |
| --- | --- | --- | --- |
| `mode` | select | `off` / `follow_stage` / `interval` | `off` |
| `interval_secs` | number | `interval` 模式的切换间隔（秒），钳在 `5..=86400` | `300` |
| `playlist_len` | number | **播放列表长度**（有几张图可切），钳在 `0..=MAX_PLAYLIST_LEN`（16） | `0` |

**宽容口径**（配置写错不打死行为）：

- `mode` 未知值 / 非字符串 / 缺失 → `off`（fail-safe 到「不接管」）；
- `interval_secs` 接受整数与整数形态浮点；负数 / `NaN` / 无穷 / 非数字 / 缺失 → `300`；
- 最终值**钳位**，**永不返回 0**——于是 `interval_ms()` 恒非 0，策略里没有除零路径；
- `playlist_len` 负数 / 非数字 / 缺失 → `0`（没有可切的图）；超上限 → 钳到 16。

`playlist_len` 是 **Wave 2 新增的「前端写回位」**：图住 Flutter 的
`DisplayPrefs.stagePlaylist`（§5），只有它知道有几张；Flutter 在列表长度变化时用
既有 `POST /api/v1/mods/wallpaper/config` 写回（**整份替换**，所以是从服务端现有
config 出发只覆盖这一个键）。它也能在 Mod 面板里手填（schema 里就是普通 number），
但正常路径由 Flutter 维护。

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

## 5. 决策 → 显示层落点（**Wave 2：已接线**）

### 5.1 通道：Mod 不写盘，只投影数据

`DisplayPrefs`（`shell/flutter/lib/settings/display_prefs.dart`）与 stage-bg 通道
（`Live2DStage.sendStageBg` → wasm `stage-bg`）都在 Flutter / wasm 侧，Mod API
**没有**壁纸写入口——这一点没变，变的是：**不再有占位**。

```
WallpaperDecision::to_prefs_patch()            （Rust 纯函数，唯一真投影）
      │  None / {"sync_shell_stage_bg":true} / {"stage_index":N}
      ▼
WallpaperRuntime::state_json()                 （基座 Wave 2 API，只读快照）
      ▼
GET /api/v1/mods/wallpaper/state               （基座 Wave 2 端点；503 = 暂时读不到）
      ▼
applyWallpaperPatch(prefs, patch)              （Flutter 纯函数，VM 可测）
      ▼
DisplayPrefs.copyWith(…) → Live2DStage.sendStageBg（既有通道，不碰 stage_bg.rs / framebuffer）
```

| 决策 | `prefs_patch` | Flutter 落点 |
| --- | --- | --- |
| `WallpaperDecision::None` | `null` | **什么都不做**（`reason` 说明为什么，不是「假装成功」） |
| `WallpaperDecision::SyncStage` | `{"sync_shell_stage_bg":true}` | `copyWith(syncShellStageBg: true)`（已是 true → 跳过，`already_synced`） |
| `WallpaperDecision::Advance { index }` | `{"stage_index":N}` | `stageImage = stagePlaylist[N % len]` → 既有 `sendStageBg` |

早期版本的 `apply_decision`（对 `SyncStage` / `Advance` 记一条 warn 并返回 `false`）
**已删除**：它是「落点未接线」的假话，留着会让人以为决策没人消费。
现在决策的消费方就是 §5.2 的前端闭环。

### 5.2 `state_json` 形状（§3B 契约）

```json
{"mode":"interval","active":true,"playlist_len":3,
 "decision":{"kind":"advance","index":1},
 "prefs_patch":{"stage_index":1},
 "reason":"advance"}
```

- `kind` ∈ `none` / `sync_stage` / `advance`；`decision.index` 只在 `advance` 出现；
- `prefs_patch` 与 `decision` **同源**（同一个纯函数产出），前端不必自己推导；
- `reason` ∈ `mode_off` / `already_synced` / `playlist_empty` / `interval_not_due` /
  `sync_stage` / `advance`——「壁纸怎么不动」的答案，故意把
  `mode_off` 与 `interval_not_due`、`playlist_empty` 分开（处置完全不同）。

**节拍**：策略时钟**只**在 `state_json` 里推进——用内部 `Instant` 算距上次调用的
增量毫秒喂既有纯策略 `tick(delta_ms)`。不在 Mod 里起线程、不订阅事件
（`ModEventTopic` 里没有时钟）、不在 `state_json` 里写盘（契约：
`ModRuntime::state_json` 头注）。首帧增量为 0，于是 `interval` 模式第一次快照
就把第 0 张推上去。

### 5.3 播放列表来源：Flutter 自己的偏好（不新增图库 / 文件服务 / 在线拉图）

列表住 `DisplayPrefs.stagePlaylist`（`List<String>` dataURL），与 `stageImage`
**同一条 localStorage 记录**、同一份长度预算。UI 在既有「外观与互动 → 舞台背景图」
区里加两个最小操作：**「加入轮播」**（把当前 `stageImage` append 进列表）与
**「清空轮播」**，并显示当前张数。不做删除单张 / 拖排序 / 大图库。

三条预算（**常量即契约**，Flutter 与 Rust 各自钳一次）：

| 预算 | 常量（Flutter / Rust） | 行为 |
| --- | --- | --- |
| 单项长度 | `kStageImageMaxChars` / 同（按 `interval_secs` 同款宽容口径） | 超限的项**在读取时丢弃**；append 报 `item_too_large` |
| 列表总长 | `kStagePlaylistMaxChars` / — | 读取时保留前面的、丢掉放不下的；append 报 `budget_exceeded` |
| 项数 | `kStagePlaylistMaxItems` = 16 / `MAX_PLAYLIST_LEN` = 16 | 读取时截断；append 报 `limit_reached`（**列表原样不动**） |

为什么要总长上限：localStorage 是**一条记录**存整份 `DisplayPrefs`，
**超配额会让整份偏好都写不进去**（连主题 / 音量一起丢，rc.5 的教训）。

### 5.4 闭环节拍（前端）

1. wallpaper **已启用**且 `mode != off` → 前端每 **5 s**（`kWallpaperPollInterval`）
   轮询一次 `GET /api/v1/mods/wallpaper/state`；停用 / 关模式 → `Timer` **立即取消**
   （判据 `shouldPollWallpaperState`，纯函数，可单测）。
   轮询本身**就是** Mod 的时钟，所以间隔不能太小（≥5 s 是契约下限）。
2. 拿到 `prefs_patch` → `applyWallpaperPatch` → `DisplayPrefs` → 既有 `sendStageBg`。
3. 列表长度与服务端 `playlist_len` 不一致（含启动引导那一次）→ 用既有
   `POST /api/v1/mods/wallpaper/config` 写回 `{"config":{…,"playlist_len":N}}`
   （**整份替换**语义，所以先从服务端 config 出发只覆盖这一个键，别抹掉
   `mode` / `interval_secs`）；长度没变**不写**（每次写回都会重启 Mod、冲掉游标）。
4. `503 state_unavailable`（刚停用 / worker 正持锁）是**正常**：跳到下一次轮询，
   不弹错误。

### 5.5 列表为空的情形（**明确跳过**）

`interval` 模式下列表为空时，策略层直接返回 `None`（`reason = playlist_empty`），
`prefs_patch` 为 `null`；前端 `applyWallpaperPatch` 返回
`applied=false, reason='playlist_empty'`。**这不是失败，也不是「假装成功」**：
它以 `reason` 如实报告，`stageImage` 保持不动（用户当前那张仍然在舞台上）。
「加入轮播」在无当前图 / 超预算 / 到上限时同样给一句可读文案，列表不动。

## 6. 门禁与测试

- `cargo test -p live2d-ai-mod-wallpaper` → **34 passed / 0 failed**（0 doc-test，纯逻辑无需示例）；
- `cargo fmt -p live2d-ai-mod-wallpaper -- --check` clean；
- `cargo clippy -p live2d-ai-mod-wallpaper --all-targets -- -D warnings` → 0 warning；
- Flutter 侧新增 `test/wallpaper_wiring_test.dart` **24 条**（本轮全量 `flutter test` **857 passed**）。

覆盖（Rust）：模式映射与缺省、间隔缺省/钳位/非数字、`playlist_len` 缺省/钳位/超上限、
`off` 恒不动作、`follow_stage` 只同步一次且重配置后重新同步、`interval` 首帧/到点才换/
取模回绕/空列表保持/长挂起不追帧/`u64::MAX` 饱和、`set_playlist_len` 钳位与清空、
`reconfigure` 复位并重读 `playlist_len`、schema 三字段与选项顺序且无 `enabled`、
`start` 只注册 settings 不订阅事件、`to_prefs_patch` 三态、`to_json` 的 `kind`、
`reason` 六种取值、**`state_json` 形状/时钟推进/off 不动作/follow_stage 只同步一次/
空列表 reason**。

覆盖（Flutter）：`applyWallpaperPatch` 三种 patch + 空列表跳过 + `index % len` 取模 +
非法/未知 patch 不猜、`stagePlaylist` 的 `toJson → jsonEncode → jsonDecode → fromJson`
往返、旧存档/坏值回落、三条预算（单项超限丢弃 / 总长保留前面的 / 项数截断）、
append 三种失败原因（列表原样不动）、`shouldPollWallpaperState` 停用与 `off` 不轮询、
轮询间隔 ≥5 s、列表长度写回判据与「整份替换只覆盖 `playlist_len`」、
`WallpaperApi.state()` 的 200 / 503 / 404 / 500 四态。

涉及文件：

- `crates/live2d-ai-mod-wallpaper/src/lib.rs`（Mod 集成：descriptor / schema / runtime / factory / `state_json`）
- `crates/live2d-ai-mod-wallpaper/src/strategy.rs`（纯策略 + `to_prefs_patch` 投影 + `playlist_len` 钳位）
- 根 `Cargo.toml`（members 一行）、`crates/live2d-ai-desktop/Cargo.toml`（path 依赖一行）
- Flutter：`lib/settings/display_prefs.dart`（`stagePlaylist` + `applyWallpaperPatch` + 预算常量）、
  `lib/api/wallpaper_api.dart`（读状态 + 两个纯判据）、`lib/app/shell_wallpaper.dart`（轮询闭环）、
  `lib/app/shell_prefs.dart`（加入 / 清空轮播）、`lib/settings/sections/appearance_section.dart`（两个按钮）、
  `test/wallpaper_wiring_test.dart`

## 7. 已知缺口 / 下一步

1. **不追帧是有意的限制**：一次快照至多前进一格，长挂起后不补播（§4 第 1 条）。
2. **列表没有删除单张 / 拖排序**：只有「加入轮播」与「清空轮播」。真正多图管理
   （缩略图、删除、排序、导入目录）属后续 rc——本轮刻意不做「大轮播」。
3. **列表项不做过期 / 引用计数**：`stagePlaylist` 与 `stageImage` 可能持有同一张图的
   两份 dataURL（同一条记录里重复存），字符预算已经把它算进去了，但没有去重。
4. **前端不做裁剪**：超大图仍只能「本次会话有效，不写盘」（与 rc.5 同一条已知缺口）。
5. **`playlist_len` 手填与前端写回会互相覆盖**：Mod 面板里手改它之后，下一次列表
   变化会被前端写回覆盖——它是**前端维护的运行值**，不是用户偏好（schema 里保留
   只是为了让配置面完整）。

## 8. 非目标

- 动态壁纸（视频 / shader / 桌面级壁纸）——本 Mod 只管**背景图**；
- 与舞台**各自独立**的壁纸轮播（rc.5 已把壳与舞台收敛为「一份真相」）；
- 在线图源 / 拉图 / 缓存；
- 把壁纸 Mod 变成动作 / 编舞的驱动方（动作在产品路径上不存在，见
  [`core-chain-baseline.md`](core-chain-baseline.md) §3.1）。
