# BATCH-0166 · `post_once`：**超时在使用点钳位**，但**响应体读取无上限**（记 KL-3，不记发现）

Phase 1 · 域覆盖 · `performance/client.rs`（`post_once` 主体）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/performance/client.rs` — 652（定点 276-296 `post_once` 全段）

## 跑的命令（全部只读）
```
grep -n "fn post_once" -A 40 performance/client.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；**一处候选池（KL-3）** + **一处结构观察**
### ① 四个可核性质（通过）
- **超时在使用点钳位**（:283）`timeout_ms.clamp(MIN_TIMEOUT_MS, MAX_TIMEOUT_MS)`
  ⇒ 与 B0134 的 `tts_queue_capacity.max(1)`、B0122 的「合并后再校验」**同形状**：不指望配置给合法值；
  且是**每请求各自**的 ⇒ 降级路径的第二次请求**有自己的完整超时**（总上界 2×）
- **key 只进头**（:289-291）`bearer_auth(key.expose_secret())` ⇒ B0153 的机制
- **两个失败点都是 `.ok()?`**（:292 发送 · :294 读体）⇒ **无 panic 路径**，一律回落
- **返回 `(bool, String)`**（:282）—— 而 :343 的头注正说明「只拿得到『成功与否 + 正文』」

### ② ⭐ 结构观察：**类型签名是「启发式」的上游原因**
`post_once` 返回 `(bool, String)` ⇒ **状态码没被传出** ⇒ 上层的 `client_error_4xx`（B0164）
**不得不去猜**（嗅探响应体）。
⇒ **把状态码传出来，启发式就不必存在** —— 也就是说：
> **一个函数的返回类型，会决定上一层需不需要写启发式。**
⇒ **正面模式 P13（提示侧↔强制侧同句）的镜像**：这里缺的不是注释，是**一个类型**。

### ③ ⚠ 候选池 **KL-3**：成功路径的**响应体读取无上限**
:293-294 拿到了 `status` 并用了它，但 `:294` `response.text().await.ok()?` **把整份 body 读进内存**，
**没有尺寸上限**。而本仓在别处**处处有界**：
`MAX_WAV_BYTES`（B0121）· `MAX_SNIPPET_CHARS`（B0160，**恰好是错误路径**上的同一类片段）·
`MAX_SEGMENT_CHARS` / `MAX_SPEAK_CHARS`（B0127）· `MAX_RECORDS`（B0117）· 注入预算（B0104/B0113）
⇒ **这里是不设界的那一处**（且是**成功路径**；错误路径的片段是截断过的）。
**为什么不记发现**（两条，逐条）：
1. **危害上界无法从源码确定** —— `.timeout()` 在当前 reqwest 版本里**是否覆盖 body 读取**，
   **取决于库版本的语义**，而**我禁止运行任何构建/测试** ⇒ 读不出来；
   最坏情况按注释推测是「超时后回落」＝**降级而非崩溃**。
2. **触发需要用户把 `base_url` 指到一个会返回/流式吐大量数据的地方**
   （如 CDN、大文件、被劫持的中间层）⇒ 无远程默认路径。
⇒ **建议（若日后确认 reqwest 的 `timeout` 覆盖 body）**：`text()` 前先按
`Content-Length` 拒绝超限，或改用 `.take(n)` 形态的限流读取 —— **成本一行**。

## 未核实项
1. `client.rs` 余 ~430 行未读（wire 回归的**断言体**、`build_body`、`prompt.rs` 余段）
2. **`reqwest` 当前版本里 `.timeout()` 是否覆盖 body 读取** —— **未核，且禁止运行**（KL-3 的前提）
3. `plan.rs` 余 ~560 行未读（`message()` 逐一、`action_cue_payload`(851)、自测）
4. `mod.rs` 余 ~350 行未读（`PerformanceStats` 对外暴露面、Budget 注入点）
5. `llm.rs` 余 ~200 行 · `sse.rs` 余 ~200 行 · `config.rs:51-140` 未读
6. `runtime/src/lib.rs` 的 `join_endpoint` 回归（:184）断言体未读
7. `_finishTurn` 是否幂等 · `secrets.rs` 10 条测试断言体未读 · B0120「chmod toml」未核
