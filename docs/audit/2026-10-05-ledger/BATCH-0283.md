# BATCH-0283 · ⭐ `gpu.rs`：**一次失败尝试被完整记下**（现象 · 结构性理由 · 为什么不再需要 · 结论），且**与 AGENTS 互相印证**

Phase 1 · 域覆盖 · 回读 `l2d-wasm-demo` 余面 —— `web/surface/gpu.rs`(168)

## 跑的命令（全部只读）
```
sed -n '1,20p' crates/l2d-wasm-demo/src/web/surface/gpu.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ **本仓「记录失败尝试」的最佳样本**
```rust
//! **2026-09-14 rc.5 记录**：**曾把这里改成「WebGL2 优先」以换取透明 canvas**，
//! 但 HUD 实测用户机仍走 WebGPU（`request_gl` **未生效**），且透明 canvas 在
//! WebGPU 下**结构性不可达**。舞台底色与背景图现在**画进 framebuffer**（见 `background.rs`），
//! 因此**不再需要动后端顺序——恢复 WebGPU 优先**。                    // :6-10
```
五个可核点：
1. ⭐ **记的是一次**失败尝试**（「曾改成 WebGL2 优先」）⇒ 而**那段代码确实不在**（头注写的是「恢复」）
2. ⭐ **写明失败的样子**：「`request_gl` **未生效**」⇒ ⇒ **可核的具体现象**，不是「效果不好」
3. ⭐ **写明为什么不能改**：「透明 canvas 在 WebGPU 下**结构性不可达**」
   ⇒ ⇒ 「**结构性**」把「难办」升级成「**做不到**」
4. ⭐⭐ **写明现在的方案为什么取代了那个尝试**：「底色与背景图现在**画进 framebuffer**」
   ⇒ ⇒ **新方案让老问题不再存在** ⇒ ⇒ **「换一条路」而不是「继续试」**
5. ⭐ **结论落在同一行**：「因此**不再需要动后端顺序**」⇒ ⇒ **「不需要」比「不要」更好** ——
   它说明这个约束**已经消失**，而不是「谁也别去动」

### ⇒ 而它与 AGENTS rc.5 ⑥ **说的是同一件事、互相印证**
AGENTS rc.5 ⑥ 逐字记着「曾据此改成 WebGL2 优先想换透明 canvas，但**用户机 HUD 实测仍是 `GPU: webgpu/WebGPU`**（`request_gl` 未生效）→ 该改动**已回滚**（`gpu.rs` 恢复 WebGPU 优先）」
⇒ ⇒ **代码头注与变更历史对同一次失败实验的描述一致**
⇒ ⇒ ⭐ **这正是 F-0209-01 的镜像**：那里是**只有文档旧**；**这里是代码与文档都写对了**
⇒ ⇒ ⇒ **同一个仓里两种状态并存** ⇒ 而**判据不是「文档可不可信」，是「这条事实有没有两边都记」**

## 未核实项
1. `main.rs`(962) 未读
2. `apply_bridge_effects` 的**调用顺序**未核（B0279 留）
3. `stage-bg.dataUrl` 的**发送方**在 Flutter 侧哪一处 —— 未核（B0277 留）
4. 真实动作计划的 token 长度（F-0020-01 永久敞口）· `shell/flutter` 是否真可能出现密钥
5. 那四个「未知 id」在文档里有没有被点名（B0270 敞口）· `refresh_from_disk` 敞口（B0266）
6. 能否点名会在持锁期间 panic 的 Mod（B0268 敞口）
7. 面板余面：`dev_tools_section` ~1845 · `live2d_stage` ~600 · `memory_panel` ~545 ·
   `persona_panel` ~520 · `message_bubble` ~470 · `chat_panel` ~465 · `error_banner` ~15
8. 其余 6 处 `mod_count_is_*` 逐条对账未核 · `ErrorResponse` 定义未读
9. `onReady` 是否间接清过 `presetStatus`（F-0263-01 反证 (a)，**结论不依赖它**）
10. **两侧措辞同步无机制**（B0260 敞口）· `chat_session.dart` 的 `maxSessions=50` 施加点未读
11. 前端 `.dart` 仍 181 未读（**机械值**）；Mod crates 42 未读；`mod-system` 余 8 文件未读
12. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
13. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
14. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
