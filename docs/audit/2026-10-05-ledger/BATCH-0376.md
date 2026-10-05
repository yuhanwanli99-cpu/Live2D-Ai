# BATCH-0376 · ✅ **F-0001-02 这条线彻底闭合** —— 而那个 `#[test]` 的 `||` 是**唯一**的缺口

Phase 1 · 域覆盖 · **Rust**（CLI 侧）—— `path_resolve` 的缺省模型解析（B0375 降级后剩余的活）

## 跑的命令（全部只读）
```
grep -rn "bai" crates/live2d-ai-desktop/src/ --include=*.rs | head -8 ; grep -rc "bai" …/model_root.rs
grep -rn "DEFAULT_BAI_MODEL3_RELATIVE" crates/ --include=*.rs
sed -n '28,58p' crates/live2d-ai-desktop/src/cli/path_resolve.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**F-0001-02 的线彻底闭合**（0 条新发现）
### ① 其余 `bai` 引用都**不是**「仓库自带 bai」的依赖
| 位置 | 性质 |
|---|---|
| `adapter/mod.rs:285/317` | **测试名**里的 `bai`（frame entries / 眼睛开合 around bai state）⇒ **不是路径依赖** |
| `cli/tests.rs:294-296/320` | ⭐ **测试自己 `create_dir_all` + `fs::write` 造了一个 `bai` 目录与假 manifest** |
| `cli/mod.rs:48` | `DEFAULT_BAI_MODEL3_RELATIVE` ⇒ **一个生产常量** |

### ② ⭐⭐ 而缺省路径解析这一侧**把三件事都做到了**（`path_resolve.rs:34-53`）
```rust
let candidates = [cwd_base.join(relative), manifest_dir.join("../../").join(relative)];  // ① 两个候选
for candidate in &candidates { if candidate.is_file() { return Ok(candidate.clone()); } }   // ② 都试
Err(format!("未找到缺省 Bai 皮套清单（{}），尝试过:\n  {tried}\n  请用 --model-smoke <model3.json> 显式指定路径", …))
//  ③ **列出试过的每一条** + **给出可执行的下一步**
```
⇒ ⇒ **「默认模型不存在」这个必然发生的失败，被转成了一条带上下文的可操作指令**
⇒ ⇒⇒ ⭐⭐ **这是 B0352 那条 P31 的第四处应用**（前三次都在 **UI 侧**；**这次在 CLI 侧**）
⇒ ⇒ **同 B0197「快照要能拿到」· B0316「把机器说清」的思路**

### ③ ⇒ **所以这条线上三处的状态是**
| 位置 | 状态 |
|---|---|
| 路由层 `handle_import` | ✅ **对**（四段判定 + 码 + 文案）—— B0374 |
| CLI 缺省解析 | ✅ **对**（两候选 + 列出试过 + 指下一步）—— **本批** |
| **`model_root.rs:111` 的 `#[test]`** | ⚠ **唯一缺口**（`\|\| !root.join("bai").is_dir()` 空洞）⇒ **P3** |

⇒ ⇒⭐⭐⭐ **而 `cli/tests.rs` 那个测试**自己造 `bai`**，**正是 `model_root_walks_up_to_the_repo_root()`
应该做的事** ⇒ ⇒ ⇒ **它才是「测试没做到它该做的事」的那个实例** ⇒ ⇒⇒ **B0375 的降级完全站得住**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `model_root()` 的**实现本体**（怎么向上走 · 候选顺序）—— 未读
2. `UiSignals` 定义本体 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 ·
   `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义 · `PersonaCardFilePicker` typedef 注释
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~580 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~425）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
