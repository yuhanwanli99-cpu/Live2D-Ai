# BATCH-0555 落盘 · ModStatus 五个变体，缺省那个带理由

## 跑的命令（全部只读）
```
grep -n "enum ModStatus" -A 14 crates/live2d-ai-desktop/src/mod_registry.rs   # => 零命中
grep -rn "enum ModStatus" -A 12 --include=*.rs crates/                          # => mod-system/src/status.rs
```

## 逐字（mod-system/src/status.rs:5-17）
```
:5  pub enum ModStatus {
:6    /// 未启用（manifest `enabled=false`）。
:7    #[default]
:8    Disabled,
:9    /// 正在构造/注册。
:10   Starting,
:11   /// 运行中（`start` 已成功）。
:12   Running,
:13   /// 失败（含原因；核心继续）。
:14   Failed { message: String },
:15   /// 正在关闭。
:16   Stopping,
:17 }
```

## 三个可核点
1. 五个变体是一条**完整生命周期**（未启用 → 启动 → 运行 → 失败 → 关闭）
   => 与 B0304 核的「相位从状态派生」同族（**状态是相位、不是布尔**）
2. `Disabled` 标 `#[default]`、而它的**理由写在头注里**（「manifest `enabled=false`」）
   => 与 B0583 `dark: true` / B0584 `restarted = false` / B0629 `primary = false` **同一族**
   （**缺省值 + 它的来源**）
3. `Failed { message }` 是**唯一带数据的变体**、头注写「（含原因；**核心继续**）」
   => 而 B0551 核的 api_version 门禁写的正是「不兼容 -> Failed，**主链不崩**」
   => **⇒ 同一个枚举、同一条承诺，两处各自写了一遍**（不是同一行，但是同族）

## 第四次记错路径（B0440）
`ModStatus` 我先在 `desktop/src/mod_registry.rs` 找 —— 零命中；
**实际在 `mod-system/src/status.rs`** ⇒ 这是本轮 B0440 第 1 种形态（路径不对）的又一次。

## 未核
`status.rs` 之外是否有 `From<ModStatus> for String` 之类的渲染映射 ·
`registry.rs` · `error.rs` · 其余 9 个 mod crate
