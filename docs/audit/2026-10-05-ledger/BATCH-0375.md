# BATCH-0375 · 🔻🔻 **一条 P1 降为 P3** —— 我记错了主体（它在 `#[test]` 里，而生产侧**没有**这个判据）

Phase 4 · **证伪** —— 结清 B0374 留的「那条 assert 还有没有调用者」

## 跑的命令（全部只读）
```
sed -n '100,120p' crates/live2d-ai-desktop/src/web_api/model_root.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**F-0001-02 由 P1 降为 P3**
```rust
/// 从 crate 目录启动时也要落到仓库根（`cargo test` 的 cwd 就是 crate 目录）
#[test]
fn model_root_walks_up_to_the_repo_root() {
  let root = model_root();
  assert!(
    root.join("bai/runtime/bai.model3.json").is_file() || **!root.join("bai").is_dir()**,
    "向上找到的根应含内置模型的 model3.json：{}", root.display());
  assert!(root.parent().is_some_and(|p| p.ends_with("assets")), …);
}
```
### ① ⭐⭐⭐ **它在 `#[test]` 里** ⇒ **它是一条「测试断言」，不是「生产校验」**
### ② 而 **B0374 刚核过**：生产路径 `models_routes::handle_import` 用 `find_model3_json`
**做了正确的四段判定**（`invalid_model3_json` + 文案）⇒ ⇒ **生产侧根本没有这个判据**
⇒ ⇒⇒ **我记的「整个判据形同虚设、导入可能静默通过」** —— **生产侧不存在这条路径**
### ③ ⇒ **实际影响**：**这条测试失去了它要证明的能力**（`model_root()` 是否找对了根），
**而不是**「用户会导入失败」⇒ ⇒ **降 P3**；**修法随之变简单**：
**去掉那个 `||`**（让 `bai` 不存在时**测试失败并暴露真相**），**或该测试随 `bai` 被移除而一并删除**

### ④ ⭐⭐⭐ 而本条最大的价值是它提炼出的规则
> **在测试里发现的空洞，默认要问「它在守什么」。**
> 若它守的判据**在别处已被正确实现**，那是**测试的缺陷**，不是**系统的缺陷**。
> ⇒⇒ **级别要按「谁受害」定，不是按「代码多丑」定。**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `model_root()` 的**实现本体**（它怎么向上走）· `assets/models` 被移除后**是否还有其他 `#[test]` 依赖 `bai`**
2. `UiSignals` 定义本体 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 ·
   `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义 · `PersonaCardFilePicker` typedef 注释
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~580 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~425）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
