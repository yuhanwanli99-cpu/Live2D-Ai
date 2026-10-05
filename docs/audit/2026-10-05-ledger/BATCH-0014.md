# BATCH-0014 · web_api 最后三个测试文件 → 本目录结项

Phase 1 · 域覆盖 → `crates/live2d-ai-desktop/src/web_api/**`（**结项批**）

## 读过的文件（来自 `git ls-files`，行数 = `wc -l` 真值）

1. `crates/live2d-ai-desktop/src/web_api/tests_dev_mode.rs` — 328（**全读**）
2. `crates/live2d-ai-desktop/src/web_api/tests_mod.rs` — 266（**全读**）
3. `crates/live2d-ai-desktop/src/web_api/tests_ws_client.rs` — 257（**结构枚举 + 2 条测试名**；
   它是 helper 模块，风险低，未逐行读）

## 跑过的命令（全部只读）
```
grep -n "pub fn |pub(crate) fn |fn |#\[test\]" tests_ws_client.rs
grep -rn "RouteId::Env|/api/v1/env" tests_mod.rs tests_security.rs    # → 零命中
sed -n '111p' model_root.rs ; sed -n '334p' cli_entry.rs             # P1 复核
sed -n '177,178p' supervisor_slot.rs                                  # P1 复核
grep -n "lock()\.unwrap()" mod_registry.rs | head -3 ; sed -n '252,253p' mod_registry.rs
for f in chat_routes external_routes voice_routes mods_routes; do grep -c 'tracing::' ...; done
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 模式 B 复查最终结论（web_api 全目录）
| 文件 | 结论 |
|---|---|
| `tests_dev_mode.rs`（9 条，全读） | **零模式 B**，且是**质量上界**：三态矩阵（`null`/缺省/值）全覆盖、双向翻 403↔200、同时断言**内存态与磁盘文件**（:324-325）、`tmp_ctx_with_settings` 用 pid+name+thread_id 唯一化并写明理由（:30-34） |
| `tests_mod.rs`（14 条，全读） | 零模式 B，但**契约覆盖面缺一块** → F-0014-01。另 :194-199 记录了一次「不得依赖机器状态」的既往修复（模型列表测试曾假设本机 registry 为空），是**主动消除假红灯**的正面案例 |
| `tests_ws_client.rs`（2 条） | helper 模块，两条测试名（`handshake_residual_bytes_are_retained` / `first_ws_frame_coalesced_with_101_is_not_lost`）均承诺可观察行为 |

⇒ **web_api 49 个文件复查完毕：模式 B 命中 2 处**（`model_root.rs`、`supervisor_slot.rs`），
且两处都被独立复核为「结构上不可能失败」。没有第三处。

## 关闭候选池 C-3
`dispatch()`（dispatch.rs:37-47，把 `allow_no_origin` 翻 true 的测试兼容层，`pub` + `#[allow(dead_code)]`）
**不是死代码**：本批读 `tests_dev_mode.rs` 见它被真实使用 6 次（:111 / :116 / :158 / :163 / :173 /
:207 / :211 / :227 / :229 / :234 / :238 / :244 / :261 / :263），`tests_p0c.rs` / `tests_reload.rs` 亦用。
`#[allow(dead_code)]` 的注释「非 cfg(test) 下未直接调用」**准确**。
⇒ **C-3 关闭**，「误接线导致全站 Origin 门禁失效」是纯理论风险，不构成发现。

## 维度覆盖（本批）
- 维度 C：`dev_mode` 三态 → 日志门控 → `app/status` DTO 字段的完整闭环有真测试（:221-271）
- 维度 J：web_api 全目录模式 B 复查**收口**（见上表）

## 未核实项
1. `tests_ws_client.rs` 未逐行读（helper 模块）。
2. web_api 内**未逐行读完**的清单（如实列出，供 Phase 2 维度横扫补）：
   `mod_registry.rs`（510-687/713-938/988-1362）、`cli_entry.rs`（1-109 / 180-320 / 387-469 / 534-603 / 644-769）、
   `chat_routes.rs`（-1-0 全部已读）、`handlers.rs`（330-427）、`dto.rs`（241-443）、
   `models_routes/dto.rs`（151-275）、`settings_routes/{mod,test_endpoints}.rs` 的剩余段、
   以及 `models_routes/tests_models_*.rs`（4 个大测试文件）与 `settings_routes/tests.rs` 的未读段。
3. **web_api 未 100% 逐行读完**，但**全部 49 个文件都至少被审计或结构枚举过一遍**。

## 本批新增
P0 0 · P1 0 · P2 0 · P3 1 ｜ 另：**候选池 C-3 关闭**
