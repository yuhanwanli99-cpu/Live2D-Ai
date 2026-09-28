# BATCH-0015 — Phase 2 · 横扫 C：竞态 / 取消 / 三态纪律

> 账本：`AUDIT-B/`。

## 一、`.timeout` 覆盖面（全仓 5 处，全在「可能永久挂起的平台操作」上）

| 站点 | 对象 | 超时 | 判定 |
|---|---|---|---|
| `main.dart:182` | `createBackgroundStore()` | 3 s | ✓（IndexedDB 隐私模式下既不 resolve 也不 reject） |
| `main.dart:193` | `hydrateBackgrounds(...)` | 3 s | ✓ |
| `data/background_store_web.dart:106` | IndexedDB open | 有 | ✓ |
| `data/background_store_web.dart:159` | 单次操作 `done.future` | 有 | ✓ |
| `live2d_bridge.dart:161` | 等 `loaded` 回执 | 15 s | ✓（且只捕 `TimeoutException`） |

**刻意不给普通 HTTP 调用加 timeout**（慢 ≠ 坏，应当显示「慢」而不是「失败」）——这是设计选择，不是缺口。**但它与 F-0008-1 合起来意味着：全仓没有任何「整体卡住」的探测器**，唯一的兜底是用户手动刷新。

## 二、加载三态盘点（用户可见的 6 条加载路径）

| 路径 | loading | error | empty | 兜底 catch | finally 复位 | 判定 |
|---|---|---|---|---|---|---|
| `SettingsController.load()` | ✓ | ✓ | （读不到=有文案） | ✓ `catch (e)` | ✓ | **样板** |
| `memory_panel._refreshList` | ✓ | ✓ | ✓ | ✓ | — | ✓ |
| `dev_tools._loadState` | ✓ | ✓ | — | 失败原样抛给面板 | — | ✓ |
| `voice_input_panel` | ✓ | ✓ | ✓ | ✓ | — | ✓ |
| **`shell_admin._loadAdmin`** | ✓ | ✓ | — | ✗ 只 `on ApiException` | ✗ 标志在两个分支里各写一次 | **F-0015-1** |
| **`shell_admin._activateModel / _importModel / _toggleMod`** | ✓（`_busyId`） | ✓ | — | ✗ 同上 | ✗ | **F-0015-1** |

## 三、B1 未核实候选的处置
> 「非 ApiException 逃逸 → `_loadAdmin`/`_testLlm` 卡 loading」

**降级为 P3 并给出证据**：本批逐个核了 `ModelInfo/ModInfo/AppCapabilities/LogLine/EnvKey/SettingsTestOutcome` 的解析器，**全部用宽容的 `_str`/`_int` 辅助**（`value is String ? … : 默认值`），**没有一个会抛**；`ApiClient._guard` 又把传输层异常统一包成 `ApiException('network_error')`。所以「逃逸」的现实概率很低——但**结构性缺口仍在**（一旦逃逸，`_busyId`/`_adminLoading` 永久停在「处理中」，而 `SettingsController` 的样板就在同一个应用里）。

## 发现
- **F-0015-1（P3）** `shell_admin` 的 5 条用户可见操作只捕 `on ApiException`、且靠分支内手动复位 busy 标志，缺 `SettingsController` 那套「兜底 catch + finally」样板。
- **F-0015-2（P3）** `swapModel` 超时后 `_modelLoads.stream.first` 的订阅**没有取消**，桥随后 dispose 时该 future 以 `StateError` 完成 ⇒ **未处理的异步错误**逃到 zone。

## 本批核对过、不成发现的（正面记录）
- 模型切换链路的诚实度是全仓最佳之一：三种结局（已确认 / 服务端称需重启 / 舞台未回执）各有**不同**文案，且都指向可执行的下一步（重试或看舞台错误层）。
- `ApiClient._guard` 把「一切传输异常」收敛成一个码 `network_error`，让上层的 `on ApiException` 分支真的能覆盖住绝大多数故障——这是让 F-0015-1 只剩 P3 的关键。
- `ws_liveness.isLiveSocket` 把「僵尸 socket」判据抽成纯函数并有回归，B8 复核确认仍成立。

## 本批未核实
- F-0015-2 的「未处理错误」在 Flutter Web 里的实际表现（控制台 uncaught vs 静默）需真机 DevTools；`flutter test` 环境下会直接 fail（未处理异常会被测试框架捕获），这一点是可验证的。
