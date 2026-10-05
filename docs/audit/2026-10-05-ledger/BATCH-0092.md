# BATCH-0092 · `normalize_resource_ref` 本体与四路读点 —— **0 条新发现**（模式 H 第 7 次复查零命中）

Phase 1 · 域覆盖 · `crates/l2d/src/asset/`

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/l2d/src/asset/mod.rs` — 571（**定点 302-321 `normalize_resource_ref`** + 161/172/211/215/219/223
   四路读点 + **551-570 测试**）

## 跑过的命令（全部只读）
```
grep -n "fn normalize_resource_ref" -A 45 asset/mod.rs
grep -n "normalize_resource_ref(" asset/mod.rs
grep -n "fn load\b" -A 22 asset/mod.rs
sed -n '551,571p' asset/mod.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**0 条新发现**；crate 侧路径安全**完整且被测试钉住**
### 实现：比「归一后放行」**更严**——`..` 是**拒绝**而不是**折叠**
```rust
if name.is_empty() || name.starts_with('/') || name.contains('\\') { return Err(invalid()); }
for segment in name.split('/') {
    match segment {
        "" | "." => {}                 // 空段与 "." 折叠
        ".." => return Err(invalid()),  // :313 **任何一段是 ".." ⇒ 直接拒绝**
        _ => parts.push(segment),
    }
}
if parts.is_empty() { return Err(invalid()); }
```
⇒ 「`a/../b` 归一后其实安全」这种**放行**做法被**换成拒绝**——更严，且不依赖归一逻辑正确。
### 四个读点**全部**经过它（`:211` moc · `:215` 纹理 · `:219` physics · `:223` cdi），
落盘读是 `base_dir.join(规范化后的名字)`（:161）⇒ **磁盘入口无穿越面**。
### 测试**恰好钉住难形态**（模式 H 第 7 次复查，**零命中**）
```rust
for bad in ["", "/abs/moc.bin", r"a\b.png", "../up.moc3", "a/../b", "."] {
    let LoadError::InvalidResourcePath { name } = err else { panic!(…) };
    assert_eq!(name, bad, "错误必须携带原始资源名");
}
```
⇒ **`"a/../b"`（嵌入式上跳）在列** —— 朴素的「只查 `starts_with("..")`」会**漏掉它**，而它被显式钉住；
`.` 单独出现也钉住（靠 `parts.is_empty()` 兜住）。

### ⭐ 对 F-0046-01 修法 ③ 的**强化**：要加的校验**已经存在、已被测试**
B0091 记录的「抓取前必须校验 ref」，其规则集**就是**这个函数。
⇒ 修法因此从「加一套校验」（会被讨论「该用哪些规则」）变成
> **把 `normalize_resource_ref` 提前一个层次调用**：在 `l2d-wasm-demo/src/web/net.rs`
> 的 `fetch_bytes` 之前，对 `extract_refs` 得到的每个 ref 先过同一条规则。
> 规则本身**不需要新设计**，且**已有测试**（`asset/mod.rs:551-570`）。
⇒ 与 F-0034-01 同形：**正确的判据就在同一个 workspace 里，只是调用方用了更弱的那个**。

## 未核实项
1. `asset/mod.rs` 其余段落（纹理解码 / physics3 解析 / cdi 取用 / `from_memory_map` 细节）未读
2. `crates/l2d/src/renderer/` 4 个文件（`mod.rs` 417 / `model_core.rs` 241 / `offscreen.rs` 137 /
   `gpu.rs` 136 / `tier.rs` 84）未读
3. `crates/l2d/src/{model,format,report}.rs` 未读；`examples/` 2 个未读；`tests/bai_asset.rs` 未读
4. `from_memory_map` 是否**也**对传入 map 的键做规范化（doc :16 说「内存映射查找都使用规范化后的键」，
   **未逐行核**）—— 若否，则同一份数据经两条入口会有**两种键形**（潜在的第二处接缝）

## 本批新增
**0 条**
