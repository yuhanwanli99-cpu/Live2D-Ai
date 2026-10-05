# BATCH-0213 · `wallpaper/strategy.rs` 的配置读取：**在转换前就挡掉 `NaN`/无穷**，且**两个读取器同口径**

Phase 1 · 域覆盖 · Mod crates 逐文件（`mod-wallpaper/src/strategy.rs`，655 行；本批定点其配置读取层）

## 跑的命令（全部只读）
```
grep -nE "^pub fn |^fn |^pub struct |interval|计时" mod-wallpaper/src/strategy.rs
sed -n '104,130p' mod-wallpaper/src/strategy.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；**一处强正面样本**（配置读取的失败面被逐条堵住）
### ① `interval_secs_from_config`（:110-120）**在转换之前**就挡掉两类危险值
```rust
let raw = config.get("interval_secs").and_then(|v| {
    v.as_u64().or_else(|| {
        v.as_f64()
            .filter(|f| f.is_finite() && *f >= 0.0)   // :115  ⭐ **先过滤，再转换**
            .map(|f| f as u64)
    })
});
raw.unwrap_or(DEFAULT_INTERVAL_SECS).clamp(MIN_INTERVAL_SECS, MAX_INTERVAL_SECS)   // :118-119
```
⇒ ⭐ **这个 `.filter` 是承重的**：Rust 里 `f64::NAN as u64` ⇒ **0**、`f64::INFINITY as u64` ⇒ **`u64::MAX`**
⇒ **若无此 filter**，`NaN` 会静默变成 **0 间隔**、无穷会变成一个巨大间隔
⇒ **失败被挡在「转换前」，而不是在转换后补救** —— 与 B0166 的 `clamp(MIN,MAX)`、
B0134 的 `.max(1)` **同形**（在边界上保证下游拿不到非法值）

### ② 而「永不返回 0」这个**后果**被写明了（:107-108）
> 「结果钳在 `[MIN, MAX]`（**永不返回 0**，于是 `WallpaperConfig::interval_ms` **恒非 0**，
> **策略里没有除零路径**）。」

⇒ **`interval_ms()`（:99-100）用 `saturating_mul(1000)`** ⇒ 大值不会**回绕**成小值/0
⇒ ⇒ **两级保证**：钳位（入口）⇒ 饱和乘（出口）⇒ **下游不需要自己再判一次**

### ③ ⭐ `playlist_len_from_config`（:124-130）**明确拒绝「报错」这个选项，并给了理由**
> 「超出上限 → 钳到 `MAX_PLAYLIST_LEN`（**不报错**：**前端多写一个 0 不该让 Mod 起不来**，
> 策略只需知道『有几张可切』）」

⇒ 这是一个**边界职责的判断**：**前端可以粗心，Mod 必须能起来** ⇒ **宽容放在 Mod 侧、错误留在真的错处**
⇒ 与 B0149 的 `hint_for`「401/403 才追加 `non_auth_note`」**同族**（按「谁能修」决定宽容度）

### ④ 两个读取器**同口径**（:129 逐字：「与 `interval_secs_from_config` **同口径**」）
⇒ **正面模式 P2（私有汇合）** 的形态：不是共享一个函数，而是**两条独立的路径写成同一口径**，
并**把「同口径」写成一句可核的断言** ⇒ 比共享函数更灵活（各自的缺省不同），比各写各的更可控。

## 未核实项（本批新增）
1. ⚠ **`MIN_INTERVAL_SECS >= 1` 我没核** —— 头注声称「永不返回 0」，而这**依赖下界 ≥ 1**
   ⇒ **下一批第一件事**（一行 grep）。若 `MIN` 真是 0，则该声称不成立、且存在**零间隔**路径
2. `strategy.rs` 其余 ~520 行未读（决策主体：播放列表前进 / 与舞台同步的判定）
3. `mod-template` 未读；Mod crates 逐文件覆盖率（11/61）
4. `design_tokens_test.dart` 余 ~32 条断言体 · `tokens.dart` 其余 ~600 行未读
5. `dev_tools_section.dart` 余面未读（1874 行）
6. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
7. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
8. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
