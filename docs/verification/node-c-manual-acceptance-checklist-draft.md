# 节点 C 手动验收清单草稿（C10 拖动 / C11 托盘恢复链路）

> **草稿：待高级 AI 确认后转正式验收清单。**
>
> 适用范围：`docs/plans/node-c-render-platform-audit-brief.md` 中需列入手动验收清单的 C10（拖动人工复验项）与 C11（托盘恢复链路）。本文件只起草操作步骤与判据，不做最终裁决。
>
> 自动化复测条件（须先满足）：
>
> 1. 已阅读 `docs/verification/desktop-platform-smoke-2026-08-26.md` 了解 WSLg 实测矩阵。
> 2. 真实桌面窗口管理器（X11 或 Wayland），**非 WSLg 沙箱**（WSLg 实测已记 drag/tray 不可自动化）。
> 3. 已在仓库根目录构建：`cargo build -p live2d-ai-desktop --release`（产物在 `target/release/live2d-ai-desktop`）。
> 4. CLI 入口一致性：所有命令使用 `cli.rs` 已声明参数（`--chat` / `--pet-mode` / `--window-smoke` / `--model-smoke` / `--smoke-frames` / `--smoke-timeout-secs`），无自造 flag。
>
> 退出码参考（`cli.rs` 末尾说明）：`0` 成功 / `1` 代码或资产错误 / `2` CLI 用法错误 / `3` 运行环境不满足（GPU/显示/声卡）。

---

## 0. 准备：能力表打印与 baseline 对照

每节复测前先在终端跑一次 Info 模式，确认 `RuntimeCapabilities` 表格完整、字段名与本清单一致。

```bash
./target/release/live2d-ai-desktop
```

预期输出（节选，字段顺序以 `platform.rs::RuntimeCapabilities::as_table` 为准）：

```
window_created           true
surface_initialized      true
transparent_window_requested true
alpha_mode               PreMultiplied
transparent_alpha        Available
backend                  X11    # 或 Wayland
always_on_top            <Available|Unavailable|Unknown>
global_position          <Available|Unavailable|Unknown>
click_through            <Available|Unavailable|Unknown>
drag_move                <Available|Unavailable|Unknown>   # C10 关注
tray                     <Available|Unavailable|Unknown>   # C11 关注
always_on_top_active     false
click_through_active     false
```

**判据**：

- 字段名与上表一一对应；任何字段名漂移说明源码已变，本清单需同步更新。
- `drag_move` 初次 Info 输出恒为 `Unavailable`（仅打印、不会调 `drag_window`），需在 C10 复测中以交互触发后再次打印。

---

## 1. C10 拖动人工复验清单

### 1.1 背景与现状

- 拖动代码路径在 `crates/live2d-ai-desktop/src/app/capability.rs:200-222`（`ShellApp::begin_interactive_drag`），左键按下时若 `click_through_active == false` 且后端 `supports_drag_window()`，调用 `Window::drag_window()`；首次成功后将 `runtime.drag_move` 置为 `Available`。
- WSLg 实测（`docs/verification/desktop-platform-smoke-2026-08-26.md` 第 17 行）：代码路径在，但沙箱无法注入真实鼠标拖拽，**记 unavailable**。
- 本节提供真实桌面 WM 下的人工复测步骤与判据。

### 1.2 环境要求

- [ ] 真实桌面 WM（GNOME / KDE / XFCE / Sway 等任一真实合成器，**不是 WSLg 也不是 headless**）。
- [ ] `DISPLAY`（X11）或 `WAYLAND_DISPLAY`（Wayland）已设置且 `echo $XDG_SESSION_TYPE` 返回 `x11` / `wayland`。
- [ ] 桌宠二进制已构建（见 §0）。
- [ ] 终端与桌宠窗口在同一会话，便于聚焦窗口。

### 1.3 复测步骤

#### 步骤 1：以 `--window-smoke --pet-mode` 启动，确认非穿透初始态

```bash
# 拖动前必须先关掉穿透：穿透态下鼠标点不到窗口（受 click_through_active 守卫）。
# 真实桌面若托盘可用，pet-mode 会自动置顶 + 穿透；用 --smoke-timeout-secs 限制时长。
./target/release/live2d-ai-desktop --window-smoke --pet-mode --smoke-timeout-secs 60
```

**预期**：

- 窗口出现（无边框或仅极薄装饰，类型 `Bgra8Unorm` 透明）。
- 若托盘已就绪（`runtime.tray == Available`），窗口**默认点击穿透**（cursor 直接穿透到下层窗口）。
- 若托盘未就绪（无 D-Bus / 无 StatusNotifierWatcher），按 §3 降级路径，窗口**保持交互**且仍可被聚焦。

#### 步骤 2：关闭点击穿透（为拖动打开通路）

- **托盘已就绪**：左键点击托盘图标打开菜单，勾掉「点击穿透」一栏；菜单收起后窗口恢复交互。
- **托盘未就绪或无托盘**：按 `Alt+F4` / 窗口管理器关闭按钮无效时，使用 `wmctrl` 手动聚焦：

  ```bash
  wmctrl -a "Live2D-Ai" || xdotool search --name "Live2D-Ai" windowactivate
  ```

**判据**：

- 鼠标移到窗口上，cursor 不再穿透（被窗口截获）。
- 终端可观察到日志 `set_visible 已应用`（仅切显隐时）或 `set_cursor_hittest 关闭` 之类（若实际文案不同以 `app/capability.rs` 为准，本清单不锁定文案）；至少托盘状态簿记中 `click_through_active` 已变 `false`。

#### 步骤 3：在窗口上做左键按下 → 移动 → 松开

操作：把鼠标移入窗口范围 → **按住左键** → 拖到屏幕另一处 → **松开左键**。

**预期**：

1. 按下瞬间窗口进入「跟随光标」状态（紧贴光标移动，无明显滞后）。
2. 移动过程中窗口四角与光标保持恒定像素偏移（无回弹、无飘回原点）。
3. 松开后窗口**停留在新位置**，不回到原坐标，也不被合成器重置到 (0,0)。
4. 终端日志应出现 `drag_window 成功（交互态左键拖动可用）`（来自 `app/capability.rs:217`，首次成功一次性输出）。

**失败判据**：

- 窗口不跟随、立即弹回、回到原点、坐标在松开后被合成器改写 → 拖动失败。
- 终端未出现 `drag_window 成功` 字样 → 至少本次未记录能力。

#### 步骤 4：第二次拖动（验证幂等）

重复步骤 3，**判据**：

- 行为与步骤 3 一致。
- 终端**不再**重复打印 `drag_window 成功`（`drag_recorded` 守门，仅首次记录），不产生新日志噪声。

#### 步骤 5：再开回穿透，验证拖动路径在穿透下被屏蔽

```bash
# 1) 通过托盘菜单重新勾选「点击穿透」；
# 2) 尝试在窗口范围内左键拖动。
```

**预期**：鼠标点不到窗口（穿透态），日志中 `begin_interactive_drag` 早返回，不调 `drag_window`、不更新 `drag_move` 状态。

#### 步骤 6：能力表对账

按 §0 重新跑一次 Info 模式（或在 `RunReport` 中查找 `runtime.drag_move`）：

```bash
./target/release/live2d-ai-desktop | grep -E '^(drag_move|backend)\s'
```

**判据**：`drag_move` 字段值为 `Available`（步骤 3 成功后已写入 `runtime.drag_move`）；`backend` 与本机 `$XDG_SESSION_TYPE` 一致。

> 备注：Info 模式本身不调 `drag_window`，所以 `drag_move` 的能力记录依赖本次复测运行的内存态。本节要求步骤 3 与步骤 6 必须在同一次 `--window-smoke --pet-mode` 会话内（或先关进程再开新会话、由 RunReport 输出 capability 表）——若用 Info 模式单跑看到 `drag_move == Unavailable` 是预期，需要在 `--window-smoke --pet-mode` 的 RunReport 中确认。

### 1.4 异常分支与判据

| 现象 | 判定 | 下一步 |
|---|---|---|
| 拖动跟随正常、松开被合成器改回原点 | 全局定位回归问题（与 `desktop-platform-smoke` 第 15 行同型） | 单独记录，复测 C9 全局定位项；与 C10 拖动能力分账 |
| 拖动过程中窗口闪烁 / 残影 | 实时路径在 resize 抖动 | 回到 C4 表面契约复测 |
| Wayland 环境下拖动不工作 | `drag_window` 在 Wayland 上依赖具体合成器实现，平台性差异 | 与 C9「Wayland 置顶不支持」同口径，按环境差异记录，**不视为缺陷** |
| 终端 panic / segfault | 拖动路径存在回归 | 立即停止手动复测，保留 coredump 走缺陷流 |

---

## 2. C11 托盘恢复链路清单

### 2.1 背景与现状

- 托盘实现：`crates/live2d-ai-desktop/src/tray.rs`（ksni SNI 协议，纯 Rust；菜单条目规格在 `build_menu_model`，顺序固定为「点击穿透 / 窗口置顶 / 显示窗口 / 退出」）。
- 菜单事件落地：`crates/live2d-ai-desktop/src/app/interaction.rs:19-48`（`ShellApp::handle_tray_event`），用户事件源在 `crates/live2d-ai-desktop/src/user_event.rs:18-32`。
- pet-mode 安全规则：`user_event.rs:55-73`（`plan_launch`）—— 托盘未就绪时**强制禁自动穿透**，防「穿透 + 无恢复入口」不可恢复状态。
- 托盘初始化超时/失败/无 D-Bus：记 `degradation_note` 并可见降级，**不阻塞启动**（`tray.rs:327-412`）。
- 本节覆盖：托盘已就绪环境下的「隐藏 → 托盘唤回 → 穿透开关」闭环。

### 2.2 环境要求

- [ ] 真实桌面 WM + 完整桌面会话（X11/Wayland 任一）。
- [ ] D-Bus session bus 可用：`echo $DBUS_SESSION_BUS_ADDRESS` 非空；`dbus-send --session --dest=org.freedesktop.DBus --type=method_call / org.freedesktop.DBus.ListNames` 能列出名字。
- [ ] StatusNotifier 服务可达：GNOME 需安装 `AppIndicator` 扩展（`gnome-shell-extension-appindicator`），KDE/Plasma/XFCE 默认支持。
- [ ] 桌宠二进制已构建（见 §0）。

### 2.3 复测步骤

#### 步骤 1：托盘就绪探针

```bash
./target/release/live2d-ai-desktop --pet-mode --smoke-timeout-secs 120
```

**预期**：

- 启动后**≤1.5s**（`TRAY_CONFIRM_WAIT = 1500ms`）出现托盘图标，标题为「Live2D-Ai 桌宠」。
- 终端日志出现 `托盘就绪（StatusNotifierItem 已注册）`（来自 `tray.rs:390`）。
- Info 模式 / RunReport 中 `tray == Available` 且 `click_through == Available`、`click_through_active == true`（pet-mode 在托盘就绪时按 `plan_launch(true, true)` 走「置顶 + 穿透」全开）。

#### 步骤 2：核对托盘菜单条目

在系统托盘区右键点击「Live2D-Ai 桌宠」图标。

**预期**：菜单依次为

1. 「点击穿透」（已勾选 ✓，来源 `tray.rs:90-91` `CLICK_THROUGH`）
2. 「窗口置顶」（已勾选 ✓，来源 `tray.rs:92` `ALWAYS_ON_TOP`）
3. 「显示窗口」（已勾选 ✓，来源 `tray.rs:93` `VISIBLE`）
4. 「退出」（普通条目，来源 `tray.rs:94` `EXIT`）

四个条目顺序、文案、勾选态须与 `tray.rs::build_menu_model` 单测一致；条目数 = 4。

#### 步骤 3：托盘图标左键激活 = 显隐切换（关闭入口之一）

左键单击托盘图标（不是菜单）。

**预期**：

- 窗口被隐藏（从屏幕消失，但进程不退）。
- 终端日志 `set_visible 已应用 visible=false`（来源 `app/capability.rs:184-186`，文案以源码为准）。
- 托盘菜单中「显示窗口」勾选同步翻转（共享簿记 `TraySharedState.visible`）。

#### 步骤 4：托盘图标再左键单击 = 唤回（核心恢复链路）

在不关闭窗口、不退进程的前提下，再次左键单击托盘图标。

**预期**：

- 窗口**重新出现在屏幕**（位置与隐藏前一致；不重定位）。
- 终端日志 `set_visible 已应用 visible=true`。
- 「显示窗口」勾选回到 ✓。
- 整体效果：从「穿透态 + 隐藏态」回到「穿透态 + 可见态」，无需 Alt+Tab / wmctrl。

**判据**：这是 C11 主链路的关键正向判据；若唤回失败（窗口不出现 / 出现在错位置 / 进程被结束），记阻断。

#### 步骤 5：托盘菜单「显示窗口」勾选项 = 等价路径

走菜单而非左键激活：

- 步骤 3 已隐藏窗口 → 打开托盘菜单 → 勾掉「显示窗口」（即切到「隐藏」态后再次点选，使其回到「显示」）。
- 预期与步骤 4 一致：窗口回到屏幕。
- 该路径用 `PetUserEvent::TraySetVisible(true)`（左键激活用）vs `PetUserEvent::TrayToggleVisible`（菜单勾选用），二者经 `event_target` 归一到同一目标态 → 行为等价（见 `user_event.rs:107-117`）。

#### 步骤 6：托盘菜单「点击穿透」切换

窗口可见且聚焦后，打开托盘菜单 → 单击「点击穿透」。

**预期**：

- 勾选翻转（✓ → ☐ 或反向）。
- 终端日志 `set_cursor_hittest ...` 一类应用记录（文案以源码为准）。
- 鼠标交互性随之变化：勾掉时窗口截获输入，勾上时鼠标穿透到下层。
- `runtime.click_through` 在 RunReport 中保持 `Available`（能力本身不变，仅 `click_through_active` 翻转）。

#### 步骤 7：托盘菜单「窗口置顶」切换

类似步骤 6，单击「窗口置顶」：

**预期**：

- 勾选翻转。
- X11：实际置顶状态生效（窗口层变化；用 `xprop` 或 WM 工具自检）。
- Wayland：依据 `desktop-platform-smoke-2026-08-26.md` 第 14 行，置顶在 Wayland 上记 `Unavailable`（API 平台性差异），勾选翻转但**视觉层变化不一定可见**——属预期，不视为缺陷。

#### 步骤 8：托盘菜单「退出」= 收尾路径

打开托盘菜单 → 单击「退出」。

**预期**：

- 进程退出，退出码 `0`。
- 终端日志 `托盘菜单请求退出`（来源 `interaction.rs:42`）→ 后续按 `StopReason::TrayExit` 走正常 shutdown。

### 2.4 异常分支与判据

| 现象 | 判定 | 下一步 |
|---|---|---|
| 启动后 1.5s 内未出现托盘图标 | 可能 D-Bus/StatusNotifier 链路问题 | 查终端 `tray 不可用` 警告；按 §3 降级路径继续 |
| 托盘出现但菜单条目数 ≠ 4 或顺序错 | 菜单模型回归 | 查 `build_menu_model` 单测是否仍通过；运行 `cargo test -p live2d-ai-desktop tray::` |
| 左键激活唤回失败 | 显隐恢复链路回归 | 立即停止后续复测，保留 coredump 走缺陷流 |
| 菜单切换穿透后窗口「卡死」无法再交互 | 罕见（仅当托盘菜单本身被穿透层截断） | 用 `wmctrl -a "Live2D-Ai"` 强制聚焦；非托盘关闭后的恢复场景，按已知限制记录 |

---

## 3. 无 D-Bus / 托盘不可用降级路径（与 C11 配套的预期行为）

> 来自 `desktop-platform-smoke-2026-08-26.md` 第 18 行（WSLg 实测已记录）。本节把降级行为列成可复测清单，确保「降级不阻塞启动」是契约而非偶然。

### 3.1 触发条件

- 无 D-Bus session bus（如 `DBUS_SESSION_BUS_ADDRESS` 空、容器化环境）。
- 无 StatusNotifierWatcher（罕见桌面环境）。
- StatusNotifierHost 不可显示（GNOME 未装 AppIndicator 扩展 → `ksni::Error::WontShow`）。
- 托盘初始化超过 1.5s 未确认（D-Bus 总线挂起等异常）。

### 3.2 复测步骤

#### 步骤 D1：清空 D-Bus 模拟降级

```bash
# 临时去掉 D-Bus 地址跑一次
env -u DBUS_SESSION_BUS_ADDRESS \
  ./target/release/live2d-ai-desktop --pet-mode --smoke-timeout-secs 15
```

**预期**：

- 进程仍正常启动，**退出码 `0`**（注意：若加了 `--smoke-timeout-secs 15` 且到时未退，是兜底退出码 `0`；如以 `Ctrl+C` 收尾亦同）。
- 终端日志 `tray 不可用` + 降级原因（其中之一，依赖实际触发）：
  - `无可用 D-Bus session bus（无桌面会话或沙箱未配置总线）`
  - `StatusNotifierWatcher 不可达（当前桌面环境不支持 SNI 托盘或扩展未启用）`
  - `托盘初始化超过 1.5s 未确认（D-Bus 总线可能挂起）——按不可用降级`
  - `托盘已注册但无 StatusNotifierHost 显示它（如 GNOME 未启用 AppIndicator 扩展）——图标不会出现`
- `pet-mode：托盘未确认可用，本次运行已禁用自动点击穿透 ...`（来源 `user_event.rs:84-88`）也需可见。

#### 步骤 D2：窗口交互性保持

降级启动后，用鼠标在窗口上点击。

**预期**：可正常点击、拖动（能力本身仍 available，缺的是「托盘恢复入口」）；点击不会穿透到下层。

#### 步骤 D3：Info 表对账

```bash
env -u DBUS_SESSION_BUS_ADDRESS \
  ./target/release/live2d-ai-desktop --pet-mode --smoke-timeout-secs 5
# 进程退出后（5s 兜底），再跑一次 Info 模式查 baseline：
./target/release/live2d-ai-desktop | grep -E '^(tray|click_through|drag_move)\s'
```

**判据**：

- `tray == Unavailable`（如实记录）。
- `click_through == Available`（能力本身在，只是 pet-mode 安全规则不让它自动启用）。
- `drag_move == Unavailable`（Info 模式未触发拖动；同 §0 备注）。
- **不出现 `tray == Available` 或漏报**。

### 3.3 关键约束（与 C11 关闭条件一致）

- 降级路径**不阻塞启动** —— `--pet-mode` 在任何 D-Bus 状态都能跑完窗口初始化。
- pet-mode 降级**禁自动穿透** —— 防止「穿透 + 无恢复入口」不可恢复状态。
- 托盘迟到就绪（>1.5s 后才注册）句柄仍被接管：`TrayRuntime::handles` 持续持有，后续 `refresh` / `shutdown` 仍受控（见 `tray.rs:351-355`）——此行为属源码保证，不需手动复测。

---

## 4. 复测结果记录模板

每次复测建议在 `docs/verification/` 下新建一份 `<日期>-c10-c11-manual.md`，结构建议：

```markdown
# C10 / C11 手动复测记录（<日期>，<环境>）

## 环境
- 发行版 / WM / 会话类型（x11 | wayland）/ D-Bus 状态
- binary：`target/release/live2d-ai-desktop` @ <commit>

## C10 拖动
- [ ] 步骤 1-6 全部通过 / 失败描述
- [ ] drag_move 终值：Available / Unavailable
- [ ] 异常：<如有，按 §1.4 表填>

## C11 托盘恢复链路
- [ ] 步骤 1-8 全部通过 / 失败描述
- [ ] tray 终值：Available / Unavailable
- [ ] 唤回链路：成功 / 失败
- [ ] 异常：<如有>

## 降级路径（仅当 D-Bus 不可用时复测）
- [ ] 降级文案出现
- [ ] 进程启动不阻塞
- [ ] 交互性保持
```

---

## 5. 待高级 AI 确认事项

1. 是否将 §1.3 步骤 6 的「Info 模式 + grep」替换为对 RunReport 的解析（待 `backend.rs` 接口稳定）。
2. §2.3 步骤 4 唤回链路的「位置与隐藏前一致」是否需要坐标级回归（建议依赖 §1.3 步骤 6 同一证据）。
3. §3 降级路径是否要纳入正式 RC 验收线（C9 能力承诺表终稿待定）。
4. 本清单的更新频率：随 `cli.rs` / `tray.rs` / `user_event.rs` / `platform.rs` 任一文件变更须同步刷新。
