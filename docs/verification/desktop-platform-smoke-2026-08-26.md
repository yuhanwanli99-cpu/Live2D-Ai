# 桌面平台能力实测记录（WSLg，2026-08-26）

> 目的：回收「X11/Wayland/托盘能力」最后一轮验证证据（RFC 批次 6 遗留项）。
> 环境：WSLg（DISPLAY=:0 / WAYLAND_DISPLAY=wayland-0），GPU=llvmpipe(Vulkan,
> Mesa 26.0.3)，无 D-Bus session bus、无声卡。所有结论以
> `RuntimeCapabilities` 实测口径为准（静态代码路径 ≠ 运行时承诺）。
> 复现命令见 `crates/live2d-ai-desktop/README.md`「冒烟验证记录」。

## 结果矩阵

| 能力 | X11(XWayland) | Wayland | 说明 |
| --- | --- | --- | --- |
| 透明窗口 + surface | ✅ available(PreMultiplied) | ✅ available(PreMultiplied) | 两会话 Bgra8Unorm，presented 全量无跳帧 |
| 置顶 set_window_level | ✅ available（生效） | ❌ unavailable（如实记录） | API 平台性差异，非缺陷 |
| 底部右侧定位 | ⚠️ 请求正确、合成器拒绝 | 同左（预期不采纳） | 回读 (-32774,-32795) ≠ 目标 (1988,687)，记 unavailable——**WSLg 合成器行为，待真实 WM 复测（节点 C）** |
| 点击穿透 set_cursor_hittest | ✅ available | ✅ available | |
| 拖动 drag_window | 代码路径在；本沙箱无法注入真实鼠标拖拽 | 同左 | **待人工交互复验（节点 C 清单项）** |
| 托盘 ksni | 无 D-Bus → 降级可见不阻塞启动 | 同左 | 降级文案用户可见（中文原因） |
| pet-mode 安全规则 | 无托盘 ⇒ 自动穿透禁用、保持交互 ✅ | — | `plan_launch` 实测路径生效 |

## 模型渲染冒烟（X11，120 帧）

- Bai 加载：moc V4_2_0；475 artmesh / 549 deformer / 159 parts / 128 参数；
- 六动作固定顺序：nod → shake_no → tilt → look_around → listen…（Medium）；
- 合成口型通道：writes=120 peak=0.871；
- **性能基线（节点 C 输入）**：llvmpipe 软渲染 frame_time avg≈116ms max≈144ms
  （当前实时路径含阻塞式 GPU wait；非阻塞 poll 改造前后须用同法对比）。

## 音频冒烟

- 本沙箱无声卡（ALSA cannot find card '0'）→ 退出码 3，环境分类语义正确；
  真声卡链路（RMS 对实际输出采样）此前批次已实测通过。
