# BATCH-0091 · 第四次换根：`crates/l2d/` + `net.rs` 路径解析

Phase 1 · 域覆盖 · **换根到从未被追过的区**（`l2d` 此前 0 审）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/l2d/src/lib.rs` — 90（**全读**：crate 契约 + `V0_SUPPORTED_FORMATS`）
2. `crates/l2d/src/asset/mod.rs` — 571（定点 8-16 / 52-73：路径规范化与拒绝规则）
3. `crates/l2d-wasm-demo/src/web/net.rs` — （定点 52-63 `resolve_relative` / 94-110 `extract_refs`）

## 跑过的命令（全部只读）
```
git ls-files 'crates/l2d/' | xargs wc -l | sort -rn
sed -n '1,42p' l2d/src/lib.rs
grep -n '"\.\."|starts_with("/")|is_absolute|上跳|规范化' l2d/src/asset/mod.rs
grep -n "fn resolve_relative" -A 18 wasm-demo/src/web/net.rs
grep -n "fn extract_refs" -A 14 wasm-demo/src/web/net.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## ★ 本批产出：**F-0046-01 的机制削尖**（定级不变 P1，修法**增补第三条**）
- crate 的路径加固**确实存在且很严**（拒绝 `..` / 绝对路径 / `\`；错误携带原始资源名）—— 正面
- ⭐ **但它在抓取之后**：`main.rs:170-175` 先 `extract_refs` 解析**刚抓回的**字节 →
  `resolve_relative`（**纯文本拼接、零校验**）→ **立刻 fetch** ⇒ `asset/mod.rs` 的加固排在最后
- ⇒ 攻击者 model3.json 里的 `"Textures": ["../../../../api/v1/settings"]` 会让浏览器
  **抓本机服务的任意路径**，而加固在那之后才判定非法 —— **请求早已发出**
- ⇒ **修法增补 ③**：**在 `net.rs` 抓取前**校验 ref；**只加 origin 校验是不够的**
  （V1 `?model=` 根本不走 postMessage）
- **危害严格限定**：`..` **不能换 host**（`dir` 前缀恒在），绝对 URL / `//host` 都被拼成畸形同源路径
  ⇒ 仍是**盲抓 + 同 host 路径上跳**，**不夸大成任意主机 SSRF**

## 未核实项
1. `asset/mod.rs` 的 `normalize_resource_ref` **实现本体**未读（只核了它的契约与错误文案）
2. `asset/mod.rs` 余 ~500 行（磁盘入口 `load`、纹理组装、physics/cdi 取用）未读
3. `crates/l2d/` 其余 15 文件（`renderer/` 4 个 / `model.rs` / `format.rs` / `report.rs` /
   `examples/` 2 个 / `tests/bai_asset.rs`）未读
4. `extract_refs` 的 `FileReferences` 校验是否对 `Textures` 的**元素**也做了相对性检查 ——
   已核它只取字符串（:104-110 `filter_map(Value::as_str)`）⇒ **无校验**，但未核后续是否有别处补

## 本批新增
**0 条新缺陷** ｜ **F-0046-01 修法增补第 ③ 条（抓取前校验 ref）**
