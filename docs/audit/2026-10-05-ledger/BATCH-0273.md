# BATCH-0273 · 🔻 **F-0020-01（P1）撤回** —— Phase 4 第一次**推翻**（不是措辞问题，是前提错了）

Phase 4 · **证伪 / 自相矛盾** —— 攻第七条 P1

## 跑的命令（全部只读）
```
sed -n '1,14p' crates/live2d-ai-runtime/src/performance/client.rs
grep -rn "MAX_TOKENS" crates/ --include=*.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**撤回 F-0020-01（P1）** —— **审计第三次撤回**，也是 Phase 4 的第一次推翻
### ① 攻法
若那个 `MAX_TOKENS` **就是主链的预算** ⇒ 我「1024 vs 4096 ⇒ 常量错」的批评成立
⇒ ⇒ **反之若它是独立的一路 ⇒ 我的前提就错了**
### ② ⭐ 攻击成立：它**显式声明独立**
```
performance/client.rs:1  「**表演层** HTTP 客户端：**非流式** POST {base_url}/chat/completions」
performance/client.rs:4-6 「**独立配置**：base_url / model / timeout_ms / api_key_env ——
                           **独立于 `[llm]`（主模型）**；**二路断了不影响主链**」
settings.rs:187         pub const DEFAULT_MAX_TOKENS: u32 = 4096;   ← **主链的，另有其人**
```
⇒ ⇒ **我原来的框架前提不成立** ⇒ ⇒ **撤回**

### ③ ⭐ 而「一场调用一个预算」**正是这一层的既有设计**（同族三处）
| 调用点 | 预算 |
|---|---|
| `director/staging_http.rs:48` | `512` |
| `memory/summary.rs:54` | `512` |
| `performance/client.rs:139` | `1_024` |
⇒ 而这一路返回的是**一份 JSON 动作计划**（`:1`「表演层要的是一份 JSON，**不是逐字流**」）
⇒ ⇒ **比对话小是合理的** ⇒ ⇒ **我没有任何证据说 1024 不合适**
⇒ ⇒ 按我自己的规矩（**拿不出证据的就不进 FINDINGS**）⇒ **撤回**

### ④ ⚠ 而「若日后要重启这条」需要的**不是**与主链比较
需要的是「**一份动作计划 JSON 到底需要多少输出 token**」这一条证据（真实动作计划的 token 长度）
⇒ ⇒ **在仓库里拿不到**（禁止运行、也没有样本）⇒ **故不列为 P1/P2/P3**

## 未核实项
1. **Phase 4 最后一条 P1**：F-0046-01（wasm `origin`）—— **其世界状态前提也要重核**（B0271 立的规则）
2. 真实动作计划的 token 长度（**仓库里拿不到** ⇒ 永久敞口，**不是 P**）
3. 那四个「未知 id」在文档里有没有被点名（B0270 敞口）
4. 能否点名会在持锁期间 panic 的 Mod（B0268 敞口）· `refresh_from_disk` 的文件变更路径（B0266 敞口）
5. 面板余面：`dev_tools_section` ~1845 · `live2d_stage` ~600 · `memory_panel` ~545 ·
   `persona_panel` ~520 · `message_bubble` ~470 · `chat_panel` ~465 · `error_banner` ~15
6. 其余 6 处 `mod_count_is_*` 逐条对账未核 · `ErrorResponse` 定义未读
7. `onReady` 是否间接清过 `presetStatus`（F-0263-01 反证 (a)，**结论不依赖它**）
8. **两侧措辞同步无机制**（B0260 敞口）· `chat_session.dart` 的 `maxSessions=50` 施加点未读
9. 前端 `.dart` 仍 181 未读（**机械值**）；Mod crates 42 未读；`mod-system` 余 8 文件未读
10. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
11. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
12. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
