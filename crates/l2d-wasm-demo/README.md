# l2d-wasm-demo — 可选 WASM 渲染演示（RFC D10）

浏览器端渲染 `crates/l2d` 封装的 Live2D 皮套：**WebGPU 优先、WebGL2 回退**，
固定 dt 渲染循环，错误全部显示在页面底部 `<pre id="status">` 状态栏。
本 crate 是**演示/调试用途的可选件**，应用本体仍为原生渲染（RFC D10）。

> **交付口径（2026-08-26）**：开发环境无浏览器/GPU 实跑条件，本次只完成
> **编译门禁**——
> `cargo check -p l2d-wasm-demo --target wasm32-unknown-unknown` 真实通过；
> native target 为 stub（打印运行指引即退出），workspace 原生构建不受影响。
> 浏览器实跑请按下方步骤自备工具链与模型资产。

## 运行流程（wasm 实现）

1. 从 query `?model=<model3 URL>` 取清单地址，缺省 `/models/bai/runtime/bai.model3.json`；
2. fetch model3 清单 → 按 `FileReferences` 引用逐个 fetch moc3 / 纹理 / physics3.json / cdi3.json；
3. `ModelPackage::from_memory_map` 内存组包 → `LoadedModel::resolve` 统一加载路径（含版本交叉核对）；
4. `<canvas>` surface 协商设备（WebGPU → WebGL2 回退）→ `GpuContext::from_parts_with_label`
   接入已有 device/queue → `ModelRendererCore::load_model`；
5. requestAnimationFrame 循环：每帧固定 dt（60 Hz）`update` + `render_to_view` 直出交换链视图；
6. resize（窗口缩放/DPR 变化）→ 重配 surface + 同步视口；任何失败写入状态栏并停止。

## 编译门禁

```bash
rustup target add wasm32-unknown-unknown   # 如未安装
cargo check -p l2d-wasm-demo --target wasm32-unknown-unknown
```

native target 无需 wasm 工具链（依赖全部 target-gated），`cargo test --workspace`
与 `cargo clippy --workspace` 在原生平台直接可用；demo 的 native 二进制是 stub。

## 浏览器实跑（需自备）

前置：安装 [trunk](https://trunkrs.dev)（`cargo install trunk --locked`），
并已通过上面的编译门禁。

```bash
cd crates/l2d-wasm-demo
trunk serve --release          # 默认 http://127.0.0.1:8080
```

### 模型静态目录挂载要求

页面从**同源** `/models/<...>` 路径取模型资产，而 `trunk serve` 只输出构建产物，
**不会自动提供模型文件**。Bai 资产不进仓库、不进公共包（分发口径见
`docs/legal/pc-preview-publication.md`），需要你在本地把合法取得的模型目录
挂到可服务的路径上，二选一：

- **代理方式（推荐，配合 `trunk serve`）**：把模型目录用任意静态服务暴露，
  并启用 `Trunk.toml` 中现成的 `[[proxy]]` 示例：

  ```bash
  # 例：模型位于 <repo>/assets/models/bai/runtime/…
  python3 -m http.server 8123 \
    --directory <repo>/assets/models
  # 然后取消 Trunk.toml 里 [[proxy]] 的注释（rewrite=/models/ → backend 上面的服务）
  ```

- **产物内挂载（配合静态托管）**：`trunk build --release` 后，把模型目录复制/
  符号链接到 `dist/models/...`，再用任意静态服务器托管 `dist/`：

  ```bash
  trunk build --release
  ln -s <你的 live2d-models 目录> dist/models
  python3 -m http.server 9000 --directory dist
  # 打开 http://127.0.0.1:9000/?model=/models/bai/runtime/bai.model3.json
  ```

`?model=` 可指向任意同源或允许 CORS 的 model3.json URL，用于预览非缺省皮套；
资源按 manifest 相对引用解析到该 URL 所在目录。

## 边界说明

- 本 crate 不做 headless 初始化、不用 pollster 阻塞、不做帧读回（读回语义属于
  `l2d` 的离屏管线）；渲染核心复用 `ModelRendererCore::render_to_view`；
- 兼容报告口径与 `crates/l2d` 一致：非 v0 锚定档位照常预览，但状态栏明确标注
  「v0 支持=否」及问题条数，不做静默放行；
- Bai 资产**未复制**进本 crate 或任何公共包；`dist/` 已被仓库根 `.gitignore` 覆盖。

## 许可

AGPL-3.0-only（应用侧 crate，RFC D11）：以仓库根 [LICENSE](../../LICENSE) 为准。
