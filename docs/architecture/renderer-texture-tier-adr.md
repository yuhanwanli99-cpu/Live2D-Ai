# ADR：渲染纹理 / 离屏靶标档位（4096 / 8192 / 16384）

> 状态：**接口已落地、真机验收仍属外部**，2026-09
> 分支：`dev/integrity`
> 关联：`docs/plans/integrity-takeover-fix-plan-2026-09-07.md` §4（渲染档位可行性）
> 本 ADR 定义「纹理档位」如何落在离屏渲染 target 尺寸与设备 limits 上。**当前环境（llvmpipe
> 软渲染、无独立 GPU）不做真机验收**；代码/接口是可门禁交付的，16384 档的真机可用性见 §6。
> 面向平台定位：不引入复杂上层——配乐最小、走既有 `Renderer`/`OffscreenRenderer`/wasm `stage-config`
> 三处既有触点，不新增机制。

---

## 1. 背景与事实

- ripgrep 整个 `crates/`：**无**任何 render-texture tier 常量/ flag；`tier` 仅指情绪档位，无关渲染尺寸。
- 唯一定 limits 的调用点是 native 路径 `crates/l2d/src/renderer/gpu.rs:107`：
  ```rust
  required_limits: wgpu::Limits::default(),
  ```
  `wgpu::Limits::default().max_texture_dimension_2d = 8192`。
- `crates/l2d-wasm-demo/src/web/surface.rs:163-173`：wasm 尺寸派生

  ```rust
  /// v1 不暴露 UI 档位，固定 1.5 cap；如后需切换档，在改 `DPR_CAP` 即可。
  const DPR_CAP: f64 = 1.5;
  // canvas_pixel_size(): dpr = window.device_pixel_ratio().clamp(1.0, DPR_CAP)
  ```
  wasm device descriptor 是 `..Default::default()`（surface.rs:80-83）。
- `OffscreenRenderer`（`crates/l2d/src/renderer/offscreen.rs:19-25`）：`width/height` 由调用方传入，
  目标纹理 `UVec2::new(width, height)` + `Rgba8Unorm` + 1 mip（offscreen.rs:120-128）。**没有尺寸上限**，
  仅受 device limits 约束。
- 前端「外观与舞台」设置**纯浏览器 localStorage + iframe postMessage**，不走后端持久化
  （`app.js:399-464`、`postStageConfig` app.js:376-387 发 `scale/dark/lipSync/idleEnabled/clickEnabled`）。

## 2. 目标

1. 把「纹理档位」定义为**离屏渲染 target 尺寸的离散档**（超采样/降采样目标），而非纹理资产名
   （`bai.16384/` 只是文件夹名，实际纹理是 `texture_00_4096.png` 4096×4096）。
2. native 路径可变 `max_texture_dimension_2d`（≥16384）以支持高档；wasm 保持浏览器上限。
3. 档位经前端「外观与舞台」应用 → `stage-config` 新增字段 → wasm surface 设定离屏 target 尺寸。

## 3. 冻结的接口设计

### 3.1 档位常量

```rust
// crates/l2d/src/renderer/tier.rs（新增，仅常量）
/// 离屏渲染 target 的离散档位（RGBA8 单张 ≈ 4096:64MB / 8192:256MB / 16384:1GB）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderTier {
    Tier4096 = 4096,
    Tier8192 = 8192,
    Tier16384 = 16384,
}
impl RenderTier {
    pub const DEFAULT: Self = Self::Tier4096;
    pub fn side(&self) -> u32 { *self as u32 }
    pub fn required_max_texture_dimension(&self) -> u32 { self.side() }
}
```

### 3.2 native limits（gpu.rs）

把 `required_limits` 从 `wgpu::Limits::default()` 改为按档位构造：

```rust
let mut limits = wgpu::Limits::default();
limits.max_texture_dimension_2d = tier.required_max_texture_dimension(); // 默认 4096
```

- `limit_down()` 时不会失败：档位只有 <=8192 时 default 已满足；16384 需自定义 16384。

### 3.3 wasm surface（surface.rs / main.rs）

- `BridgeState` 新增字段 `tier: RenderTier`（默认 `Tier4096`）。
- `main.rs "stage-config"` arm 新增解析字段 `tier`（JSON number 4096/8192/16384，非法忽略）。
- `canvas_pixel_size` 保留 DPR_CAP 语义，但其结果**作为目标侧边的下限**：把 `tier.size()` 设为
  buffer 尺寸上限，实际离屏 target = `min(tier.size(), canvas_pixel_size)`，避免放大 do_private
  超 sampler 上限而失败。

### 3.4 前端（web_api/app.js）

- 「外观与舞台」新增下拉「渲染档位」(4096/8192/16384)，默认 4096；存 `dsh.app.tier`（localStorage）。
- `postStageConfig()` 追加 `cfg.tier`。
- 与既有 `dsh.app.scale/dark/lipSync` 一致：纯本地，不进后端。

### 3.5 接口契约图谱

| 层 | 触点 | 改动 |
|---|---|---|
| native 离屏 | `crates/l2d/src/renderer/gpu.rs:107` | limits 按档 |
| wasm surface | `surface.rs:163-173, canvas_pixel_size` | tier 钳制 |
| wasm bridge | `surface.rs` BridgeState | 新字段 `tier` |
| wasm msg dispatcher | `main.rs:245 "stage-config"` | 解析 `tier` |
| 前端 | `web_api/app.js:376-387` | 下拉 + `dsh.app.tier` |

---

## 5. 真机验收（S3，外部环境）

- native 路径需在**带独立 GPU** 的环境实测 `16384` target 能否建立、并给出 5060 8GB 等典型卡的
  安全档位结论。
- wasm `/render` 受浏览器 GPU limits 约束，16384 档一般不现实，上限取决于浏览器。
- **验收（外部）**：档位切换后离屏 target 尺寸肉眼可见（缩放/描边清晰度）变化；无效功能/显存不足
  回退上一档 + toast，不 crash。`cargo test --workspace --all-targets` 全绿（含 tier 解析/钳制单测）。

## 6. 范围界定
- **本期（已落地）**：tier 常量/解析/钳制代码 + 单测 + 接口冻结文档均已合入；`RenderTier` 枚举、native limits 按档、wasm surface 钳制、前端档位下拉全部实现。
- **外部验收期**：S3 真机 16384 target 验证 + wasm 浏览器档位实测（仍属外部，不在门禁范围）。

## 变更历史
- 2026-09：设计冻结（文档先行，代码原生/浏览器档位按此 ADR 分属 native/surface。