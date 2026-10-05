# BATCH-0094 · `renderer/mod.rs` 便利层：热路径无分配 + 冒烟 oracle 可失败

Phase 1 · 域覆盖 · `crates/l2d/src/renderer/`

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/l2d/src/renderer/mod.rs` — 417（API 枚举 + 定点 123-167：`aspect_fit_transform` /
   `RgbaFrame` / `alpha_coverage` / `visible_bbox`）

## 跑过的命令（全部只读）
```
grep -n "pub fn |fn render_to_view|fn update" renderer/mod.rs
grep -n "vec!|Vec::|to_vec()|collect()|String::" renderer/mod.rs
sed -n '128,167p' renderer/mod.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**0 条新发现**（两项核验通过）
### ① 维度 D：**热路径零分配**（正面）
全文件的分配只出现在三处：`save_png` 的一次性拷贝（:187 `RgbaImage::from_raw(…)`）
与**测试代码**（:401 `pixels: vec![0; 16]`、:406 `let mut pixels = vec![0_u8; 12]`）。
⇒ **生产热路径（`render_to_view` / `update`）无 `vec!` / `collect` / `to_vec` / `String`**。
（注：`aspect_fit_transform` 返回 `Affine2::from_scale` 是栈值，:130。）

### ② 冒烟 oracle **不是恒真的**——两个判据都能真失败（正面，且是模式 B 的反例）
```rust
pub fn alpha_coverage(&self) -> f64 {                    // :144
    if self.pixels.is_empty() || self.width == 0 || self.height == 0 { return 0.0; }
    let opaque = self.pixels.as_chunks::<4>().0.iter().filter(|px| px[3] != 0).count();
    opaque as f64 / (self.width as f64 * self.height as f64)
}
pub fn visible_bbox(&self) -> Option<(u32,u32,u32,u32)> { // :160
    … if px[3] == 0 { continue; } …  全透明 ⇒ None
}
```
- 全透明帧 ⇒ `coverage == 0.0` 且 `bbox == None` ⇒ 断言 `coverage > 0` / `bbox.is_some()`
  的测试**会真红**；
- 空缓冲 / 零尺寸**显式返回哨兵值**而不是 panic（:145-147 / :161-163）；
- 用 `as_chunks::<4>()`（= `chunks_exact`）而非 `chunks_exact(4).remainder()` 的手写循环
  ⇒ 无越界 panic 面。
⇒ **这是模式 B（假绿灯）的反例**：判据有**明确的失败信号**，且哨兵值有测试。

### 顺带记一条**有界的观察**（不入发现）
`as_chunks::<4>().0` 会**静默丢弃**末尾不足 4 字节的残余；而分母用的是**声明的**
`width * height`（:155）。若将来某条路径产出**尺寸不符**的缓冲，
`coverage` 会偏**小**而不是报错。
**为什么有界**：结构文档写明不变量（:138「长度 = width * height * 4」），
且缓冲由固定尺寸渲染目标产出 ⇒ 不变量**由构造保证**；
而任何「尺寸严重不符」都会被 `coverage > 0.5` 这类断言抓住 ⇒ **不会静默通过**。
**记此备忘**是为了日后若有人加「逐像素比对」类断言时知道这条边界。

## 未核实项
1. `renderer/mod.rs:168-417`（`save_png` 实现 / `OffscreenRenderer` 本体 / 409-417 测试）未读
2. `renderer/model_core.rs`(241) 的其余段（B0058 读过 :84-104）· `renderer/offscreen.rs`(137) 未读
3. `crates/l2d/src/{model,format,report}.rs` 未读
4. `examples/{render_model,static_frame_trace}.rs` 未读；`tests/bai_asset.rs`(177) 未读
5. **`crates/l2d` 的 16 个文件中已读 7 个**（`lib.rs` / `asset/mod.rs` / `renderer/{mod,tier,gpu}.rs`
   + 2 处定点）⇒ 覆盖率约 **45%**

## 本批新增
**0 条**
