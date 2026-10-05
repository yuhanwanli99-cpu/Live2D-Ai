# BATCH-0279 · ⭐ 「必须保留」的不变式**写在拆分现场**，且**唯一的写入者被点名**

Phase 1 · 域覆盖 · 回读 `l2d-wasm-demo` 余面 —— `web/surface/idle.rs`(270)

## 跑的命令（全部只读）
```
sed -n '1,16p' crates/l2d-wasm-demo/src/web/surface/idle.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ **一条 AGENTS 里标了「必须保留」的红线，在代码里被正确守着**
```rust
//! 待机生命体征（RM6）：**呼吸 / 眨眼 / 微表情**。                    // :1
//! 与动作系统是**两套机制**：动作层**已整体删除（rc.2）**，本模块是产品路径上
//! 待机体征，**必须保留**。每帧由 [`super::input::apply_bridge_effects`] 调用
//! [`apply_idle_life`]，经 `set_parameter` 写 input 层。               // :2-6
///
/// 2026-09-12（rc.2）起**其上不再有动作 `final_override` 层**（动作已整体删除），
/// breath/blink 参数（ParamBreath / ParamEyeLOpen / ParamEyeROpen）
/// **仍然只由这里驱动**。                                            // :12-15
```
四个可核点：
1. ⭐ **不变式写在「拆分现场」** —— 「**必须保留**」+「与动作系统是**两套机制**」
   ⇒ ⇒ **将来再拆这个文件的人，第一眼就在头注里看到它**（AGENTS 的休眠台账对它有专门一行）
2. ⭐ **它点名了「造成这个风险的那次删除」**：「动作层**已整体删除（rc.2）**」
   ⇒ ⇒ 头注说出的正是**最可能顺手删掉它的那次操作**
3. ⭐ **并记录了那次删除的**后果**：「**其上不再有动作 `final_override` 层**」
   ⇒ ⇒ AGENTS 台账原话是「只共用 override 层」⇒ ⇒ **现在那层没了，头注记下了新的现实**
4. ⭐⭐ **唯一的写入者被点名**：「breath/blink 参数…**仍然只由这里驱动**」
   ⇒ ⇒ **一个状态、一个写入者、名字写在文件里**
   ⇒ ⇒ **本轮又见一例**同一纪律：`presetStatus`「**只由 ack 驱动**」（B0232）·
   `presetStatusUpdateFor` 是**唯一写者**（B0268）· `canPreserveStage` 不建镜像（B0116）·
   事件面 9 臂**无兜底**（B0135）
5. 另：保留了**出处与许可**：「照抄 py 版 `idle-breath.ts` / `idle-blink.ts` / `idle-micro-expr.ts`（**MIT**）」
   ⇒ ⇒ **跨重写仍留许可署名**（本仓另有 `mod-community-license.md` 专门写许可边界）

⇒ ⇒ **0 findings**；而这一批确认的是**一条 AGENTS 明写「绝不可删」的东西，确实还在、且理由写在原地**

## 未核实项
1. `web/surface/{render,gpu}.rs` 与 `main.rs`(962) 未读
2. `apply_bridge_effects` 的**调用顺序**（idle 每帧被调，但与 `final_override` 的关系已变）—— 未核
3. ⭐ F-0278-01 敞口：`apply` 的**分支体**未读 ⇒ 「某路径上 CSS 仍生效」未证
   ⇒ 但该注释**没说「仅在某条件下」** ⇒ **至少不完整** ⇒ **不因未证例外而上调级别**
4. `stage-bg.dataUrl` 的**发送方**在 Flutter 侧哪一处 —— 未核
5. 真实动作计划的 token 长度（F-0020-01 永久敞口）· `shell/flutter` 是否真可能出现密钥
6. 那四个「未知 id」在文档里有没有被点名（B0270 敞口）· `refresh_from_disk` 敞口（B0266）
7. 能否点名会在持锁期间 panic 的 Mod（B0268 敞口）
8. 面板余面：`dev_tools_section` ~1845 · `live2d_stage` ~600 · `memory_panel` ~545 ·
   `persona_panel` ~520 · `message_bubble` ~470 · `chat_panel` ~465 · `error_banner` ~15
9. 其余 6 处 `mod_count_is_*` 逐条对账未核 · `ErrorResponse` 定义未读
10. `onReady` 是否间接清过 `presetStatus`（F-0263-01 反证 (a)，**结论不依赖它**）
11. **两侧措辞同步无机制**（B0260 敞口）· `chat_session.dart` 的 `maxSessions=50` 施加点未读
12. 前端 `.dart` 仍 181 未读（**机械值**）；Mod crates 42 未读；`mod-system` 余 8 文件未读
13. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
14. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
15. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
