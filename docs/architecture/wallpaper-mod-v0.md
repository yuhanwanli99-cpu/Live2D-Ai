# 壁纸 Mod 策略 v0（`live2d-ai-mod-wallpaper`）

> # ⛔ 已封存（ARCHIVED，2026-09-14）——**非产品路径**
>
> 本 Mod **已移出 `AVAILABLE_MOD_FACTORIES`**（7 → 5），不再注册、不再编译进 binary。
> 用户裁决：本波**删除并封存 wallpaper Mod，不做本地下载库 / 自由切换**。
> **用户手动的舞台/壳背景能力继续保留**（`DisplayPrefs.stageImage` / `stagePlaylist` /
> `syncShellStageBg` 与「外观与互动」里的选图/清图 UI）——被封存的只是**自动换壁纸策略**。
> crate 已于 **2026-10-01（W2-A / D1 第一段）物理删除**（只存在于 tag `checkpoint/pre-d1-dormant`），**禁止挂回**；原因与恢复条件见
> [ARCHIVED-mods.md](ARCHIVED-mods.md)。**下面正文是封存前的历史记录。**
>
> **状态（历史）**：2026-09-14 Wave 1（分支 `mod/wallpaper` @ `2d492447`）起草；
> `0.2.0-rc.2` 集成时按
> [`../plans/parallel-mods/REGISTER-wallpaper.md`](../plans/parallel-mods/REGISTER-wallpaper.md)
> **曾注册**进 `AVAILABLE_MOD_FACTORIES`（封存前工厂数由 `mod_count_is_seven` 守住，**缺省停用**；已于产品级加强波次**封存**，见顶部横幅）。
> **2026-09-14 Wave 2（分支 `mod/wallpaper-wire` @ 基座 `429609f2`）§5 落点已接线**：
> Mod 决策 → `state_json` 的 `prefs_patch` → Flutter 纯函数 `applyWallpaperPatch`
> → 既有 `DisplayPrefs` / `Live2DStage.sendStageBg`；播放列表来源 = Flutter 自己的偏好
> `DisplayPrefs.stagePlaylist`。接线清单见
> [`../plans/parallel-mods/REGISTER-wallpaper-wire.md`](../plans/parallel-mods/REGISTER-wallpaper-wire.md)。
>
> **2026-09-14 Wave 3（分支 `mod/w3-wall` @ 基线 `118bd435`）列表可维护 + 字段不再自相矛盾**：
> ① **列表增删 / 排序**做实——纯函数 `removeStagePlaylistAt` / `moveStagePlaylist` /
> `stagePlaylistIndexOf`（`display_prefs.dart`，VM 可测）+ 「舞台背景图 → 背景轮播列表」
> 每张一行的「上移 / 下移 / 删除」文字按钮；② **`playlist_len` 从 `settings_spec` 删除**
> （它是运行值，真源 = Flutter `stagePlaylist.length`，手填必被写回覆盖）——只在
> `state_json` 与 `GET /api/v1/mods` 的 `config` 里**只读**暴露；③ 写回 `playlist_len`
> 收进 `_updatePrefs` 单一漏斗（长度变了才写）；④ `state_json` 的 `follow_stage` /
> `interval` 轨迹各补一条可脚本验证的回归。**Rust 单测 34 → 37；Flutter 测试 +10。**
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
| 大轮播 / 图库 / 在线拉图 / 缩略图 / 拖拽排序 | **不做**。`interval` 只按**调用方给的列表长度**推进一个游标；列表编辑是**文字按钮**（Wave 3），不引入图源服务 / 图片预览 / 拖拽手势 |

## 3. 配置契约（`settings_spec` v1：**两个可编辑字段**）

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

### 3.1 `playlist_len` 是**运行值**，不是可编辑字段（Wave 3）

`playlist_len`（播放列表长度）**不在** `settings_spec` 里（Wave 2 曾放进去，Wave 3 删除）：

- **真源** = Flutter `DisplayPrefs.stagePlaylist.length`（图住 Flutter，Mod 不持图）；
- Flutter 在长度变化时用既有 `POST /api/v1/mods/wallpaper/config` 写回（**整份替换**，
  所以是从服务端现有 config 出发只覆盖这一个键）；
- 它仍**只读**出现在两处：`state_json` 顶层的 `playlist_len` 与 `GET /api/v1/mods` 的
  `config`（`redacted_config` 只剥 `secret` 字段，非 secret 键原样回）；
- 留在表单里手填必被下一次写回覆盖——**自相矛盾的字段从 schema 里一起消失**
  （回归：`static_spec_fields_and_no_enabled` + `playlist_len_is_run_value_read_from_config_and_state`）。

读取时的钳位口径不变（`playlist_len_from_config`）：负数 / 非数字 / 缺失 → `0`；
超上限 → 钳到 `MAX_PLAYLIST_LEN`（16）。

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
区里提供一组最小操作，并显示当前张数：

| 操作 | 纯函数（`display_prefs.dart`） | 说明 |
| --- | --- | --- |
| **加入轮播** | `appendToStagePlaylist` | 把当前 `stageImage` append 进列表（三条预算把关） |
| **清空轮播** | — | 列表清空；`stageImage` 不动 |
| **删除**（第 N 张） | `removeStagePlaylistAt` | 越界原样返回；删当前张也**不清空舞台** |
| **上移 / 下移** | `moveStagePlaylist` | 首尾按钮禁用；项数与项集合不变 |
| **「当前」标记** | `stagePlaylistIndexOf` | 按 dataURL **数据相等**找当前张，不用 Mod 游标 |

删除 / 重排只会让列表变小或重排，**不可能让列表超预算**，所以这两类操作不做预算校验；
只有「加入」需要三条预算。**不做**拖拽排序与缩略图（见 §7 / §8）——拖拽要靠 gesture +
重排动画 + 落点判定，几乎全是不可单测的代码；文字按钮把「删哪张 / 移到哪」变成
两个可 VM 断言的纯函数。

三条预算（**常量即契约**，Flutter 与 Rust 各自钳一次）：

| 预算 | 常量（Flutter / Rust） | 行为 |
| --- | --- | --- |
| 单项长度 | `kStageImageMaxChars` / —（Rust 不持图，只看项数） | 超限的项**在读取时丢弃**；append 报 `item_too_large` |
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
   写回的**唯一漏斗**是 `shell_prefs.dart::_updatePrefs`：加入 / 清空 / 删除 / 上移下移
   都经过它，`next.stagePlaylist.length != widget.prefs.stagePlaylist.length` 为真才发请求
   ——所以上移下移（长度不变）不会写回，也不会重启 Mod。
4. `503 state_unavailable`（刚停用 / worker 正持锁）是**正常**：跳到下一次轮询，
   不弹错误。

### 5.5 列表为空的情形（**明确跳过**）

`interval` 模式下列表为空时，策略层直接返回 `None`（`reason = playlist_empty`），
`prefs_patch` 为 `null`；前端 `applyWallpaperPatch` 返回
`applied=false, reason='playlist_empty'`。**这不是失败，也不是「假装成功」**：
它以 `reason` 如实报告，`stageImage` 保持不动（用户当前那张仍然在舞台上）。
「加入轮播」在无当前图 / 超预算 / 到上限时同样给一句可读文案，列表不动。

## 6. 门禁与测试

- `cargo test -p live2d-ai-mod-wallpaper` → **37 passed / 0 failed**（0 doc-test，纯逻辑无需示例）；
- `cargo fmt -p live2d-ai-mod-wallpaper -- --check` clean；
- `cargo clippy -p live2d-ai-mod-wallpaper --all-targets -- -D warnings` → 0 warning；
- Flutter：`flutter analyze` 无问题 + 全量 `flutter test` **867 passed**
  （`test/wallpaper_wiring_test.dart` 本轮 **+10**，共 34 条）。

覆盖（Rust）：模式映射与缺省、间隔缺省/钳位/非数字、`playlist_len` 缺省/钳位/超上限、
`off` 恒不动作、`follow_stage` 只同步一次且重配置后重新同步、`interval` 首帧/到点才换/
取模回绕/空列表保持/长挂起不追帧/`u64::MAX` 饱和、`set_playlist_len` 钳位与清空、
`reconfigure` 复位并重读 `playlist_len`、schema **两字段**与选项顺序且无 `enabled` /
`playlist_len`、`playlist_len` 只读面（config + `state_json`）、
`start` 只注册 settings 不订阅事件、`to_prefs_patch` 三态、`to_json` 的 `kind`、
`reason` 六种取值、**`state_json` 形状/时钟推进/off 不动作/空列表 reason**、
**两条 `state` 轨迹**（`state_trajectory_interval_advances_index_over_time`：
`advance 0 → none → advance 1 → advance 2 → advance 0` 逐拍断言 kind/index/patch/reason；
`state_trajectory_follow_stage_syncs_then_resyncs_after_reconfigure`：
`sync_stage` → `none` → 重配置后再 `sync_stage`）。

覆盖（Flutter）：`applyWallpaperPatch` 三种 patch + 空列表跳过 + `index % len` 取模 +
非法/未知 patch 不猜、`stagePlaylist` 的 `toJson → jsonEncode → jsonDecode → fromJson`
往返、旧存档/坏值回落、三条预算（单项超限丢弃 / 总长保留前面的 / 项数截断）、
append 三种失败原因（列表原样不动）、`shouldPollWallpaperState` 停用与 `off` 不轮询、
轮询间隔 ≥5 s、列表长度写回判据与「整份替换只覆盖 `playlist_len`」、
`WallpaperApi.state()` 的 200 / 503 / 404 / 500 四态、
**列表编辑**（`removeStagePlaylistAt` 的正常 / 边界 / 越界 / 入参不被改、
`moveStagePlaylist` 上移下移 / 越界 / 原地 / 项集合不变、`stagePlaylistIndexOf`、
删除触发写回而重排不触发、`append → move → remove` 组合闭环、编辑不动 `stageImage`）。

涉及文件：

- `crates/live2d-ai-mod-wallpaper/src/lib.rs`（Mod 集成：descriptor / schema / runtime / factory / `state_json` / 轨迹回归）
- `crates/live2d-ai-mod-wallpaper/src/strategy.rs`（纯策略 + `to_prefs_patch` 投影 + `playlist_len` 钳位）
- 根 `Cargo.toml`（members 一行）、`crates/live2d-ai-desktop/Cargo.toml`（path 依赖一行）
- Flutter：`lib/settings/display_prefs.dart`（`stagePlaylist` + `applyWallpaperPatch` + 预算常量 +
  `removeStagePlaylistAt` / `moveStagePlaylist` / `stagePlaylistIndexOf`）、
  `lib/api/wallpaper_api.dart`（读状态 + 两个纯判据）、`lib/app/shell_wallpaper.dart`（轮询闭环）、
  `lib/app/shell_prefs.dart`（加入 / 清空 + 写回唯一漏斗 `_updatePrefs`）、
  `lib/settings/sections/appearance_section.dart`（轮播行 + `_StagePlaylistEditor`）、
  `test/wallpaper_wiring_test.dart`

## 7. 已知缺口 / 下一步

1. **不追帧是有意的限制**：一次快照至多前进一格，长挂起后不补播（§4 第 1 条）。
2. **列表项不做过期 / 引用计数**：`stagePlaylist` 与 `stageImage` 可能持有同一张图的
   两份 dataURL（同一条记录里重复存），字符预算已经把它算进去了，但没有去重。
3. **前端不做裁剪**：超大图仍只能「本次会话有效，不写盘」（与 rc.5 同一条已知缺口）。
4. **未做端到端点火肉眼验收**：需启用 wallpaper + 至少两张列表图才能看到真实换图，
   本分支只覆盖纯逻辑与 API 四态，肉眼验收留给收束后的 `ignite.sh`。

> Wave 2 记的两条缺口已在 Wave 3 关闭：①「列表没有删除单张 / 排序」→ 现在有
> `removeStagePlaylistAt` / `moveStagePlaylist` + 每行文字按钮；②「`playlist_len` 手填与
> 前端写回互相覆盖」→ 它已**移出** `settings_spec`，只在 `state_json` / `config` 里只读
> 暴露（§3.1）。

## 8. 非目标

- **在线图库大轮播**：不做图源服务 / 批量导入目录 / 在线拉图 / 缓存；
- **缩略图**：列表行只显示序号、大小与「当前」标记，不渲染图片预览；
- **去重**：不比较 dataURL 内容，同一张图可以被加入多次（预算已计入）；
- 动态壁纸（视频 / shader / 桌面级壁纸）——本 Mod 只管**背景图**；
- 与舞台**各自独立**的壁纸轮播（rc.5 已把壳与舞台收敛为「一份真相」）；
- 拖拽排序（用文字按钮替代，理由见 §5.3）；
- 把壁纸 Mod 变成动作 / 编舞的驱动方（动作在产品路径上不存在，见
  [`core-chain-baseline.md`](core-chain-baseline.md) §3.1）。
