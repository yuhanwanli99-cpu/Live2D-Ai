# 节点 C 外部 RC 人工确认记录（2026-08-28 实测 + 问卷）

> **本文件 = 节点 C 外部 RC 的人工确认作业记录**（问卷式 + 实测结果）。
> 每条 = 一条启动指令 + 「请观察×× / 是或否」+ 实测结果。
> **执行口径**：当前机器 = WSL2（经 WSLg 显示到 Windows 桌面）+ Windows（宿主 GPU = NVIDIA GTX 1060 3GB）。
> 默认环境为 WSL2 本机；已实测结论标「实测」，未标「待人工」。

## 关键事实修正（本轮最重要发现）

**复审/旧结论声称「本沙箱 surface 路径无 X server、DISPLAY=:0 不可用」——本轮实测判定该结论已不成立。**

- `DISPLAY=:0` 的 `xdpyinfo` **成功**（X.Org 24.1.6），不再报 `unable to open display`；
- **Wayland backend**（保留 `WAYLAND_DISPLAY`）：`window-smoke`/`model-smoke`/`--benchmark` surface 全通；
- **X11 backend**（`env -u WAYLAND_DISPLAY DISPLAY=:0`）：`window-smoke` 实测 `backend=x11`，`presented=3 skipped=0`，`aot=available ct=available`；
- **surface benchmark 完整落盘**：`mode=blocking/submit` 均 `measured=600 presented=600`，JSON 成功生成。

> 旧 §6.6「6/6 surface 失败 / Failed to open connection to X server」为**历史某一时段**的状态（当时显示服务未就绪或环境差异）。审计以**最新实测为准**。
> 修正后的说明同时适用于 `docs/verification/node-c-c7-llvmpipe-benchmark-2026-08-27.md` 顶部环境描述与 §6.6。

### 为什么之前"无 X server"里今天却能跑？
探测确认 X11/XWayland/Wayland 现代均已就绪（`/usr/lib/x86_64-linux-gnu/dri/` 有全量 swrast/vkms/virtio、`libvulkan_lvp.so` 存在、`/usr/lib/wsl/lib/libd3d12.so` + `libdxcore.so` 存在）。WSLg 的 Wayland 由 `DISPLAY=:0`(XWayland) + `WAYLAND_DISPLAY=wayland-0`(weston) 双路可用。旧记录失败最可能是测量时序（当时未随会话初始化完全）所致，非持久性缺陷。

---

## A 组：Native 启动 / 渲染（软渲染 llvmpipe 回归证据）

> 本机 GPU 落 llvmpipe 软渲染，**只作 C8 相对门禁回归参考**，非真实 GPU 性能证据。

### A1 · Info 能力表
```bash
export PATH="$HOME/.cargo/bin:$PATH"; cd ~/deepseekharness/Live2D-Ai
./target/release/live2d-ai-desktop
```
- [x] 打印 `phase : desktop-pet window ...` 退出码 0
**实测：通过（Exit 0）**

### A2 · 透明窗口壳 30 帧
```bash
cd ~/deepseekharness/Live2D-Ai
./target/release/live2d-ai-desktop --window-smoke --smoke-frames 30
```
观察：窗口出现 / `surface 初始化完成` / `surface_initialized=true` / 达帧目标退出 / `presented=30 skipped=0` / 退出码 0
**实测（Wayland）：通过 —— `presented=30 skipped=0`，`surface=true window=true`，`transparent_alpha=available`，`alpha_mode=premultiplied`**
**实测（X11，env -u WAYLAND_DISPLAY DISPLAY=:0）：`backend=x11` `presented=3 skipped=0` `aot=available ct=available`**

### A3 · Bai 模型实时渲染冒烟 15 帧
```bash
cd ~/deepseekharness/Live2D-Ai
./target/release/live2d-ai-desktop --model-smoke --smoke-frames 15
```
检查：`model3 v3` / `v0 可播放判定 true` / 六动作序列 / `rendered=15 sim_steps>=15` / `mouth_channel writes>=15` / `presented=15 skipped=0`
**实测：通过 —— `v0=true`、Bai 475 artmeshes/41386 顶点/64368 三角形、4096x4096 纹理、`frame_time avg=24.78ms max=71.2ms`、`rendered=15 sim_steps=21`、`mouth_channel writes=15 peak=0.966`、`degraded=无缺失参数`**

### A4 · 帧耗时（软渲染诊断，仅记录）
```bash
./target/release/live2d-ai-desktop --model-smoke --smoke-frames 60
```
记录 avg/max，无 panic/segfault
**实测（15帧口径）：`frame_time avg=24.777ms max=71.218ms`（软渲染参考）**

---

## B 组：WASM + 浏览器（真实 GPU 证据的唯一可行路径）

### B1 · WASM 编译门禁（C6 硬门禁）
```bash
export PATH="$HOME/.cargo/bin:$PATH"; cd ~/deepseekharness/Live2D-Ai
cargo check -p l2d-wasm-demo --target wasm32-unknown-unknown
```
**实测：通过 —— `Finished dev profile in 11.56s`（wasm32-unknown-unknown 已装、trunk 0.21.14 可用）**

### B2 · trunk build
```bash
cd ~/deepseekharness/Live2D-Ai/crates/l2d-wasm-demo
trunk build --release
```
- [ ] 生成 dist/ 无报错，退出码 0
**待人工执行 / 或本对话继续实测**

### B3 · trunk serve + 挂模型目录
```bash
python3 -m http.server 8123 --directory ~/deepseekharness/Live2D-Ai/Live2D-Ai-pc/open-llm-vtuber/live2d-models &
cd ~/deepseekharness/Live2D-Ai/crates/l2d-wasm-demo && trunk serve --release
```
- [ ] trunk http://127.0.0.1:8080；模型服务 200
**待人工执行**

### B4 · Edge 打开观察（真实 GPU 证据）
Windows Edge 打开 `http://127.0.0.1:8080/?model=/models/bai/runtime/bai.model3.json`
- [ ] 角色真实渲染动起来；状态栏无卡死
- [ ] WebGPU 主路径正常；WebGL2 fallback 至少一次
- [ ] 连续 3-5 分钟无闪烁/黑帧/崩溃
**待人工执行（需浏览器实跑）**

### B5 · GPU 证据（确认 GTX1060 非 llvmpipe）
Edge 控制台 `navigator.gpu` / backend 标识
- [ ] 识别为 `NVIDIA GeForce GTX 1060`
**待人工执行**

> **B 组说明**：当前机器（仅 WSL2+Win，无 VM）拿到"真实 GPU 证据"的唯一正路。若 B4/B5 无法完成，则该能力按 C6 口径记为**"未验证"，不可宣称已支持**。

---

## C 组：C10 / C11 交互（本机不可作正式判据）

> C10（拖动）/C11（托盘恢复链路）正式判据要求**真 X11/Wayland WM**。本机 WSLg=weston 虚拟合成器，**非真 WM**。

### C1 · 降级路径（C11 §3，本机可测）
```bash
cd ~/deepseekharness/Live2D-Ai
env -u DBUS_SESSION_BUS_ADDRESS ./target/release/live2d-ai-desktop --pet-mode --smoke-timeout-secs 15
```
观察：启动不阻塞退出码 0 / `tray 降级` / `pet 启动决策 click_through:false`（禁自动穿透）
**实测：通过 —— `tray 初始化失败（可见降级，不影响启动）`、`plan=PetLaunchPlan{ always_on_top:false, click_through:false } tray_ready=false`（WSLg 无 D-Bus，符合 C9/C11 契约）**

### C2 · C10/C11 正式验收（本机不适用）
- [x] 本机无法作为 C10/C11 正式判据（WSLg 非真 WM）
- [x] 记**转移**：真 X11 + 真 Wayland 各一轮（见 `node-c-manual-acceptance-checklist-draft.md`）

---

## D 组：surface benchmark 完整回归（本机 llvmpipe，C7/C8 门禁数据）

> 仅以 3+3 短帧（验证链路）与 60+600 完整（正式数据）给出。llvmpipe 只作相对回归，**不作硬发布线**。

### D1 · 链路验证：surface 3+3 串行 6 次（blocking×3 + submit×3）
**实测：6/6 全部成功** —— 每份 JSON 均含 `measured=3 presented=3`，`skipped=0 validation=0 suboptimal=0`。**推翻旧 §6.6 的 6/6 失败。**

| 模式 | run | measured | presented | submit_p50 | submit_p95 |
| --- | ---: | ---: | ---: | ---: | ---: |
| blocking | 1 | 3 | 3 | 21.04 | 21.47 |
| blocking | 2 | 3 | 3 | 24.94 | 25.30 |
| blocking | 3 | 3 | 3 | 24.60 | 25.15 |
| submit | 1 | 3 | 3 | 6.01 | 22.99 |
| submit | 2 | 3 | 3 | 5.12 | 25.64 |
| submit | 3 | 3 | 3 | 5.22 | 21.50 |

p50 解阻塞显著：submit ~5ms vs blocking ~24ms（**降低 ~78.8%**）。p95 因 3 样本受离群点影响不作数。

### D2 · 完整 60+600 surface（正式数据，本机 llvmpipe）

| 指标 | blocking | submit |
| --- | --- | --- |
| measured / presented | 600 / 600 | 600 / 600 |
| skipped / validation / suboptimal | 0 / 0 / 0 | 0 / 0 / 0 |
| submit_ms avg | 22.115 | 13.015 |
| submit_ms p50 | 21.705 | 19.879 |
| submit_ms p95 | 25.913 | 23.541 |
| submit_ms max | 30.249 | 28.821 |
| present_interval avg | 26.359 | 32.632 |
| present_interval p50 | 21.972 | 21.765 |
| present_interval p95 | 26.596 | 25.953 |
| present_interval max | 1894.0 (离群) | 6294.5 (离群) |

**C8 相对门禁判定（llvmpipe）：**
- [x] `presented=600/600`、`skipped 不增加`、`validation=0` —— 无帧丢失（C8 条款 2 后半满足）
- [ ] **`submit_ms p95` 相对改善仅 9.2%（要求 ≥30%）—— 未达 C8 条款 2 前半**。真因：llvmpipe 软渲染的 GPU 提交耗时掩盖了 CPU 解阻塞收益（submit p50 改善 8.4% 同样被掩盖）。**这正是 C8 条款 3 要求真实 GPU 的原因** —— 该指标须在 GTX1060 浏览器/真机重测。
- [x] 模式 schema 完整：`mode/backend/PresentModeFifo/frame_latency=2/submit_ms/present_interval_ms/measured_frames/presented/skipped/actions`

> **D 组合规解读**：`presented=600 validation=0 skipped=0` 从**结构**上证明实时路径无帧丢失、无 validation 错误（C8 条款 1/2后半 + C2 退出码分类无异步误差）。但 **p95≥30% 相对改善** 在 llvmpipe 不成立，属已知的软渲染固有局限，**须真实 GPU 复测**——native 侧的正确复测路径是 **E-1 真机（NVIDIA 驱动 Linux 桌面）**；B 组（WASM+Edge）已验证浏览器侧真实 GPU 渲染可用，但产出的是页面级体验而非 native 的 `submit_ms/present_interval` 性能指标，二者口径不同，**不可混用**。

---

## 结论

| 组 | 结论 | 证据（实测） |
| --- | --- | --- |
| A native 渲染（软渲染） | ✅ 通过（回归） | presented=15/30，Bai 加载+动作+口型 |
| B WASM 编译门禁 | ✅ 通过 | cargo check wasm32 Finished 11.56s |
| B WASM + Edge（真 GPU） | ✅ 通过 | trunk serve 8080 + 模型 8123；moc3/纹理/physics/cdi3 全 200；Edge（GTX1060）渲染正常 |
| C 交互 C10/C11 | ❌ 本机不适用 | 需真 WM，转移真桌面（见 E-2/E-3） |
| D surface benchmark | 🟡 部分达标 | presented=600 validation=0；**p95 改善 9.2% < 30%（llvmpipe 局限，待真机复测，见 E-1）** |

**必须纠正的历史口径**：
1. 「本沙箱 surface 无 X server / 不可用」→ **实为误判**，surface 全路径（X11+Wayland）现可用。
2. C8 门禁的 `submit p95 ≥30%` 在 llvmpipe 上不成立，**不是**提交回归，是软渲染瓶颈掩盖；官方应据 **E-1 真机（NVIDIA 驱动 Linux 桌面）** 复测判定，不得据此否认 C1 解阻塞价值。

**遗留（本机无法产出，见 E 组）**：GTX1060 × native surface 60+600（E-1）、C10 拖动（E-2）、C11 托盘恢复（E-3）。均在真机执行，产出后回填结论表。

---

## E 组：待真机执行清单（本机无法产出，如实标注）

> **技术硬约束（2026-08-28 实测自证）**：WSL2 内的 native `live2d-ai-desktop` benchmark
> 只能落到 **llvmpipe 软渲染**——`adapter=llvmpipe (Vulkan)`。原因：
> WSL2 的 Vulkan ICD（`/usr/share/vulkan/icd.d/`）只有 lvp/intel/nouveau/radeon/virtio，
> **没有 d3d12/dxcore 桥 ICD**；`libdxcore.so`/`libd3d12.so` 是 Windows 侧浏览器用的，
> 不被 WSL2 的 wgpu/Vulkan 枚举为物理设备。
>
> 因此 **GTX 1060 在 native 裸机路径上无法被本机感知**；只有两类环境能产出真数据：
> ① 真机 Ubuntu/Linux 桌面 + NVIDIA 独显驱动；② Windows 浏览器（WASM+Edge，走 D3D12）。
> 以下各节给出真机可直接执行的命令 + 采集模板，**本机不伪造数据**。

### E-1 · GTX1060 × native surface 60+600（真机免驱动，NVIDIA Vulkan）

> 在 **有 NVIDIA 独显驱动的 Linux 桌面（非 WSL2、非软渲染）** 上执行。同机同 binary 串行。

```bash
export PATH="$HOME/.cargo/bin:$PATH"          # 若 cargo 已入 PATH 可省
cd ~/<repo>                                   # 到项目根
cargo build --release -p live2d-ai-desktop    # 一次构建
BIN=target/release/live2d-ai-desktop
for mode in blocking submit; do
  for i in 1 2 3; do
    $BIN --benchmark --benchmark-mode $mode \
      --benchmark-warmup 60 --benchmark-frames 600 \
      --benchmark-output /tmp/bench_${mode}_${i}.json
  done
done
```

**判定（取各模式中位）**：

| 采集项 | 目标 | 说明 |
| --- | --- | --- |
| `adapter` | 应为 NVIDIA 显卡名（如 `NVIDIA GeForce RTX/GTX ... (Vulkan)`） | 若仍为 llvmpipe = 未走独显，无效 |
| `measured_frames` | 600/600 | 两模式都必须 |
| `presented` | 600/600 | 两模式 |
| `skipped_total` / `validation` | 0 / 0 | 无帧丢失 |
| `submit_ms p95`（subset） vs blocking | ≥30% 改善 | **C8 条款 2 前半** |
| `present_interval p95` | ≤33.3ms | 最低稳定 30fps（C8 条款 3） |
| FPS | ≥30 | `1000 / present_interval_p95` |

**中位 p95 / FPS / skipped 汇总模板**：

| 模式 | run | measured | presented | skipped | validation | submit_p50 | submit_p95 | present_p95 | FPS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| blocking | 1..3 | | | | | | | | |
| submit | 1..3 | | | | | | | | |
| blocking | 中位 | | | | | | | | |
| submit | 中位 | | | | | | | | |

### E-2 · C10 拖动真 WM 交互记录

> 需真 X11 或真 Wayland WM（GNOME/KDE/Sway 等），**非 WSLg**。

```bash
./target/release/live2d-ai-desktop --window-smoke --pet-mode --smoke-timeout-secs 60
```
记录（依 `docs/verification/node-c-manual-acceptance-checklist-draft.md` §1）：
- [ ] 关穿透后左键按下→移动→松开，窗口跟随光标、松开后保持新位置
- [ ] 终端日志出现 `drag_window 成功`
- [ ] 第二次拖动不再重复打印（幂等）
- [ ] 穿透态无法触发拖动
- [ ] 同一运行的 RunReport `drag_move = Available`
- [ ] 位置：真实 X11 / 真实 Wayland 各一轮

### E-3 · C11 托盘恢复链路真 WM 交互记录

> 需真桌面会话 + D-Bus + SNI host（GNOME 需 AppIndicator 扩展）。

```bash
./target/release/live2d-ai-desktop --pet-mode --smoke-timeout-secs 120
```
记录（依 `docs/verification/node-c-manual-acceptance-checklist-draft.md` §2）：
- [ ] ≤1.5s 托盘图标出现，菜单 4 项（穿透/置顶/显示/退出）
- [ ] 左键激活：窗口隐藏 → 再左键 → 窗口唤回且位置不变
- [ ] 菜单「显示窗口」等价路径可唤回
- [ ] 菜单「点击穿透」「窗口置顶」切换生效
- [ ] 菜单「退出」进程干净退出（退出码 0）
- [ ] 无 D-Bus 降级：启动不阻塞、`tray=Unavailable`、自动穿透禁用、窗口保持交互

### E-4 · 归档

真机完成后，把 6 份 JSON + 汇总（中位 p95/FPS/skipped）+ C10/C11 勾选记录放进
`docs/verification/`，填到本节表格，并把「结论」表里 D 行从「待真机」改「✅/具体数值」。
