# BATCH-0377 · ⭐ **那条 `||` 的来由被完全解释了**：它守的是一个**被 `.gitignore` 排除掉的事实**

Phase 1 · 域覆盖 · **Rust** —— `model_root()` 实现本体（B0376 留）

## 跑的命令（全部只读）
```
sed -n '/pub fn model_root/,/^}/p' model_root.rs
grep -nE "assets|is_dir|ancestors" model_root.rs | head -6
sed -n '37,75p' model_root.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐⭐ **F-0001-02 降级判定的最后一层证据到手**
```rust
pub(crate) fn model_root() -> PathBuf {                                     // :46
  let direct = cwd.join("assets").join("models");
  if direct.is_dir() { return direct; }                                     // ① 常态
  for d in cwd.ancestors() {                                                // ② 向上找
    if d.join("Cargo.toml").is_file() && d.join("assets/models").is_dir() { return d.join("assets/models"); }
  }
  direct                                                                    // ③ 兜底：返回**未命中的** direct
}
```
### ① ⭐⭐⭐ **`model_root()` 从不需要 `bai` 存在就能返回一个根** ⇒ **而它是对的**
- 第二级的判据是「**同时含 `Cargo.toml` 与 `assets/`**」（`:45` 逐字：「**避免在 `crates/*/Cargo.toml` 就停下**」）
- ⇒ ⇒ **那个判据与 `bai` 无关** ⇒ ⇒ **所以那条 `#[test]` 断言的其实是「这个仓库恰好带了 bai」这条**具体事实**
- ⇒ ⇒⇒⭐ 而 `:24` 逐字：「该目录被 `.gitignore` 排除（`assets/models/*`，**仅 `README.md` / `.gitkeep`**）」
  ⇒ ⇒⇒⇒ **那条测试断言的是一个被 `.gitignore` 排除掉的事实**
  ⇒ ⇒⇒⇒⭐ **而 `|| !root.join("bai").is_dir()` 正是为了让这个断言在「仓库不带 bai」时不失败而加的**
  ⇒ ⇒⇒⇒⭐⭐ **它把「断言的根据不成立」变成了「断言恒真」**
⇒ ⇒⇒ **这与 B0375 的降级理由完全一致** ⇒⇒ **P3 判定的最后一层证据到手**

### ② ⭐ `:57` 的兜底是「返回那个**未命中的** direct」**
⇒ ⇒ **不返回 `None`、不 panic** ⇒ ⇒ **「找不到」也是一种确定的值**
⇒ ⇒ **同 B0308「空态被定义」族**（**空 ≠ 缺失**）

### ③ ⭐ 而 `builtin_model_id()`（`:70-76`）又是一条 **P31**
> 「失败（常量被写成别的形状）= **空串**：**宁可状态栏空着，也不要报一个猜出**…」
⇒ ⇒ **猜出来的东西宁可留空** ⇒ ⇒ **P31 第五处**（UI 侧四处 · **CLI/Rust 侧第一处**）

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 定义本体 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 ·
   `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义 · `PersonaCardFilePicker` typedef 注释
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~580 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~425）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
