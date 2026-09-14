# REGISTER — `wallpaper-wire`（Wave 2 B 轨 → 集成 PR 用）

> 本文件**不是**注册动作本身，而是交给收束 PR 的**交付说明 + 待办**。
> 分支 `mod/wallpaper-wire`，worktree `/home/skystar/Live2D-Ai-w2-wall`，
> 基座 **`429609f2`**（Wave 2 基座：`ModRuntime::state_json` + `GET {id}/state`）。
> 范围真源：[`PARALLEL-WAVE2-2026-09-14.md`](PARALLEL-WAVE2-2026-09-14.md) §3B。
> 设计：[`../../architecture/wallpaper-mod-v0.md`](../../architecture/wallpaper-mod-v0.md) §3/§5。
>
> **本分支不改注册表**：`wallpaper` 早在 `0.2.0-rc.2` 就已注册
> （`AVAILABLE_MOD_FACTORIES` 5 项之一，**缺省停用**）。本轮只做**接线**，
> 不碰 FACTORIES / `mod_count_*` / 缺省 manifest / 全局版本。

## 1. 一句话

占位结束：决策不再「warn + 返回 false」，而是
`WallpaperDecision::to_prefs_patch()` → `state_json().prefs_patch` →
Flutter 纯函数 `applyWallpaperPatch` → 既有 `DisplayPrefs` / `Live2DStage.sendStageBg`；
列表来源 = Flutter 自己的 `DisplayPrefs.stagePlaylist`（在既有「舞台背景图」区
加「加入轮播」/「清空轮播」）。

## 2. 本分支做完的（worker 侧）

| 项 | 位置 | 说明 |
| --- | --- | --- |
| **占位删除** | `live2d-ai-mod-wallpaper/src/lib.rs` | `apply_decision`（对 `SyncStage` / `Advance` warn + `false`）**整段删除**；假话从源码与文档里一起消失 |
| 真投影（纯函数） | `src/strategy.rs` `WallpaperDecision::to_prefs_patch()` | `None` / `{"sync_shell_stage_bg":true}` / `{"stage_index":N}`，三态可单测 |
| `decision` JSON | `src/strategy.rs` `to_json()` | `kind` ∈ `none` / `sync_stage` / `advance`（`advance` 带 `index`） |
| `reason` 诊断 | `src/strategy.rs` `reason(mode, len)` | `mode_off` / `already_synced` / `playlist_empty` / `interval_not_due` / `sync_stage` / `advance` |
| `state_json()` | `src/lib.rs` `impl ModRuntime` | 内部 `Instant` 算增量毫秒喂既有 `tick(delta_ms)`；返回 §3B 的**六个键**；**只在这里推进时钟**，不起线程 / 不订阅事件 / 不写盘 |
| `playlist_len` 配置 | `src/lib.rs` schema + `src/strategy.rs` `playlist_len_from_config` | number，钳 `0..=MAX_PLAYLIST_LEN`（16），缺省 0；`start` 注册与静态 schema 仍是**同一份** |
| 回归 | `src/lib.rs` + `src/strategy.rs` | `state_json` 形状 / 时钟推进 / off 不动作 / follow_stage 只同步一次 / 空列表 reason / `playlist_len` 两端钳位 / 重配置重读；**34 passed** |
| 偏好字段 | `shell/flutter/lib/settings/display_prefs.dart` | `stagePlaylist`（`List<String>` dataURL，缺省 `const []`），与 `stageImage` 同一条 localStorage 记录、同一份预算 |
| 落点纯函数 | 同上 `applyWallpaperPatch(prefs, patch)` | 三种 patch + 空列表跳过（`playlist_empty`）+ `index % len` 取模 + 非法/未知不猜 |
| 三条预算 | 同上 `kStagePlaylistMaxItems`(16) / `kStagePlaylistMaxChars` / `kStageImageMaxChars` | 见 §3；`appendToStagePlaylist` / `readStagePlaylist` 逐条把关 |
| UI 最小操作 | `settings/sections/appearance_section.dart` | 既有「舞台背景图」区加「加入轮播」/「清空轮播」+ **显示当前张数**；超限时给可读文案，列表不动 |
| 闭环接线 | `app/shell_wallpaper.dart`（新 `part`）+ `app/shell_prefs.dart` + `app/shell_admin.dart` + `main.dart` | 见 §4 |
| 读取面 | `lib/api/wallpaper_api.dart`（新） | `GET …/wallpaper/state` + 两个纯判据（`shouldPollWallpaperState` / `needsPlaylistLenWriteBack`）+ `configWithPlaylistLen`；**零新依赖** |
| Flutter 回归 | `test/wallpaper_wiring_test.dart` | **24 条**：三种 patch / 空列表 / 取模 / 往返序列化 / 坏值 / 三条预算 / append 三态 / 停用不轮询 / 间隔 ≥5 s / 写回判据 / API 四态 |

## 3. 播放列表预算（**主 agent 追加的硬约束**，已照做）

localStorage 是**一条记录**存整份 `DisplayPrefs`；**一旦超配额，整份偏好都写不进去**
（连主题 / 音量 / 口型一起丢，rc.5 的教训）。所以列表有**三条独立预算**：

| 预算 | 常量 | 读取（`readStagePlaylist`） | 追加（`appendToStagePlaylist`） |
| --- | --- | --- | --- |
| 单项长度 | `kStageImageMaxChars`（1 500 000 字符） | 非字符串 / 空串 / 超限 → **丢该项**，后面的继续读 | 报 `item_too_large`，列表不动 |
| 列表总长 | `kStagePlaylistMaxChars = kStageImageMaxChars` | 会把总长推过上限 → **停止**（保留前面的，丢掉放不下的） | 报 `budget_exceeded`，列表不动 |
| 项数 | `kStagePlaylistMaxItems = 16` | 达到即停止 | 报 `limit_reached`，列表不动 |

Rust 侧对应 `MAX_PLAYLIST_LEN = 16`（**同一个数**，两边各自钳一次）。
所有失败都**不抛异常**：UI 把 `reason` 翻成一句可读文案
（「轮播列表已到上限 16 张——先『清空轮播』再重新加入」等）。

## 4. 闭环节拍（收束方需要知道的运行语义）

| 环节 | 约定 |
| --- | --- |
| 轮询门禁 | `shouldPollWallpaperState(registered, enabled, mode)`：三者同时成立才轮询；**停用 / `mode=off` → `Timer` 立即取消** |
| 轮询间隔 | `kWallpaperPollInterval = 5 s`（契约下限，纯测例断言 `>= 5s`） |
| 时钟 | Mod 的节拍**就是**这次轮询调用的增量毫秒（`state_json` 内 `Instant`）；前端不额外发时钟信号 |
| 落点 | `prefs_patch` → `applyWallpaperPatch` → `DisplayPrefs` → 既有 `Live2DStage.sendStageBg`（不碰 `stage_bg.rs` / framebuffer / wasm） |
| 写回 | 列表长度 ≠ 服务端 `playlist_len` → 既有 `POST /api/v1/mods/wallpaper/config`，body 从服务端现有 config 出发**只覆盖 `playlist_len`**（该端点整份替换，否则会抹掉 `mode` / `interval_secs`）；长度没变**不写**（写回会 restart Mod、冲掉游标） |
| 启动引导 | `main.dart::initState` 里 `_bootstrapWallpaper()` **只多一个** `GET /api/v1/mods`（不能等用户打开设置面才启用闭环） |
| 503 / 404 | `state_unavailable` / `not_found` 都按「暂时读不到」处理（下一次轮询再试），**不弹错误横幅** |
| 列表为空 | `reason = playlist_empty`，`prefs_patch = null`，前端 `applied=false` → **明确跳过**，`stageImage` 保持不动 |

## 5. 集成 PR / 收束待办

- [ ] 合入 `crates/live2d-ai-mod-wallpaper/`（`lib.rs` + `strategy.rs`）
- [ ] **不加** FACTORIES 行、**不改** `mod_count_*` / `mod_factory_ids_match_expected`
      / 缺省 manifest（wallpaper 已在表里且缺省停用；本轮 FACTORIES 5 → 6 的 `memory`
      由 C 轨负责）
- [ ] 合入 Flutter：`lib/settings/display_prefs.dart`、`lib/api/wallpaper_api.dart`、
      `lib/app/shell_wallpaper.dart`、`lib/app/shell_prefs.dart`、
      `lib/app/shell_admin.dart`、`lib/main.dart`、
      `lib/settings/sections/appearance_section.dart`、`lib/app/browser_io.dart`（仅头注）
- [ ] 合入文档：`docs/architecture/wallpaper-mod-v0.md` §1/§3/§5/§6/§7 + 本文件
- [ ] 合入测试：`shell/flutter/test/wallpaper_wiring_test.dart`
- [ ] 跑全量门禁（见 §6）并重跑 `flutter analyze && flutter test`
- [ ] 发布说明记一条：**壁纸决策已接线**（用户可见：启用 wallpaper 且 `mode != off`
      时舞台会按列表换图；`DisplayPrefs.stagePlaylist` 是新字段，旧存档读成空列表）
- [ ] 不改 `mod-product-chain.md` §5 表（wallpaper 行已存在）

## 6. 门禁实跑结果（本分支）

```text
cargo test -p live2d-ai-mod-wallpaper                              # 34 passed; 0 failed
cargo fmt --all -- --check                                         # clean
cargo clippy -p live2d-ai-mod-wallpaper --all-targets -- -D warnings  # 0 warning
cargo test --workspace --all-targets                               # （见 §7 报告）
cd shell/flutter && flutter analyze && flutter test                # No issues; 857 passed
```

## 7. 红线复述（本分支逐条遵守）

- **不改** `crates/l2d-wasm-demo/`（含 `stage_bg.rs`）/ framebuffer / `LoadOp` / wasm 任何文件；
- **不做**在线拉图 / 图库服务 / 大轮播 / 删除单张 / 拖排序；
- **不改** `AVAILABLE_MOD_FACTORIES` / `mod_count_*` / `mod_factory_ids_match_expected` /
  `default_mods_manifest` / 版本号 / `AGENTS.md`；
- **不改**基座文件（`topics.rs` / `factory.rs` / `mod_registry.rs` / `mods_routes.rs` /
  `supervisor.rs`）——`state_json` 只**实现**基座 trait，不改 trait；
- **不唤醒** `action_tx`；**不**在 Mod 里起线程 / 开 socket / 读进程环境。

## 8. 未决 / 已知缺口

1. 列表**没有删除单张 / 排序 / 缩略图**（只有加入与清空）——「大轮播」明确不在本轮。
2. `stagePlaylist` 与 `stageImage` 可能重复持有同一张 dataURL（无去重）；字符预算已计入。
3. `playlist_len` 在 Mod 面板可手填，但它由前端维护：下一次列表变化会覆盖手填值
   （文档已写明，它是运行值而非用户偏好）。
4. 前端仍**不做图片裁剪**：超大图只能「本次会话有效，不写盘」（rc.5 同款已知缺口）。
5. 未做端到端点火肉眼验收（需启用 wallpaper + 至少两张列表图）；本分支只覆盖
   纯逻辑与 API 四态，真实 UI 验证留给收束后的 `ignite.sh` 验收。
