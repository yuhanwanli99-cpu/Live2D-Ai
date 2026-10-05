# BATCH-0275 · 🔻 **F-0046-01（P1）撤回** —— 我的**发现与修法**都锚在一个已不存在的树版本上

Phase 4 · **证伪 / 自相矛盾** —— 兑现 B0274 的承诺（对 `web/surface/input.rs` 重核）

## 跑的命令（全部只读）
```
wc -l web/surface/input.rs ; grep -nE "postMessage|origin|fetch_bytes" web/surface/input.rs
grep -rn "postMessage" crates/l2d-wasm-demo/src/ --include=*.rs
grep -rn "postMessage" --include=*.rs --include=*.dart crates/ shell/flutter/lib/
sed -n '70,84p' shell/flutter/lib/live2d/live2d_host_web.dart ; git ls-files crates/l2d-wasm-demo/dist
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**撤回 F-0046-01（P1）** —— **审计第 4 次撤回**，**Phase 4 第 2 次**
### ① 三次 grep 追到源头
| 核验 | 结果 |
|---|---|
| `input.rs` 里的 `postMessage` / `origin` / `fetch_bytes` | **只有两条注释**（`:43` `:61`） |
| **wasm 源码树** `postMessage` **真实调用** | **0** |
| `crates/l2d-wasm-demo/dist/*.js` | **未跟踪**（`git ls-files` 空）⇒ **过期构建产物**，按排除清单本就不审 |

### ② ⭐ 而**幸存的那一侧用的是具体 origin，不是 `"*"`**
```dart
// shell/flutter/lib/live2d/live2d_host_web.dart:78
target.postMessage(json.toJS, **web.window.location.origin.toJS**);
```
⇒ ⇒ **我引用的 `main.rs:380-386`（无 origin 检查）与 `main.rs:705-707`（`parent.postMessage(&ack,"*")`）
在当前树上都不存在**（文件被拆过 —— AGENTS rc.3 ②「`surface.rs` 1167→**24**」）

### ③ ⭐ 而**我那条修法 ③ 建议调用的 `normalize_resource_ref`，在 wasm 树里也已是 0 命中**
⇒ ⇒ **我的发现与我的修法都锚在一个已经不存在的树版本上** ⇒ **永久撤回**
（这与 **F-0040-01** 同一判据：**对象的「对象」被改掉了**）

### ④ ⚠ 而**必须不夸大**
**本条被撤回，是因为它的对象不存在了 —— 不是因为问题被证明不存在。**
**留下的不是「已修复」，而是「这一版没有这条通道」。**
⇒ 若将来 iframe 通道**重建**且再次用通配 `targetOrigin`，**同类风险会重新出现**。

### ⑤ ⭐ 而 B0274 的那条规则**连着两天各生效一次**，形态不同
| 批 | 拦住了什么 |
|---|---|
| B0271 / B0274 | ① **凭记忆**（把 273 批前的「不存在」当今天的事实）② **凭零命中**（把「没找到」当「不存在」） |
| **本批** | ③ **凭一次 grep** —— **顺着变化的痕迹追了三步**才定案：注释 → 整树 → 跨 crate → 产物 |
⇒ ⇒ ⇒ **三条对策合成一句**：**不要在原地下结论，要顺着痕迹往外追，追到能定性为止。**

## 未核实项
1. `web/surface/{render,idle}.rs` · `web/gpu.rs` · `main.rs` 未读（**Phase 4 攻完之后回读**）
2. 真实动作计划的 token 长度（F-0020-01 撤回的永久敞口）· `shell/flutter` 是否真可能出现密钥
3. 那四个「未知 id」在文档里有没有被点名（B0270 敞口）· `refresh_from_disk` 敞口（B0266）
4. 能否点名会在持锁期间 panic 的 Mod（B0268 敞口）
5. 面板余面：`dev_tools_section` ~1845 · `live2d_stage` ~600 · `memory_panel` ~545 ·
   `persona_panel` ~520 · `message_bubble` ~470 · `chat_panel` ~465 · `error_banner` ~15
6. 其余 6 处 `mod_count_is_*` 逐条对账未核 · `ErrorResponse` 定义未读
7. `onReady` 是否间接清过 `presetStatus`（F-0263-01 反证 (a)，**结论不依赖它**）
8. **两侧措辞同步无机制**（B0260 敞口）· `chat_session.dart` 的 `maxSessions=50` 施加点未读
9. 前端 `.dart` 仍 181 未读（**机械值**）；Mod crates 42 未读；`mod-system` 余 8 文件未读
10. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
11. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
12. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
