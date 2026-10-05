# BATCH-0047 · F-0046-01 可利用性自查（→ 发现自身描述不完整，已改写）

Phase 1 · 域覆盖 → 渲染面

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/l2d-wasm-demo/src/main.rs` — 962（定点 158-198 `load_model_from_url`、:267 初始化）
2. `crates/l2d-wasm-demo/src/web/net.rs` — （**全读 1-45**：`fetch_bytes` / `model_url_from_query`）

## 跑过的命令（全部只读）
```
grep -rn "fn load_model_from_url" -A 40 crates/l2d-wasm-demo/src/
grep -rln "fn fetch_bytes" crates/l2d-wasm-demo/src/
grep -rn "fn fetch_bytes" -A 22 web/net.rs
grep -rn "model_url_from_query" crates/l2d-wasm-demo/src/
grep -rn "DEFAULT_MODEL_URL" crates/l2d-wasm-demo/src/
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出
**0 条新缺陷**；**F-0046-01 改写**（影响段）——
自查发现 `?model=` 查询串是**一条完全不需要 postMessage 的更低门槛入口**，
且 `fetch_bytes` 是**裸 `window.fetch` 零校验**，还会按 manifest 内容再抓 N 个相对资源。
原文把 V2 当成唯一入口，**低估了可达性**。已在 FINDINGS.md 保留原文 + 标出更正。

## 未核实项
1. `main.rs:720-962`（tick / 首帧就绪 / 交互）未读
2. 红线 N 正向验证（iframe 重建后状态恢复）未做
3. `web/surface/{render,idle}.rs`(821) 未读
4. `preset/*`(2800+ 行) 未读；`l2d/**`(16 文件) **完全未读**
5. 未验：浏览器侧 Private Network Access 对该 iframe 抓取的拦截强度（**不可作为定级依据**，仅备注）

## 本批新增
P0 0 · P1 0 · P2 0 · P3 0（**但改写了 F-0046-01 的影响段**）
