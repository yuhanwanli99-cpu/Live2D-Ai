# BATCH-0198 · 日志的**内容面**：主链**无文本**，但存在**两条经 Mod 通道**的候选路径（未闭）

Phase 1 · 域覆盖 · 日志出口（`log_routes.rs`）+ 全 workspace 生产 `tracing::` 穷举

## 跑的命令（全部只读）
```
grep -n "fn |limit|tail|read" web_api/log_routes.rs
grep -rn "tracing::(info|warn|error|debug)!" -A 2 crates/ --include=*.rs | grep -iE "text=|prompt=|content=|message=|assistant|user_text"
grep -rn "tracing::(info|warn|error|debug)!(" crates/ --include=*.rs | grep -v test | wc -l    # → 69
grep -rn "tracing::…" crates/ --include=*.rs | grep -v test | grep -E "\{\}|, *[a-z_]+ *[,)]"
```
未跑任何 cargo / flutter / pnpm 命令。

## ① 出口是**逐字转发**（所以内容面完全取决于写进去什么）
```rust
let (lines, truncated) = read_tail_lines(&file_path, MAX_LINES).unwrap_or_default();   // log_routes.rs:86
// 读失败 = 空（**不暴露 IO 错误给前端**）                                                          :86
```
⇒ **日志文件里有什么，前端就看到什么** ⇒ 加上 B0197 核的**可复制到剪贴板** ⇒
**内容面是红线 R 在「非密钥」维度上的出口**。

## ② ⭐ 主链**零文本**（两条 grep 各自验证）
- **结构化字段**：`text=|prompt=|content=|message=|assistant|user_text` 在全 workspace 的
  `tracing::` 调用里 **零命中**
- **生产 `tracing::` 共 69 处**，逐条看过带变量的那些：
```
env_routes.rs:150   key = %req.key, set, "env 写入成功（**值不记录**）"   ⇒ 只有键名（合 B0009）
dispatch.rs:250/252/254  %method, path, route, status                    ⇒ **无 body**（合 AGENTS「不记录请求体」）
supervisor.rs:458   %e, path                                            ⇒ 错误 + 路径
mod_registry.rs:377 mod_id, "…apply_settings 被拒绝"                      ⇒ 只有 id
```
⇒ **主链（LLM/TTS/WS/设置/环境）的日志只含标量与状态码，不含用户或助手文本。**

## ③ ⚠ 但存在**两条经 Mod 通道**的候选路径（**未闭**，精确记录）
```rust
mod_registry.rs:399  tracing::info!(target: "mod", mod_id, topic = topic.as_str(), **payload**, "Mod→host event");
mod_registry.rs:402  ModLogger::new(|_lvl, msg| tracing::info!(target: "mod", "{}", msg)),
```
⇒ **`:402` 是宿主提供给 Mod 的 logger** ⇒ **Mod 打印什么，就进宿主日志什么**
（与 B0103 核过的「Mod 侧日志**透传**进宿主」**同源**）
⇒ **`:399` 直接把 `payload` 落盘** ⇒ 而 `payload` 若是 `say_tx {text}` 之类，就含**用户文本**
⇒ ⇒ **若某个 Mod 打印了它收到的文本** ⇒ **用户内容进日志文件** ⇒
⇒ **诊断面板逐字展示** ⇒ **且可复制到剪贴板** ⇒ 这是一条**内容面**的出口。
**为何不记发现**（两条，逐条）：
1. **我未核任何 Mod 是否真的打印文本** ⇒ 「有 Mod 这么做」**未证实**；
2. **Mod 打印什么由 Mod 作者决定**，宿主无法在 logger 层做内容级脱敏
   ⇒ 若某个 Mod 该打印文本，那是**该 Mod 的问题**，不是这条宿主观律的缺陷。
⇒ **记为未核实项**（含上面两条 quote）：**逐个 Mod 检查其日志是否含用户/助手文本**。
⇒ ⭐ 而这条正好呼应 B0103 的结论：Mod 的日志**透传**是**设计**（便于排障），
**代价是宿主失去了对内容的控制** ⇒ **这是一次自觉的取舍，不是疏漏** —— 但**取舍要被写下来**。

## 未核实项
1. ⭐ **逐个 Mod 检查其日志是否含用户/助手文本**（`external-input` 处理用户输入 ⇒ **最可能**打印）
2. `logs` 的**采集点**（`tracing` 的 file sink 配置）未读 ⇒ 落盘路径与轮转未核
3. `dev_tools_section.dart` 其余实现未读（1874 行）
4. `tokens.dart`(928) 未读；`settings_controller.dart:120-222` / `:296-409` 未读
5. 新露出的 208 条自我设防名只看了 11 条
6. Mod crates 逐文件（约 53/61）· `shared/` · 根 `tests/`(3 py) · `verification/`
7. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
8. `join_endpoint` 回归断言体 · `mod_count_is_five` 断言体 · `_finishTurn` 是否幂等 ·
   `secrets.rs` 断言体 · B0120「chmod toml」未核
