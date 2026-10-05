# BATCH-0220 · `persona/src/sessions.rs`：**职责归属写在头注、落盘点 tmp+rename 且注明「同一条纪律」**

Phase 1 · 域覆盖 · Mod crates 逐文件（14/61）—— `mod-persona/src/sessions.rs`(303)

## 跑的命令（全部只读）
```
wc -l mod-persona/src/*.rs ; sed -n '1,16p' mod-persona/src/sessions.rs
grep -n "plan_atomic_write|fs::write|File::create|rename|tmp" mod-persona/src/sessions.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；**两处边界纪律核验通过**
### ① ⭐ 头注**先说清「这一半归谁」**（:1-16）
> 「L1 验收要求『角色卡绑定**当前会话**：**换会话不串卡**、**回原会话仍在**』。
> 宿主 `SessionPromptSink`（`live2d-ai-mod-system::session`）**只活在进程内存里，进程一停就没了**；
> 『**回原会话仍在*』**跨重启**那半条**必须由本 Mod **自己落盘**。」

⇒ ⇒ **进程内那半归宿主、跨重启那半归本 Mod**，**在头注里分清**
⇒ ⇒ 这正是我在 F-0062-01 记的失败形态（「三段职责无人认领」）的**反面做法**
⇒ 且它**点名了对方的模块名**（`SessionPromptSink`）⇒ 读者能顺着去看「那半归谁」

### ② 而**存的是规范化后的形式**，并给了理由（:12-15）
> 「每个值是**规范化后的 V2 卡**（`PersonaCard::to_json`），**不是原始文本** ——
> 『档案里存的到底是什么』因此**可读、可审计**，也能原样再解析。」

⇒ ⇒ 与 P25 / P2 同形：**在边界上存「已规范化的那一份」**，于是「存的是什么」**没有歧义**
⇒ 且 :15-16「会话 id 是**宿主归一化过的**（`sanitize_session_id`），**本文件不自己发明**」
⇒ ⇒ **不自己发明第二套归一化** ⇒ 正是我在 B0215 核过的 wallpaper `settings_spec` 那条纪律
（**同仓、同纪律、不同处**）

### ③ 落盘是**原子写**，且**注明属于哪条纪律**（:29 / :274-300）
```rust
// tmp + rename（**与 host 写 mods.json / 本 Mod 写导入卡同一条纪律**）：            // :29
/// 原子写档案（tmp + rename）。**序列化 / 落盘失败 → Err（调用方决定怎么报）**。  // :274
let tmp = path.with_file_name(format!("{SESSION_CARDS_FILE}.tmp"));               // :296
std::fs::write(&tmp, text).map_err(…)?;                                          // :297-298
std::fs::rename(&tmp, path).map_err(|e| { let _ = std::fs::remove_file(&tmp); … });  // :299-300
```
四个可核点：**原子**（tmp+rename）· **tmp 名带命名空间**（`persona-mod-cards.json.tmp`，
与 B0121 核的 `plan_atomic_write`「保留完整文件名」**同一纪律**）· **rename 失败清理 tmp**（:300，
与 `plan_atomic_write` 的 `Err` 路径**同形**）· **失败上抛给调用方决定怎么报**（:274）

⇒ ⇒ **原子写纪律在本仓有四处**：`live2d-ai.toml`（`plan_atomic_write`，B0120/B0121）·
`.env`（自带 chmod+rename，B0120）· `mods.json`（host）· **本文件**
⇒ 而**它们不是共享一个函数，而是「同一条纪律」被点名四次**（:29 逐字「同一条纪律」）
⇒ ⇒ **P25 的又一形态**：**跨 crate 无法共享时，就用「命名 + 指名」代替「引用」**

## 未核实项
1. `sessions.rs` 其余 ~270 行未读（读/删会话、`sanitize_session_id` 侧、错误语义）
2. `mod-persona` 其余：`lib.rs`(1004) 只定点读过 `compose_system_prompt` · `command.rs`(463) 只读过三处
3. Mod crates 逐文件覆盖率（14/61）
4. `dev_tools_section.dart` 余面未读（1874 行）· `tokens.dart` 余 ~600 行 · `design_tokens_test.dart` 余 ~32 断言
5. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
6. **前端 `.dart` 183 未读（最大面）**
7. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
8. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
