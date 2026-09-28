# BATCH-0010 — Phase 1 · lib/live2d/ 全量（13 文件）

> 账本：`AUDIT-B/`。行号基于 HEAD `5ef879f4`。

## 本批读过
| 文件 | 行 | 一句话 |
|---|---|---|
| lib/live2d/live2d_host_web.dart | 88 | 全读：iframe `src='/render'`（同源相对路径，无用户输入拼接）；**收发两侧都校验**（收：`origin == location.origin` **且** `source == iframe.contentWindow`；发：`postMessage(json, location.origin)`）；listener 在 dispose 摘除 ✓ —— G 红线本域无问题 |
| lib/live2d/live2d_bridge.dart | 508 | 读 300-508：v1 信封 `{version:1,type,payload}`；就绪前入队 + **mouth 只留最新** + 队列有上限；`_flush` 在 `ready` 时按序补发；`stage-ack` 按**历史形状**（字段挂根上）解析并写明理由 ✓；dispose 五个流/Timer/transport 全收 ✓ |
| lib/live2d/stage_pointer_interceptor.dart | 99 | 全读：根因（两张画布 `pointer-events:none` + iframe `auto`）写进注释；`enabled` 开关为折叠面板而设（不挂载只改样式，保住子树）✓；**诚实记录了垫层管不到的两处**（bottom sheet 的模态障碍层与拖拽把手） |
| lib/live2d/session_baseline.dart | 161 | 读 1-60：两路交付（HTTP body / action_cue 帧）**共用同一个解码器**；空 baseline → 恰好一次撤销（`id=='none'`）✓；不建前端镜像 ✓ |
| lib/live2d/stage_cancel.dart | 53 | 全读：四步编排纯逻辑，`lastSteps` 供回归 |
| lib/live2d/live2d_stage.dart | 666 | 读 240-350 + 495-600：`_onBridgeChanged` 转发 phase/error/ack/ready **但不转发 progress**（F-0010-2）；FPS 徽标在 build 里**现读** `bridge.fps`（同一条链的后果） |
| v1 契约核对（只读 Rust） | — | `crates/l2d-wasm-demo/src/main.rs:391-470` vs `live2d_bridge.dart:198-234`：`sync`/`stage`/`preset`/`mouth`/`stage-clock`/`destroy` 六个 type **逐字一致**；payload 键 `scale/dark/stageColor/lipSync/mouthSensitivity/actionScales{head,body,expression}/idleEnabled/clickEnabled/tier/model` **逐字段对齐**；渲染面兼容无 `payload` 的历史扁平帧 ✓（Q 红线） |
| 红线 M 覆盖盘点 | — | 8 个使用点，**8 个都有断言**，分布在三个文件：`stage_pointer_interceptor_test`(7 处：断线横幅/角标/loading/error/三宿主设置) · `confirm_discard_test.dart:141` · `session_sheet_test.dart:354` · `compact_settings_page_test.dart:333` ✓ |

## 发现
- **F-0010-1（P3）** `sync(paused:)` 是**死线字段**：前端三处签名都留着它，渲染面 `crates/` 全树**一次都没读过** `paused`，且无任何调用方。
- **F-0010-2（P2，升级 F-0001-4）** bridge 的 `progress` **与 `fps`** 两类事件都不触发任何重建 ⇒ 加载百分比与 FPS 徽标显示**陈旧值**（fps 每秒一帧；应用安静时徽标永久冻结）。

## 本批核对过、不成发现的（正面记录）
- **v1 协议逐字段对齐**（六个 type + 十个 payload 键），且渲染面显式兼容历史扁平帧与旧 type 名（`sync`→`stage-config`、`mouth`→`audio-volume`）——「只增不改」在本域是**双向**兑现的。
- **G 注入面**：`src` 是常量（无拼接）、postMessage 收发都钉死 origin、接收侧额外校验 `source`、listener 随 transport dispose 摘除。
- `StagePointerInterceptor` 的「诚实记录的边界」很关键：它明写 bottom sheet 的模态障碍层与拖拽把手**罩不到**，并给出取舍理由（铺满会吃掉模型拖动）——这类「知道自己不管什么」是本仓最好的注释类型。
- bridge 的就绪队列会把 **mouth 合并为最新**（`_queue.removeWhere(type=='mouth')`）且有 `maxQueue` 上限，60 条口型不会把 sync 挤掉——这是「高频信号不许淹没低频控制消息」的正确实现。
- `session_baseline` 的「空 baseline 必须恰好撤销一次」被显式钉住（`id=='none'` 哨兵），而不是「什么都不做」——与 `stage_cancel` 的四步顺序同源。

## 本批未核实
- `render_events.dart`(700) 与 `action_scales_sync.dart`(194) 只读了接口面（事件级 ack 的 §7.2 键表、倍率防抖），未逐行；前者是本域最大的文件，留给 Phase 2 的 E 横扫。
- F-0010-2 的「陈旧值」在真机上具体表现（FPS 徽标冻结 / 进度条卡在某个百分比）未截图取证。
