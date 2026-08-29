# 节点 C 裁决问题 C5：Ayagami `prepare` 逐帧上传行为证据

> **任务范围**：仅提供证据，不做裁决。
> **目标问题**：每帧 `renderer.prepare(&mut encoder, &options)` 是否存在逐帧
> 重复上传（mask / 纹理 / uniform buffer）？若上游不可控，l2d 层是否缓存未变更
> 的 bind group？
> **取证时间**：2026-08-27
> **关键源码**：
> - `crates/l2d/src/renderer/{mod.rs, model_core.rs, offscreen.rs, gpu.rs}`（l2d 封装）
> - `~/.cargo/git/checkouts/ayagami-c3f4333abbf9e569/640ae4b/ayagami-render/src/renderer.rs`（上游 `prepare` 实现，1656 行）
> - `~/.cargo/git/checkouts/ayagami-c3f4333abbf9e569/640ae4b/ayagami-render/src/texture.rs`
> - `~/.cargo/git/checkouts/ayagami-c3f4333abbf9e569/640ae4b/ayagami/src/driver/mod.rs`（脏标志驱动）

---

## ① 每帧 `prepare` 调用链（file:line + 关键原文）

### 1.1 l2d 层 → 每帧无条件调 `prepare`

`crates/l2d/src/renderer/model_core.rs:139-181`（`ModelRendererCore::render_to_view`），
关键段：

```rust
144:         let options = RenderOptions {
145:             transform: aspect_fit_transform(self.viewport.0, self.viewport.1) * self.transform,
146:             mask_dimensions: UVec2::new(self.viewport.0, self.viewport.1),
147:             colorspace: self.colorspace,
148:         };
...
155:         self.renderer.prepare(&mut encoder, &options);   // ← 每帧都调，无条件
...
173:             self.renderer.render(&mut pass, format);
...
175:         self.gpu.queue().submit(Some(encoder.finish()));
176:         self.gpu.device().poll(wgpu::PollType::wait_indefinitely())
```

`RenderOptions` 三个字段每帧基于 viewport/transform 重建（l2d 层无缓存）。
`prepare` 入口无任何条件。

### 1.2 ayagami `prepare` 主体（`renderer.rs:948-1304`）

**入口 + 全局相机 uniform（L948-962，无条件）**：

```rust
948:     pub fn prepare(&mut self, encoder: &mut wgpu::CommandEncoder, options: &RenderOptions) -> bool {
...
958:         let srgb = options.colorspace == RenderColorspace::SRgb;
959:         let camera = GlobalUniforms::new(&(options.transform * Affine2::from_scale(vec2(1., -1.))));
960:         self.stat
961:             .queue
962:             .write_buffer(&self.global_buffer, 0, bytemuck::cast_slice(&[camera]));   // ← 每帧无条件
```

**mask 尺寸变更时重建（L966-987，守护）**：

```rust
966:         if self.mask_dimensions != options.mask_dimensions {
967:             self.offscreen_top.clear();
968:             self.buffer_pool.clear();
969:             self.shadow_fb = None;
970:             for (i, cs) in md.clip_sets.iter_mut().enumerate() {
...
975:                 cs.create_texture(&self.stat.device, &self.stat.mask_bind_group_layout,
976:                                  &self.stat.mask_sampler, options.mask_dimensions.x, options.mask_dimensions.y);
982:                 cs.dirty.set(true);
...
986:             any_changes = true;
987:         }
```

**ArtMesh 状态扫描 + 早退（L991-1026）**：

```rust
991:         for artmesh in m.artmeshes() {
992:             let state = md.driver.artmesh_state(artmesh.uid()).unwrap();
993:             if state.updated {
996:                 am_data.dirty = true;
997:                 any_changes = any_changes || state.visual.visible;
998:             }
999:         }
1003:         if options.transform != self.transform {
1004:             redraw_clips = true;
1005:             any_changes = true;
1006:             self.transform = options.transform;
1007:         }
1012:         for clip_set in md.clip_sets.iter_mut() {
1013:             if clip_set.update_queued.get() { any_changes = true; }
1015:         }
1019:         if md.update_queued.get() { any_changes = true; }
1024:         if !any_changes {
1025:             return false;       // ← 早退：静止帧后续 write 全部跳过
1026:         }
```

**ArtMesh uniform 整段映射 + 写入（L1028-1052）**：

```rust
1030:         let mut am_buf_view = self.stat.queue
1033:             .write_buffer_with(&md.artmesh_buffer, 0, md.artmesh_buffer.size().try_into().unwrap())
1037:             .unwrap();
1041:         am_buf_view.slice(0..core::mem::size_of::<ArtMeshUniform>())
1042:             .copy_from_slice(bytemuck::cast_slice(&[ArtMeshUniform { /* blit top-level */ ... }]));
```

注释 L1139-1140 明确：「Uniforms are uploaded in one operation, so build the
whole buffer including unchanged ArtMeshes」。

**逐 ArtMesh / Part 写 uniform（L1141-1205，无 dirty 守护，仅 `opacity==0` 跳过）**：

```rust
1141:             if state.visual.opacity != 0. {
1142:                 let artmesh_uniforms = ArtMeshUniform { opacity, multiply_color, screen_color, ... };
1153:                 let off = am_data.uniform_offset;
1154:                 am_buf_view.slice(off..off + core::mem::size_of::<ArtMeshUniform>())
1156:                     .copy_from_slice(bytemuck::cast_slice(&[artmesh_uniforms]));   // ← 每帧每 slot 重写
1157:             }
```

Part uniform 同形态（L1188-1204），逐 OffscreenPart 重写，无 dirty 守护。

**顶点数据上传（L1159-1176，dirty 守护）**：

```rust
1160:             if !am_data.dirty { continue; }       // ← 守护
...
1170:             .write_buffer_with(&md.vertex_buffer, start, size.try_into().unwrap())
...
1173:             vtx_buf_view.copy_from_slice(bytemuck::cast_slice(state.vertices));
1175:             am_data.dirty = false;
```

**clip mask 重绘（L1207-1284，dirty 守护）**：

```rust
1211:             if clip.cur_use_count == 0 || !clip.dirty.get() { continue; }   // ← 守护
...
1230:             let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
1231:                 label: Some("Mask Render Pass"), ...
1283:                 clip.update_queued.set(true);
```

`render_pass.draw_indexed` 重绘 `clip.texture`（不是 `write_texture`，是
render pass 写 color attachment + `StoreOp::Store`）。

**离屏 / shadow_fb / buffer_pool（L1286-1318, 1421-1430）**：

```rust
1306:     fn get_offscreen_buffer(&mut self) -> BufferTexture {
1307:         self.buffer_pool.pop().unwrap_or_else(|| {
1308:             BufferTexture::new(&self.stat.device, ..., wgpu::TextureFormat::Bgra8Unorm, "part_buf")
1317:         })
1318:     }
...
1421:             if self.shadow_fb.is_none() {
1422:                 self.shadow_fb = Some(BufferTexture::new(..., "shadow_fb"));
1431:             }
```

复用优先：`buffer_pool.pop()`、`shadow_fb.is_none()` 都是先复用后新建。

**`render()` 末尾清 dirty（L1623-1655）**：

```rust
1645:         if md.update_queued.get() { md.update_queued.set(false); }
1649:         for clip in md.clip_sets.iter() {
1650:             if clip.update_queued.get() { clip.dirty.set(false); clip.update_queued.set(false); }
1653:         }
```

---

## ② 资源清单表

| 资源 | 上传方式 | 触发点（file:line） | 频率 |
|---|---|---|---|
| 模型纹理 `self.textures[i]` | `Texture::from_bytes` → `from_image` → `queue.write_texture` + `premultiply` + `gen_mips` + `create_bind_group` | `renderer.rs:705-734`（`load_model`） | **一次性** |
| `texcoord_buffer` | `create_buffer_init` | `renderer.rs:771-778`（`reload_model`） | **一次性** |
| `index_buffer` | `create_buffer_init` | `renderer.rs:780-787` | **一次性** |
| `vertex_buffer`（顶点） | 空 buffer + 按 dirty `write_buffer_with` | 创建 `renderer.rs:764-769`；写 `renderer.rs:1167-1171` | 仅 `am_data.dirty` 时（`renderer.rs:1160` 守护） |
| `artmesh_buffer`（ArtMesh/Part uniform slots） | 空 buffer + 整段 `write_buffer_with` 拿 view + 逐 slot `copy_from_slice` | 创建 `renderer.rs:870-875`；写 `renderer.rs:1030-1038` + 1141-1157 + 1188-1204 | **变更帧**整段映射；**每帧按 `opacity != 0` 逐 slot 写**（**无 dirty 守护**） |
| `global_buffer`（相机 uniform，64B） | `create_buffer` + `queue.write_buffer` | 创建 `renderer.rs:533-538`；写 `renderer.rs:960-962` | **每帧无条件**（在早退 L1024 之前） |
| `clip.mask` 纹理本身 | `create_texture` → `BufferTexture::new`（R8Unorm） | `renderer.rs:113-135`；调用 `renderer.rs:975-981` | **仅 mask 尺寸变化时**（`renderer.rs:966` 守护） |
| `clip.mask` 内容 | render pass 重绘到纹理 | `renderer.rs:1230-1283` | **仅 `clip.dirty && cur_use_count != 0`**（`renderer.rs:1211` 守护） |
| Offscreen part texture `part_buf` | `BufferTexture::new`（Bgra8Unorm） | `renderer.rs:1306-1317` | buffer_pool 空时新建，否则复用 |
| `shadow_fb` | `BufferTexture::new` | `renderer.rs:1421-1430` | 首次缺失时新建一次，之后全程复用 |
| `buffer_pool` | `Vec<BufferTexture>` 复用 | `renderer.rs:1286`, `1412-1413` | 每帧末补回，**复用** |
| `uniform_bind_group` | `create_bind_group`（含 `global_buffer` + `artmesh_buffer` dynamic offset） | `renderer.rs:877-901` | **一次性**，每帧 `set_bind_group` 用 dynamic offset 切 slot（`renderer.rs:1260/1551/1591`） |
| 每张模型纹理 `texture_bind_group` | `create_bind_group` | `renderer.rs:712-728` | **一次性** |
| `mask_bind_group` | `BufferTexture::new` 内部 `create_bind_group` | `renderer.rs:80-93` | mask 重建时随之创建 |
| layouts / pipeline / shader / sampler | `create_*` | `renderer.rs:437-587`（`new`） | **一次性** |
| l2d 出图 target | `Texture::new`（Rgba8Unorm） | `l2d/renderer/offscreen.rs:120-128` | `new` 一次；`resize` 尺寸变了才重建（`offscreen.rs:83-86`） |

---

## ③ 疑似"每帧重复上传"点（含判定依据，不裁决）

> **术语界定**："上传" = 任何把数据送进 GPU 可见 buffer/texture 的动作：
> `queue.write_buffer` / `queue.write_texture` / `write_buffer_with`、
> `begin_render_pass` 写入纹理、`copy_texture_to_texture`、`create_buffer` /
> `create_texture` / `create_bind_group`。layout / pipeline / shader / sampler
> 只创建一次，**不**计入。

### A. `global_buffer`（相机 uniform，64B）— **每帧无条件**

- **位置**：`renderer.rs:960-962`。
- **行为**：prepare 入口前无条件 `queue.write_buffer`，**在早退 L1024 之前**。
- **判定依据**：源码无任何 `if` 包裹。`GlobalUniforms` 64 字节（`Mat4` 列主序）。
  `update()` 每帧 `stack.finalize() → set_pose`，相机矩阵实际**几乎每帧
  都变**（除非姿态栈完全静止）；但**即便静止也走这条 write**。
- **C5 关系**：带宽小（64B），但**确实每帧发生**；是否"浪费"取决于对
  静止帧的定义。

### B. `artmesh_buffer`（整段映射 + 逐 slot 写）— **变更帧无差别整段写**

- **位置**：
  - L1030-1038 整段 `write_buffer_with` 拿 view；
  - L1041-1052 blit top-level uniform（写第 0 slot）；
  - L1141-1157 逐 ArtMesh `copy_from_slice`（**无 dirty 守护**，仅 `opacity==0` 跳过）；
  - L1188-1204 逐 OffscreenPart `copy_from_slice`（**无 dirty 守护**）。
- **行为**：在 `any_changes == true` 的帧，**对所有 ArtMesh + Part slot 整段
  重新写一遍**。`ARTMES_HUNIFORM_STRIDE` = `size_of::<ArtMeshUniform>().next_multiple_of(256)`
  ≈ 256B/槽（`renderer.rs:376-377`）。
- **判定依据**：注释 L1139-1140 明确「build the whole buffer including
  unchanged ArtMeshes」。**上游刻意为之**。即顶点未变时，uniform 仍重写。
- **C5 关系**：**这是当前实现的事实**——uniform 不做 dirty 判别，仅依赖
  整体 `any_changes` 早退。

### C. `vertex_buffer` — **已有 dirty 守护**

- **位置**：L1167-1171。
- **守护**：L1160 `if !am_data.dirty { continue; }`。
- **`am_data.dirty` 生命周期**：L996（driver `state.updated` 触发）置 true →
  L1175（上传后）清 false → L1651（`render()` 末尾视 `update_queued`）清 false。
- **C5 关系**：**不是浪费点**。

### D. `clip.mask` 内容（render pass 重绘）— **已有 dirty 守护**

- **位置**：L1230-1283。
- **守护**：L1211 `if clip.cur_use_count == 0 || !clip.dirty.get() { continue; }`。
- **`clip.dirty` 生命周期**：L982（mask 尺寸变化）/ L1059（`redraw_clips`）/
  L1064（依赖了 dirty ArtMesh）置 true → L1651 清 false。
- **C5 关系**：**不是浪费点**。

### E. `clip.mask` 纹理本身 — **已有尺寸守护**

- **位置**：L975-981。
- **守护**：L966 `if self.mask_dimensions != options.mask_dimensions`。
- **C5 关系**：**不是浪费点**（视口稳定时完全跳过；尺寸变化本身就是一次大重建）。

### F. Offscreen part buffer / shadow_fb — **已复用**

- **位置**：L1306-1317 / L1421-1430。
- **守护**：`buffer_pool.pop().unwrap_or_else(...)` 与 `if self.shadow_fb.is_none()`。
- **C5 关系**：**不是浪费点**（已是最优：复用优先）。

### G. l2d 层 `RenderOptions` 字段

- **位置**：`l2d/renderer/model_core.rs:144-148`。
- **行为**：每帧基于 `self.viewport` / `self.transform` 重建栈上结构体。
  **不**涉及 GPU 资源，只是 CPU 栈上值。
- **`ModelRendererCore` 字段**（`model_core.rs:24-33`）：`gpu` / `renderer` /
  `stack` / `transform` / `viewport` / `colorspace`——**没有任何 wgpu::BindGroup /
  wgpu::Texture 字段**。l2d 公开 API 字段**全部是 CPU 状态**。
- **`InnerRenderer` 类型**（`mod.rs:40-41`）：`ModelRenderer<ayagami::file::ParsedModel,
  Arc<ayagami::file::ParsedModel>>`——ayagami 私有类型，l2d 无法窥探其内部。
- **C5 关系**：l2d 层**无法**缓存 bind group。要实现「l2d 层缓存未变更的
  bind group」必须修改 `ModelRendererCore` 字段，或在 `OffscreenRenderer`
  外层包一层"无变更时复用上一帧 `RgbaFrame`"——后者更现实但**不是缓存
  bind group，而是缓存整张出图**。

### H. 全局相机 uniform 与 transform 检测的相对独立性

- `options.transform` 在 l2d 层只受 `self.viewport` / `self.transform` 影响。
  `self.transform` 在 `load_model` 后定值（`model_core.rs:93`），除非
  `set_transform`（L130）。视口稳定则 `options.transform` 不变。
- ayagami `prepare` L1003 检测 `options.transform == self.transform`，
  不触发 `redraw_clips`。
- 但 L962 的 `global_buffer` 写入**仍照常发生**——与 `any_changes` 独立。

---

## ④ 给高级 AI 的「可核查判定方法」（只给方法，不给结论）

### 4.1 静态路径核查（已完成、记录在 ①-③）

可复现命令（统计 ayagami 上游所有"上传"动作）：

```bash
rg -n 'queue\.write_buffer|queue\.write_texture|write_buffer_with|create_buffer|create_texture|begin_render_pass|copy_texture_to_texture' \
   ~/.cargo/git/checkouts/ayagami-c3f4333abbf9e569/640ae4b/ayagami-render/src/renderer.rs
```

预期 3+2+1+多处，按本文 ① 节对照。`write_buffer` 仅 3 处：L962（无条件）、
L1033（早退后）、L1170（dirty 守护后）。

### 4.2 运行时观测：静止帧重放

**复现**：

1. 构造 `OffscreenRenderer`（`offscreen.rs:32`），`load_model` + 一次
   `update(FIXED_DT_60HZ)`（`model_core.rs:105`）使其 settle。
2. **不再调用任何 setter**（不 `set_parameter` / `override_parameter` /
   `update`），循环 N=300 次 `render_frame()`（`offscreen.rs:112`）。
3. 启用 wgpu trace：在 `gpu.rs:104-111`（即 `renderer.rs:104-111`）的
   `request_device` 注入 `trace: wgpu::Trace::File("trace.bin")`。
4. 用 `wgpu-info` 或自写 Python 解析 `trace.bin`，统计每帧 `BufferWrite` /
   `TextureWrite` / `RenderPass` 数。
5. **预期可观察**：
   - 每帧固定 1 次 `BufferWrite` 到 `global_buffer`（**L962 真实存在**）。
   - 理想静止帧**无** `RenderPass(Mask)`、**无**对 `artmesh_buffer` 的写入。
   - 若每帧仍出现对 `artmesh_buffer` 的写入，说明 `any_changes` 在静止
     场景下被置 true，需追查 L1012-1021 的 `update_queued` /
     `clip.update_queued` 未清零（grep `update_queued.set(false)`）。

### 4.3 tracing 日志注入

ayagami-render 已在 L971/995/1268/1611 等处用 `debug!` / `trace!` 宏。
**方法**：在 l2d 入口（`init_headless_device` / `ModelRendererCore::new`）
注入 env_logger：

```rust
env_logger::Builder::from_env(
    env_logger::Env::default().default_filter_or("ayagami_render=trace")
).init();
```

跑 4.2 静止循环，统计：
- `ArtMesh #N changed` 命中数（`renderer.rs:995`）—— 静止帧应 **0**。
- `Render ArtMesh #N for clip set` 命中数（`renderer.rs:1268`）—— 静止帧应 **0**。
- `Create clip set N texture` 命中数（`renderer.rs:971`）—— 视口稳定应 **0**。
- `Draw ArtMesh` 命中数（`renderer.rs:1611`）—— 静止帧若早退成功应 **0**（注意
  `render()` 不在早退路径内，故 L1611 的 `Draw` 仍会出现；这与 `prepare` 早退
  是两个独立阶段）。

### 4.4 自写计数器（不改上游）

**方法**：在 l2d 入口**之前**插一层薄包装（用 feature flag 或 `#[cfg(test)]`），
记录：
- `prepare` 前后 `wgpu::Device` / `wgpu::Queue` 暴露的统计（wgpu 28+ 提供
  `device.get_memory_usage` / 全局计数器在 trace 模式）。
- **不推荐**修改 l2d 源（任务门禁）。改用外部 binary：在 `tests/` 或
  `xtask/` 写一个 binary 调 `OffscreenRenderer::render_frame` 300 次，
  `std::time::Instant` + `device.poll(Wait)` 间隔粗略衡量提交大小与
  GPU 命令队列深度。

### 4.5 跨版本对照

`crates/l2d/Cargo.toml:14-15` 锁定 `rev = "640ae4b..."`。若想观察上游不同
commit：

```bash
cargo update -p ayagami --precise <other_rev>
```

重跑 4.2 / 4.3，对比 `write_buffer_with` / 早退 / dirty 清零位置是否变化。
重点关注 `renderer.rs:1024-1026` 早退点与 `renderer.rs:1141-1157` /
`1188-1204` uniform 写入段是否在后续 commit 收紧 dirty 判别。

### 4.6 l2d 层缓存 bind group 的边界判定

- **事实**：`ModelRendererCore` 字段列表（`model_core.rs:24-33`）**没有任何**
  wgpu 资源字段。`InnerRenderer` 是 ayagami 私有类型（`mod.rs:40-41`），
  l2d 公开 API 完全对外屏蔽。
- **核查方法**：`cargo doc --no-deps -p l2d` 生成文档后，搜 `BindGroup` /
  `Texture` / `bind_group` 在 `l2d/src/renderer/` 下的命中。**预期**：
  l2d 层只在 `OffscreenRenderer::target`（`offscreen.rs:22`）持有一张出图
  texture，无任何 model bind group 缓存。
- **若裁决认为应缓存**：必须改 `ModelRendererCore` 字段或在 `OffscreenRenderer`
  外加 wrapper。后者更现实——等价于"无变更时返回上一帧 `RgbaFrame`"，但
  **不是缓存 bind group**。

---

## 附录：关键行号速查

| 位置 | 含义 |
|---|---|
| `l2d/renderer/model_core.rs:139-181` | `render_to_view` |
| `l2d/renderer/model_core.rs:144-148` | `RenderOptions` 重建 |
| `l2d/renderer/model_core.rs:155` | `self.renderer.prepare` 无条件每帧 |
| `l2d/renderer/offscreen.rs:32-58` | `OffscreenRenderer::new` / `with_gpu_context` |
| `l2d/renderer/offscreen.rs:81-91` | `resize`（仅尺寸变化时重建 target） |
| `l2d/renderer/offscreen.rs:112-116` | `render_frame` |
| `l2d/renderer/gpu.rs:75-119` | `init_headless_device`（vulkan→gl→all） |
| `ayagami-render/src/renderer.rs:430-627` | `ModelRenderer::new`（静态资源） |
| `ayagami-render/src/renderer.rs:533-538` | `global_buffer` 创建 |
| `ayagami-render/src/renderer.rs:692-739` | `load_model`（模型纹理一次性） |
| `ayagami-render/src/renderer.rs:741-922` | `reload_model`（texcoord/index/vertex/artmesh buffer + uniform_bind_group） |
| `ayagami-render/src/renderer.rs:870-875` | `artmesh_buffer` 创建 |
| `ayagami-render/src/renderer.rs:877-901` | `uniform_bind_group` 创建 |
| `ayagami-render/src/renderer.rs:948-1304` | `prepare` 主体 |
| `ayagami-render/src/renderer.rs:960-962` | **每帧无条件 `write_buffer(global_buffer)`** |
| `ayagami-render/src/renderer.rs:966-987` | mask 尺寸变更时重建 |
| `ayagami-render/src/renderer.rs:991-999` | ArtMesh dirty 标记 |
| `ayagami-render/src/renderer.rs:1003-1007` | transform 变更检测 |
| `ayagami-render/src/renderer.rs:1024-1026` | early-exit（**唯一整体防线**） |
| `ayagami-render/src/renderer.rs:1030-1038` | `write_buffer_with(artmesh_buffer, 全长)` |
| `ayagami-render/src/renderer.rs:1139-1140` | 注释：「build the whole buffer including unchanged ArtMeshes」 |
| `ayagami-render/src/renderer.rs:1141-1157` | 逐 ArtMesh uniform 写入（**无 dirty 守护**） |
| `ayagami-render/src/renderer.rs:1159-1176` | 顶点写入（dirty 守护） |
| `ayagami-render/src/renderer.rs:1178-1205` | 逐 Part uniform 写入（**无 dirty 守护**） |
| `ayagami-render/src/renderer.rs:1207-1284` | clip mask 重绘（dirty 守护） |
| `ayagami-render/src/renderer.rs:1306-1318` | `get_offscreen_buffer`（pool 优先） |
| `ayagami-render/src/renderer.rs:1421-1430` | `shadow_fb`（首次缺失时建） |
| `ayagami-render/src/renderer.rs:1623-1655` | `render`（仅 set_* + draw + 清 dirty） |
| `ayagami/src/driver/mod.rs:864/896/1010/1014/1139` | `st.updated = true` 多处（驱动脏标志） |
| `ayagami-render/src/renderer.rs:376-377` | `ARTMESH_UNIFORM_STRIDE = size_of::<...>().next_multiple_of(256)` |

**术语**：
- **早退**：`renderer.rs:1024-1026`，`any_changes == false` 时 `return false`。
  返回值在 l2d 层**未使用**（`model_core.rs:155`）。
- **dynamic offset**：`uniform_bind_group` 第二个 binding 是 `has_dynamic_offset: true`
  （`renderer.rs:524`），每帧 `set_bind_group(0, &md.uniform_bind_group,
  &[am_data.uniform_offset as u32])` 切 slot——单 bind group 覆盖所有 ArtMesh。
- **`ARTMESH_UNIFORM_STRIDE`**：单 slot 256B 步长。
