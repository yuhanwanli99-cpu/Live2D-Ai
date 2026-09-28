# BATCH-0013 — Phase 2 · 横扫 A：状态源唯一性（全仓）

> 账本：`AUDIT-B/`。方法：不看 feature，只问「这个状态的真源是谁、有几份拷贝、谁负责刷新」。

## 扫描到的状态源与写入点数

| 状态 | 声明处 | 写入点 | 读取点 | 判定 |
|---|---|---|---|---|
| 显示偏好 | `main.dart:134 _prefs` | 3（`:154` 读盘 / `:171` 水合 / `:204` `_update`） | 全部经 `widget.prefs` 下传 | **单一真源** ✓（`ShellRoot.prefs` 只是透传，不是第二份） |
| 背景字节库 | `main.dart:141 _store` | 2（`:141` 初始内存实现 / `:170` 水合换接） | `shell_prefs.dart:247/303/377` | **窗口期指向错误的库（F-0013-1）** |
| 背景轮播索引 | `main.dart:419 _backgroundIndex` + `ShellSlideshow._index` | 各 2 | `app_shell.dart:546-552` | **两份**（F-0001-2 家族，已登记） |
| 界面相位 | `UiStateTracker` 4 个布尔 | 6（`onWsStatus`/`markTurnAccepted`/`consume`/`markStopped`） | `deriveUiPhase` 唯一派生 | **单一真源** ✓（但本地失败无人复位 → F-0007-1） |
| 设置草稿 | `SettingsController.draft` | 全部经 `controller.edit` | 各分区 `effXxx(draft, view)` | **单一真源** ✓（0002 已核 15 字段五件套） |
| 当前轮气泡 | `ChatController._assistant` | 6 | `_appendDelta` 引用语义 | **单一真源** ✓（引用语义刻意，不是下标） |
| 舞台态 | `Live2DBridge` 私有字段 | 1（`_onRaw`） | `Live2DStage._onBridgeChanged` | **单一真源，但只有四类事件有出口（F-0010-2）** |
| 口型电平 | `AudioPlayer._level` | 1（`_tick`） | Stream → GlobalKey | **单一真源** ✓（红线 O 成立） |
| 记忆面板的桶 | 面板内 `_records` | `initState` + `_send` | `build` | **第二份快照**（F-0004-1） |

## 发现
- **F-0013-1（P1，升级 F-0001-3）** 启动水合窗口内：**导入的背景图会「先显示已加入、随后凭空消失」**（字节写进即将被丢弃的内存库 + 偏好项被旧快照整体覆盖），删除/重排同样被回滚，任意偏好改动被吞。窗口长度正比于背景库体积（每张图一次 `readChecked` 读回 base64），IndexedDB 挂死时可达**超时上限 3 秒**。

## 本批核对过、不成发现的（正面记录）
- `DisplayPrefs` **只有一份**：`ShellRoot.prefs` / `AppShell.prefs` / 各分区的 `view` 全是同一条透传链上的引用（`main.dart:224 → app_shell.dart:1125 → section`），B2 的「22 字段五件套」结论成立。
- `UiPhase` 的派生顺序只有一处实现（`ui_phase.dart:123-130`），`StatePill`/舞台角标/聊天面板都读它——§6.3 硬规则 1 在代码层成立。
- `ChatController._assistant` 用**对象引用**而不是下标做「当前轮」游标，多会话/延迟收口都不会串台——与外观库那个「下标当身份」的缺陷（F-0003-3）是同一课题的**正确解法**，可作为改法参考。
- `_ShellPrefsWiring` 是 `_ShellRootState` 的扩展，`widget.store` 永远等于最新的 `_store`（没有缓存副本）——**除**水合窗口外。

## 本批未核实
- 水合窗口的实际长度（取决于背景库大小与磁盘速度）需真机测量；机制层面已闭环。
- `DisplayPrefs` 之外的偏好（`_settings` 控制器的 remote/draft）与 WS 状态的交叉（B 横扫）留待 0014。
