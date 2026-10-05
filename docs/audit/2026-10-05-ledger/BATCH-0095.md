# BATCH-0095 · `format.rs`：保守二进制探测 + **panic 级难形态被钉住**（模式 H 第 8 次，零命中）

Phase 1 · 域覆盖 · `crates/l2d/` 收尾

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/l2d/src/format.rs` — 242（定点 118-136 `guess_format` + 200-242 测试全段）

## 跑过的命令（全部只读）
```
grep -n "pub fn |MOC3_HEADER_SIZE|from_slice|get(" format.rs
sed -n '118,147p' format.rs
sed -n '200,242p' format.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**0 条新发现**；二进制解析的**顺序**与**测试**都正确
### ① 实现：**先查长度再索引**（这是 panic 级的关键顺序）
```rust
pub fn guess_format(header: &[u8]) -> MocFormat {
    if header.len() < MOC3_HEADER_SIZE || header[..4] != MOC3_MAGIC { return MocFormat::Unknown; }  // :126
    if !matches!(header[5], ENDIAN_LITTLE | ENDIAN_BIG) { return MocFormat::Unknown; }              // :129
    match MocVersion::from_raw_byte(header[4]) {
        Some(version) => MocFormat::Moc3(version),
        None => MocFormat::Unsupported(header[4]),      // :134 携带原值，供报告与日志
    }
}
```
- `len() < 64` 与 `header[..4]` 在**同一条 `if`** ⇒ 短路保证切片只在 `len() ≥ 64` 时求值
  ⇒ **空输入 / 6 字节输入都不会越界**；
- 字节序标志走**白名单**（0/1），其余 ⇒ `Unknown`（头注 :122 注明「参照 Mocari 拒绝语义」）；
- 版本走**闭集白名单**（`from_raw_byte`），未命中 ⇒ `Unsupported(原值)` **不推断**。

### ② ⭐ 测试**恰好钉住那个会 panic 的形态**（模式 H 第 8 次复查，**零命中**）
```rust
fn truncated_or_garbage_input_is_unknown() {
    assert_eq!(guess_format(b"MOC3\x04\x00"),        MocFormat::Unknown);  // ← 6 字节：**魔数对、版本字节合法**
    assert_eq!(guess_format(&BAI_HEADER[..63]),     MocFormat::Unknown);  // ← 合法 Bai 头**少 1 字节**
    assert_eq!(guess_format(b""),                   MocFormat::Unknown);  // ← 空
    …
}
```
⇒ 第一条是**最有价值的一条**：一个**只有 6 字节、但前缀与版本字节都合法**的输入。
若长度检查被移到索引之后（或被删掉），这条会 **panic** 而不是 fail
⇒ 它把「先查长度」这个**顺序属性**变成了可执行断言。
⇒ 第二条把边界钉在 **63 vs 64** 的**差一**上。
⇒ 另有 `illegal_endian_flag_is_unknown`（:215）与 `other_known_versions_are_not_v0_guaranteed`（:221，
  逐个已知版本断言「识别为 Moc3 但**不被 v0 背书**」）⇒ 白名单与 v0 保证**被分开钉住**。

## `crates/l2d` 覆盖小结（本区 5 批，B0091–B0095）
**已读**：`lib.rs`(90 全读) · `format.rs`(242 定点) · `asset/mod.rs`(571 定点 4 段) ·
`renderer/mod.rs`(417 定点) · `renderer/tier.rs`(84) · `renderer/gpu.rs`(136 定点) ·
`pose_stack.rs`(503 定点 2 段，B0054) · `renderer/model_core.rs`(241 定点，B0058)
**未读**：`model.rs`(318) · `report.rs`(237) · `renderer/offscreen.rs`(137) ·
`model_core.rs` 余段 · `examples/`(2) · `tests/bai_asset.rs`(177)
⇒ **按文件计 8/16 = 50%**，按行计约 **45%**。

## 未核实项
1. `model.rs`(318) —— `ModelHandle` / `LoadedModel` 的**交叉核对**逻辑（lib.rs:8-9 称
   「『头探测 × 运行时版本』交叉核对（不一致即类型化否决）」）**未读**，而这是本 crate 的核心承诺之一
2. `report.rs`(237) —— 兼容报告的类型化诊断**未读**
3. `renderer/offscreen.rs`(137) 未读
4. `examples/` 2 个（`static_frame_trace.rs` 323 是本 crate 第二大文件）未读
5. `tests/bai_asset.rs`(177) 未读

## 本批新增
**0 条**
