# BATCH-0274 · ⚠ **F-0046-01 的前提「移动了」** ⇒ **我既不能说它成立、也不能说它不成立**（须重核）

Phase 4 · **证伪 / 自相矛盾** —— 攻第八条（最后一条）P1，**先按 B0271 的规则重核前提**

## 跑的命令（全部只读）
```
grep -rc 'ev.origin()' crates/l2d-wasm-demo/src/main.rs
grep -rc 'postMessage(&ack,"*")' main.rs ; grep -rn "postMessage" main.rs
grep -rln "postMessage" crates/l2d-wasm-demo/src/ ; grep -rn "postMessage" crates/l2d-wasm-demo/src/
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**0 条新发现**；**一条必须如实记录的「前提已移动」**
### ① 三次 grep 全零 —— 但**第三个零最有信息量**
| 核验 | 结果 |
|---|---|
| `ev.origin()`（`main.rs`） | **0** |
| `postMessage(&ack,"*")`（`main.rs`） | **0** |
| **`postMessage` 本身（`main.rs`）** | **0** ← ⭐ **连这个词都不在 `main.rs` 里了** |

⇒ ⇒ 而全目录搜 `postMessage` ⇒ 只命中 **`web/surface/input.rs`** 的两处**注释**：
```
input.rs:43  /// **任何能 postMessage 到它的东西都能** …      ← 正是本条描述的威胁，被写成了已知属性
input.rs:61  /// iframe bridge 接收状态（P0-2a）：**父** …
```

### ② ⇒ **所以我的前提「移动了」**
我引用的 `main.rs:380-386`（无 `origin` 检查）与 `main.rs:705-707`（`parent.postMessage(&ack,"*")`）
**在当前树上都不存在**
⇒ **原因清楚**：AGENTS rc.3 ② 记过「`surface.rs` 1167→**24**（render/input/idle/gpu）」⇒ **文件被拆过**
⇒ ⇒ **ack 的 postMessage 已从 `main.rs` 移进 `web/surface/input.rs`**

### ③ ⚠ 而**本批的真正价值恰恰是「不下结论」**
三次 grep 全零 ⇒ **若我照惯例写「前提重核、攻击失败、仍然成立」，**那会是一条**我根本没核过的话**
（那句「三次全零」说明的是**代码移走了**，**不是**「那里没有 origin 检查」）
⇒ ⇒ **B0271 立的那条规则**（「关于世界的事实会过期；前提必须重核，**不能靠记忆**」）
**第二次拦住了我** —— 第一次拦住的是「把 273 批前的记忆当今天的事实」，
⇒ **这一次拦住的是「把『没找到』当成『不存在』」**
⇒ ⇒ ⭐ **两条是不同的失效**：① **凭记忆** ② **凭零命中** ⇒ **共用的对策是：顺着变化的痕迹去找，而不是在原地再搜一次**

## 未核实项
1. ⭐ **F-0046-01 必须对着 `web/surface/input.rs` 重核**（`origin` 检查是否在那里、`postMessage` 的
   targetOrigin 现在是什么）⇒ **本批明确未做** ⇒ **在做完之前，不得声称本条成立或不成立**
2. `web/surface/{render,idle}.rs` · `web/gpu.rs` 未读
3. 真实动作计划的 token 长度（F-0020-01 撤回的永久敞口）· `shell/flutter` 是否真可能出现密钥
4. 那四个「未知 id」在文档里有没有被点名（B0270 敞口）· `refresh_from_disk` 敞口（B0266）
5. 能否点名会在持锁期间 panic 的 Mod（B0268 敞口）
6. 面板余面：`dev_tools_section` ~1845 · `live2d_stage` ~600 · `memory_panel` ~545 ·
   `persona_panel` ~520 · `message_bubble` ~470 · `chat_panel` ~465 · `error_banner` ~15
7. 其余 6 处 `mod_count_is_*` 逐条对账未核 · `ErrorResponse` 定义未读
8. `onReady` 是否间接清过 `presetStatus`（F-0263-01 反证 (a)，**结论不依赖它**）
9. **两侧措辞同步无机制**（B0260 敞口）· `chat_session.dart` 的 `maxSessions=50` 施加点未读
10. 前端 `.dart` 仍 181 未读（**机械值**）；Mod crates 42 未读；`mod-system` 余 8 文件未读
11. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
12. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
13. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
