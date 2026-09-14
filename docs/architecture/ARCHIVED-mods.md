# 已封存的 Mod（ARCHIVED）

> **这是什么**：项目里**曾经注册、现已明确不做**的 Mod 的封存台账。
> 与 `local-llm` 的 **DEPRECATED / 已废除启动**是同一口径：
> **移出 `AVAILABLE_MOD_FACTORIES` = 不在产品路径上**；crate 暂留 workspace 只是为了
> 避免一次性大爆炸（仍可 `cargo test`），**禁止挂回**。
>
> **恢复条件**写在每一节里——没有满足条件之前，任何人不得把这两个工厂加回
> `crates/live2d-ai-desktop/src/main.rs::AVAILABLE_MOD_FACTORIES`。
> 数量护栏是 `main.rs::mod_count_is_five`（Wave 3 的 7 → 产品级加强波次的 **5**）。

---

## 0. 当前注册面（封存后）

`AVAILABLE_MOD_FACTORIES` **恰好 5 个**（顺序即表内顺序）：

| # | id | 缺省 |
| --- | --- | --- |
| 1 | `external-input` | **on**（唯一缺省启用） |
| 2 | `persona` | off |
| 3 | `voice-input` | off |
| 4 | `memory` | off |
| 5 | `director` | off |

`live2d-ai-mod-template` 是模板 crate，**不注册**。

---

## 1. `wallpaper` — 封存

| 项 | 值 |
| --- | --- |
| 封存时间 | 2026-09-14（产品级加强波次，未 bump 版本） |
| crate | `crates/live2d-ai-mod-wallpaper` |
| 原职责 | 壁纸策略 v0：`mode`（off / follow_stage / interval）→ `state_json.prefs_patch` → Flutter `applyWallpaperPatch` → 既有 `DisplayPrefs` / `sendStageBg` |
| 现状 | 移出 `AVAILABLE_MOD_FACTORIES`（7 → 5）；移出 `live2d-ai-desktop` 依赖；Flutter 侧 `wallpaper_api.dart` / `shell_wallpaper.dart` / `applyWallpaperPatch` 一并拆除 |
| 架构文 | [wallpaper-mod-v0.md](wallpaper-mod-v0.md)（顶部有封存横幅，正文是封存前记录） |

### 1.1 为什么封存

1. **用户裁决（本波范围）**：wallpaper Mod 删除并封存，本波不做；**也不实现本地下载库 /
   自由切换**。理由：它是「自动换壁纸」策略，而产品上真正被用户使用的显示能力是
   **手动选舞台/壳背景**——后者属于 `DisplayPrefs`，与 Mod 无关，继续保留。
2. **没有独立的产品价值**：自动策略的落点（`DisplayPrefs` 的 `stageImage` /
   `syncShellStageBg`）本身就是用户可用手点到的能力；Mod 只负责「什么时候替你点」，
   收益（省一次点击）抵不上它带来的常驻轮询与状态面维护成本。

### 1.2 明确**没有**被删掉的东西（不要连坐）

- `DisplayPrefs.stageImage` / `shellImage` / `stagePlaylist` / `syncShellStageBg` /
  `effectiveShellImage`：**用户可手动选图/清图/增删排序**，全部保留；
- 「外观与互动」里的舞台/壳背景 UI、rc.5 的 `ShellBackdrop` 与 stage-bg 协议：保留；
- `l2d-wasm-demo` 的 framebuffer 背景预通道：**一行未动**。

### 1.3 恢复条件（满足全部才可挂回）

1. 先立项论证「自动换壁纸」相对「手动选图」的**独立价值**（用户裁决已否决一次）；
2. 给出常驻轮询/状态面的成本结论，并说明它为什么必须是一个 Mod 而不是主链能力；
3. 同步恢复 `mod_count_is_*` / id 断言与 `mod-product-chain` §5 表格；
4. 恢复 Flutter 落点（`applyWallpaperPatch` 等）时**不得**改动 `DisplayPrefs` 的用户语义。

---

## 2. `pet-desktop` — 封存

| 项 | 值 |
| --- | --- |
| 封存时间 | 2026-09-14（产品级加强波次，未 bump 版本） |
| crate | `crates/live2d-ai-mod-pet-desktop` |
| 原职责 | 桌宠配置 / 事件态的**可测状态面**（`GET /api/v1/mods/pet-desktop/state`），`window.opened` 恒 `false`、`reason=native_shell_dormant` |
| 现状 | 移出 `AVAILABLE_MOD_FACTORIES`（7 → 5）；移出 desktop 依赖；不再出现在 `GET /api/v1/mods` |
| 架构文 | [pet-desktop-mod-v0.md](pet-desktop-mod-v0.md)（顶部有封存横幅） |

### 2.1 为什么封存

1. **用户裁决**：pet-desktop 是目前最重的一块，**封存、暂时不推**；本波**不做真窗 /
   应用级桌宠**。
2. 它从来没有硬闭环：`window.opened` 恒 `false` 是既定裁决，所谓「软闭环」的终点
   只是一个**状态面**——既然窗口不做，这个状态面本身也没有产品出口；
3. 真窗的前置条件（`core-chain-baseline.md` §3.6 的休眠台账：先论证「谁来维护第二个
   UI 壳」）从未被满足。

### 2.2 明确**没有**被删掉的东西

- `crates/live2d-ai-desktop` 的 egui 原生壳 / 托盘 / `--pet-mode` 代码：按
  `core-chain-baseline.md` §3.6 仍是**休眠保留**（能编译、非主线、不验收）；
- Rust 侧 `live2d-ai-mod-pet-desktop` crate 本身：暂留 workspace，可编译、可跑自身测试。

### 2.3 恢复条件（满足全部才可挂回）

1. 先立项论证「谁来维护第二个 UI 壳」（`core-chain-baseline.md` §3.6 的红线）；
2. 明确「硬开窗口」的产品定义（与 Flutter `/app/` 的关系、验收人、验收方法）；
3. 同步恢复数量/id 断言与 `mod-product-chain` §5 表格；
4. 真窗落地必须单独做 wasm/平台验收，不得顺手在状态面上「假装闭环」。

---

## 3. 与 `local-llm`（DEPRECATED）的关系

三者是同一口径的不同档位：

| 状态 | 含义 | 现行例子 |
| --- | --- | --- |
| 注册在册 | 在产品路径上 | external-input / persona / voice-input / memory / director |
| **ARCHIVED（封存）** | 裁决「本波不做」，crate 暂留，可编译可测，**禁止挂回** | wallpaper / pet-desktop |
| **DEPRECATED（已废除启动）** | 能力被新基线取代，crate 暂留，**禁止挂回** | local-llm |

三者都由 `main.rs` 的**同一个**数量/id 断言护栏守住：任何「顺手挂回去」都会让门禁变红。

---

## 4. 相关文件

- [Mod 产品链路](mod-product-chain.md) §5（现行注册表）
- [Mod 社区许可与注册边界](mod-community-license.md) §4（许可状态表）
- [核心链基线 §3 休眠台账](core-chain-baseline.md)
- `crates/live2d-ai-mod-wallpaper/`、`crates/live2d-ai-mod-pet-desktop/`（crate 头注有 ARCHIVED 横幅）
