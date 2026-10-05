# BATCH-0093 · `l2d/src/renderer/`：键规范化接缝（证伪）+ 档位 + GPU 获取

Phase 1 · 域覆盖 · `crates/l2d/src/renderer/`

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/l2d/src/asset/mod.rs` — 571（定点 188-223：`from_memory_map` → `assemble` 汇合点）
2. `crates/l2d/src/renderer/tier.rs` — 84（**读 1-50**：`RenderTier` / `from_side` / `required_max_texture_dimension` + 测试头）
3. `crates/l2d/src/renderer/gpu.rs` — 136（定点 88-136：适配器循环 / limits / `request_device` / 错误聚合）

## 跑过的命令（全部只读）
```
grep -n "fn from_memory_map" -A 32 asset/mod.rs
sed -n '1,50p' renderer/tier.rs
grep -rn "required_max_texture_dimension|request_device|downgrade|回落|降级" renderer/*.rs
sed -n '105,136p' renderer/gpu.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**0 条新发现**（三项核验通过）
### ① B0092 标的「潜在第二处接缝」—— **证伪**，且是**结构性**证伪
`from_memory_map`（:188-193）→ `from_memory` → **`assemble`（:199-206，唯一实现）**，
四路读全部经 `normalize_resource_ref`（:211/:215/:219/:223）。头注 :195-198 把理由写明：
> 「统一组装逻辑（私有）……磁盘入口把 `read` 实现为「base_dir 拼接 + fs 读」，
> 内存入口实现为「映射查找」；**两条入口在此汇合，行为必然一致**。」

⇒ **两条入口只在末端策略分叉，规范化在汇合点之前完成** ⇒ 不可能有「两种键形」。
⇒ **本审计第 4 个「汇合/漏斗」样本**：`_applyPrefs`（B0078）· `confirmLeave` 三处（B0082）·
`secrets::lookup` 单一入口（B0010）· **`assemble` 两条入口（B0093）**
⇒ **可提炼**：本仓表达「多入口一致性」的手法**统一为「私有汇合 + 策略闭包」**，
而不是「两条路径各自实现 + 测试保证相同」。

### ② `RenderTier`：**闭枚举 + 非法值不猜**（正面）
`from_side`（tier.rs:38-45）用穷尽 `match`，其余一律 `None`；调用方
（`l2d-wasm-demo/src/main.rs:470-473`）`if let Some(tier) = …` ⇒ **忽略非法值、不钳位**
（与 AGENTS.md「非法值忽略」一致）。头注还**如实标注了代价**：
4096:64MB / 8192:256MB / **16384:1GB**，并注明 16384「**需真机确认**」。

### ③ GPU 获取：**逐个尝试 + 聚合全部失败原因**（正面，且是「不静默降级」的形态）
`gpu.rs:95` `for (label, backends) in attempts` ⇒ 逐个试；
`request_device` 失败（:129-132）**记一条 note（含 adapter 名与错误）并继续**；
全部失败 ⇒ `:135` `Err(RenderError::NoGpuBackend(notes.join("; ")))`
⇒ **把每一次尝试的失败原因拼成一条消息交给用户**。
**关键判断**：用户要 16384 而 GPU 不支持时，本实现**不静默降到 8192**，
而是**如实失败并说明** ⇒ 与本仓「不谎报」的纪律一致（B0081 的「不谎报已生效」同族）。

## 未核实项
1. `renderer/mod.rs`(417) / `model_core.rs`(241，B0058 定点过 :84-104) / `offscreen.rs`(137) 未读
2. `tier.rs:51-84`（测试）未读
3. `asset/mod.rs` 余段（纹理解码 / physics3 解析 / cdi）未读
4. `crates/l2d/src/{model,format,report}.rs` 未读；`examples/` 2 个未读；`tests/bai_asset.rs` 未读

## 本批新增
**0 条**
