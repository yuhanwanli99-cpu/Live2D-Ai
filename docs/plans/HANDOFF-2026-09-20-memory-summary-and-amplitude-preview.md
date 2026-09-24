# HANDOFF 2026-09-20 —— 记忆真摘要（P1-5 补齐）+ 动作幅度滑条即时预览

> 工作树：`/home/skystar/Live2D-Ai-l1`。**不 bump / 不 push / 不打 tag。**
> 范围真源：本波任务书两条——①补上 P1-5 的「真摘要」；②外观三滑条改草稿即预览。
> **未做**（任务书禁止）：导演 staging 重做、motion3 / exp3、本地 ASR、扩大主链、
> 改出厂默认幅度。

---

## 1. 做了什么

| # | 必做 | 结果 |
| --- | --- | --- |
| 1 | 触发：占用 >75% 且过冷却 → **非阻塞**跑摘要 | ✅ 判据改成**桶内原文总字符** / 注入预算 > `summary_ratio`（凭「本轮注入块占用」永远触发不了）；`std::thread::spawn` 后台线程跑 HTTP，主链只 `try_recv` 收已完成结果 |
| 2 | 输入：待压历史（排除最近 K=4 轮） | ✅ `summary::build_request`：`[covers_upto, n-keep)` 且按 8 000 字符从**最旧**侧截断（没喂进去的下轮再来） |
| 3 | 输出：短摘要 + 覆盖位点/版本号 | ✅ `SummaryVersion{version, covers_upto, covers_turn, created_at, text}` |
| 4 | 落盘：旁车可回滚；`state_json` 可见 | ✅ `x.memory.jsonl` → `x.memory.summary.json`（版本栈最新在前、原子 rename、上限 10）；`state_json.summary` 含 pending/version/covers_upto/last_error/text/… |
| 5 | 注入：五段里「摘要」段真进本 conversation | ✅ 打在本 Mod 的**同一会话槽**里（`- [摘要] …` 在原文命中之前）；无 conversation **不写全局** |
| 6 | LLM：OpenAI 兼容、独立配置、失败不写坏摘要 | ✅ `summary_http::OpenAiSummarizer`（reqwest blocking + rustls，密钥走 `secrets::lookup`）；超时/非 2xx/坏 JSON/只有思考 → `None` → **旁车 versions 一字不改**，只记 `last_error` |
| 7 | 面板：显示状态 + 「回滚上一版摘要」 | ✅ `memory_panel.dart` 新增「记忆摘要」块（版本/覆盖位点/桶占比/失败原因/正文预览）+ 回滚按钮（无版本则禁用） |
| 8 | 回归：触发/失败/回滚/A-B/无会话 | ✅ `summary_tests.rs` 10 条 + `tests.rs` 2 条改写 + `summary_store.rs` / `summary_http.rs` 11 条 |
| 9 | 滑条拖动即生效（防抖 ≤150–300ms） | ✅ `ActionScalesSyncer`（150ms 防抖 + 值去重 + 重建强发）由**设置控制器监听器**驱动——拖的是草稿也立刻下发 |
| 10 | 「保存」写盘；放弃/重载立刻与磁盘对齐 | ✅ 保存后用服务端回填的 view 重算有效值并下发；`discard()` / `load()` 同样触发（三者都 `notifyListeners`） |
| 11 | 与开发工具临时覆盖的优先级 | ✅ **未保存的产品拖动 = 当前预览真源**；产品值一经下发就冲掉临时覆盖；面板文案写死这条 |
| 12 | widget 测：改滑条触发下发；保存后 GET 为新值 | ✅ `action_scales_preview_test.dart` 12 条（纯函数 + 假时钟 + MockClient 保存回填） |

---

## 2. 摘要怎么触发 / 怎么回滚 / 怎么看状态

### 2.1 配好（三条）

```bash
# 1) settings_spec v2 的四个键（Mod 面板或 command 都可）
curl -s -X POST -H 'Content-Type: application/json' \
  -H 'Origin: http://127.0.0.1:18081' \
  -d '{"config":{
        "summary_enabled":true,
        "summary_base_url":"http://127.0.0.1:18181/v1",
        "summary_model":"mock-summary",
        "summary_api_key_env":"DEEPSEEK_API_KEY",
        "summary_timeout_ms":5000
      }}' \
  http://127.0.0.1:18081/api/v1/mods/memory/config
```

- 摘要是 **Mod 自己的** LLM 配置（独立于主链 `[llm]`）：Mod 拿到的是脱敏设置快照，
  里面既没有主链 base_url 也没有 key 变量名——所以**不自动继承**，要显式填；
- 想复用主链那把 key 就填**同一个变量名**；端点相同就把 base_url / model 抄进来。

### 2.2 看状态

```bash
curl -s http://127.0.0.1:18081/api/v1/mods/memory/state | python3 -m json.tool
# state.summary.client        = "openai"（没配齐 = "disabled" + note）
# state.summary.bucket_ratio  = 桶内原文 / 注入预算（触发判据，越过 0.75 才有戏）
# state.summary.version       = 0 → 还没有摘要；≥1 → 有
# state.summary.text          = 当前生效摘要正文
# state.summary.last_error    = 上次失败原因（成功清空）
# state.summary.pending       = 后台线程是否在跑
```

只想看摘要（不拉整份 state）：`POST /api/v1/mods/memory/command`
body `{"command":"summary","args":{"session_id":"…"}}` —— 与 `state.summary` 同源。

### 2.3 触发

连着聊到 `bucket_ratio > summary_ratio`（缺省 0.75）。想快点复现就把
`injection_budget_chars` 调到下限 **200**。日志锚点：

```text
memory 桶内原文 103% 越过摘要阈值：后台起摘要（覆盖前 4 条，保留最近 1 轮；不阻塞本轮）
memory 摘要 v1 已落盘（覆盖前 4 条原文，保留最近 1 轮原文；下一轮起进注入块）
```

失败时（离线 / 401 / 超时）：

```text
memory 摘要失败（摘要请求失败（client=openai, timeout_ms=5000））；本轮按无摘要处理（规则裁 top-k 仍生效）
```

### 2.4 回滚

```bash
curl -s -X POST -H 'Content-Type: application/json' \
  -H 'Origin: http://127.0.0.1:18081' \
  -d '{"command":"summary_rollback","args":{"session_id":"smoke-1"}}' \
  http://127.0.0.1:18081/api/v1/mods/memory/command
```

回滚 = **丢掉当前生效版**：`version` 退回 0（或上一版），下一轮注入不再含该摘要，
被它覆盖的原文**重新参与 top-k**。**原文一条没删**（`records` 不变）。
没有可回滚的版本 → `409 command_failed` + 「没有可回滚的版本」，不谎报成功。

面板入口：设置 → Mod → 本地记忆 → 「记忆摘要」→ 「回滚上一版摘要」。

---

## 3. 滑条：即时 vs 保存，各自怎么验

| 想验什么 | 怎么验 | 期望 |
| --- | --- | --- |
| **拖动即生效** | `flutter test test/action_scales_preview_test.dart` 的 `ActionScalesSyncer` 组（假时钟） | 20 次 `schedule` → 窗口内 **0 帧**，过 150ms → **1 帧**，值是最后一帧 |
| **值没变不重发** | 同文件 `值没变不重发` | 同一份 payload 连发两次 → 只有 1 帧（改 LLM base_url 之类不打扰渲染面） |
| **iframe 重建要补发** | 同文件 `force 绕过去重` + `onReady` 走 `_syncActionScalesNow(force: true)` | 第二次一定发出（新桥必须收到当前值） |
| **草稿优先 / 远端兜底** | `effectiveActionScales` 四条纯函数测试 | 草稿字段覆盖远端；没动的字段用磁盘值；都没有 → `null`（渲染面用出厂默认） |
| **保存才落盘** | `保存后才落盘` 组（MockClient） | 拖动后 `remote` 不变、没发 PATCH；`save()` 后 `remote=head 2.0`、草稿清空、有效值 2.0 |
| **放弃 / 重载与磁盘对齐** | `discard()` 测试 | `discard()` 后草稿 null、有效值回 `1.4`（舞台立刻回产品值） |
| **真机肉眼** | 设置 → 外观与互动 → 动作幅度，拖 head/body/expression | 舞台**立刻**变；点「保存」→ 成功提示；刷新页面仍是保存值 |

### 优先级（写清，别再猜）

1. **未保存的产品拖动** = 当前预览真源（`effectiveActionScales` = 草稿优先）；
2. 开发工具「临时幅度覆盖」**只**在没有任何产品值下发时有效——产品滑条一改，
   或保存成功把产品值下发，临时覆盖就被冲掉（面板文案已写死）；
3. 渲染面的出厂默认只在**设置还没加载**时用（前端此时**不发帧**，见 `null` 语义）。

### 为什么不能用「保存后重发」凑合

`dispatch_data` 的旧行为是「设置加载 / 保存后由 `main.dart` 下发 `actionScales`」。
拖动滑条**只改草稿**，不触发任何重建路径 → 舞台毫无反应，用户看到的就是
「要先保存才生效」。现在改成**设置控制器监听器 + 防抖**：拖草稿、保存成功、
放弃草稿、重新加载四条路共用同一个下发出口。

---

## 4. 诚实标注

1. **摘要走的是「记忆桶」而不是「LLM 多轮 messages」**：Mod 拿不到对话的 messages，
   只能看到每轮用户话。所以摘要是「用户说过什么的要点」，不含助手回复。要含助手
   回复得先让核心层把对话暴露给 Mod——那是扩主链，本波不做；
2. **摘要是累积式而非滚动式**（文档 §11.7）：新版只压「尚未覆盖的原文」，所以
   上一版的正文会被新版本替换（`versions` 栈里还能翻到，但注入只用最新版）；
3. **旁车文件不随「停用 Mod」删除**：停用只清会话注入槽；要连摘要一起清就用
   `command("clear")`（它已经会连带删旁车）；
4. **摘要线程不做取消**：Mod 停用时已在跑的那次请求会跑完，但结果因
   `shutdown` 后不再收（`summary_rx` 随 runtime 一起 drop）→ 不会写盘。
   代价是一次多余的 HTTP；不引取消机制是刻意的（复杂度换不回可观察的行为）；
5. **wasm 无改动**：`l2d-wasm-demo` 一行未改，`dist/` 未重建（渲染面不认 `actionScales`
   之外的任何新东西）——本波没有需要重建 wasm 的改动。
6. **Flutter 产物未重新构建**：`shell/flutter/build/web` 是旧产物，本波的 Dart 改动
   **没有**进产物（按任务书只做实现 + 门禁；要肉眼验收需 `flutter build web`）。
---

## 5. 活服务冒烟（本机实测，mock LLM，离线可跑）

用 `python3 /tmp/l1-mock-llm.py`（18181，认「压缩」二字决定回摘要还是回正文）+
`./scripts/ignite.sh --port 18081`（与 18080 上正在跑的开发服务隔离，只共用仓库根的
`live2d-ai.toml` / `mods.json`，记忆库指到 `/tmp`）。

```text
# 配置：summary_enabled=true，base_url=http://127.0.0.1:18181/v1，
#       summary_ratio=0.5，summary_keep_recent_turns=1，injection_budget_chars=200
# 7 轮（session_id=smoke-2）
200 200 200 200 200 200 200

state.summary.version       = 1
state.summary.covers_upto   = 4
state.summary.bucket_ratio  = 0.95

# 第 7 轮注入块（模型请求体原文）——摘要段在最前、前缀**恰好一次**
<!-- live2d-ai:memory-v0:begin -->
- [摘要] 用户自称星梦，住在海边；喜欢薄荷与薄荷茶；养了一只叫煤球的猫；与助手约定周末去爬山。
<!-- live2d-ai:memory-v0:end -->

# 摘要请求体：只含未覆盖的原文，**不含最近 K 轮**
… - 我最近在读一本关于海洋生物的书，里面讲到深海发光的鱼

# 旁车文件 /tmp/l1-mem2/sessions/smoke-2.memory.summary.json
{"versions":[{"version":1,"covers_upto":4,"covers_turn":4,"text":"…"}],"generated":1}

# 回滚（POST command summary_rollback）
HTTP 200 → {"ok":true,"result":{"rolled_back":{"version":1,…},"version":0,"text":null,…}}
```

`ignite.sh --check` 四项全 ok（`/ → 302 /app/`、`/app/ → 200`、两份产物不引 gstatic）。

**冒烟当场抓到一个真缺陷并已修**：注入块里出现过 `- [摘要] [摘要] …`。根因是
`inject_into_session` 自己拼了一次前缀、`summary::compose_lines` 又拼一次。
已删掉调用方那一次（前缀只在 `compose_lines` 一处加），并补回归
`true_summary_...` 断言前缀**恰好一次**——把双重拼接放回去，该回归**实测变红**。

**冒烟没覆盖到的**：真机肉眼看摘要是否影响了模型的回答（主链是 mock，回答固定）。
这里验的是**模型请求体原文**（比截图硬），但那不等于「模型真的用上了它」。

---

## 6. 门禁（本机实测）

| 检查 | 结果 |
| --- | --- |
| `cargo test --workspace --all-targets` | **1313 passed / 0 failed** |
| `cargo test --doc --workspace` | 3 passed / 0 failed |
| `cargo fmt --all -- --check` | clean |
| `cargo clippy --workspace --all-targets -- -D warnings` | **0 warning** |
| `cargo run -p xtask -- rust-ratio` | **96.9785% PASS** |
| `cargo test -p live2d-ai-mod-memory` | **117 passed / 0 failed**（本波 +23） |
| `flutter analyze` | No issues found |
| `flutter test` | **1001 passed**（本波 +12：`action_scales_preview_test.dart`；`memory_panel_test` 29） |
| `./scripts/ignite.sh --port 18081` + `--check` | 四项全 `[ok]`；活服务冒烟见 §5 |

## 7. 改了哪些文件

**Rust（`live2d-ai-mod-memory`）**

- `src/summary_store.rs`（**新增**：旁车版本栈 / 原子写 / 回滚 + 5 条回归）
- `src/summary.rs`（**新增**：触发判据的纯逻辑 / 切片 / 归一化 / 响应解析 / 客户端 trait）
- `src/summary_http.rs`（**新增**：OpenAI 兼容客户端 + loopback mock 4 条回归）
- `src/summary_flow.rs`（**新增**：编排 / 后台线程 / 收结果 / 回滚 / `summary_state`）
- `src/summary_tests.rs`（**新增**：11 条端到端回归，含 A/B 不串与无会话不污染）
- `src/lib.rs`（注入带摘要、检索跳过已覆盖原文、`state_json.summary`、factory 装配）
- `src/config.rs`（5 个 `summary_*` 配置 + `settings_spec` v2 + `summary_config()`）
- `src/commands.rs`（`summary` / `summary_rollback`；`clear` 连带删旁车）
- `src/tests.rs`（规格键表更新 + 两条摘要相关回归改写/新增）
- `Cargo.toml`（+ serde / reqwest blocking / live2d-ai-runtime）

**Flutter**

- `lib/live2d/action_scales_sync.dart`（**新增**：防抖下发器 `ActionScalesSyncer`）
- `lib/api/settings_models.dart`（**新增** `effectiveActionScales`）
- `lib/live2d/live2d_stage.dart`（**新增** `applyActionScales`）
- `lib/app/shell_prefs.dart`（设置监听驱动的防抖下发 + `onReady` 强制补发）
- `lib/main.dart`（监听器接线 + `dispose` + 去掉静态 `actionScales` 快照）
- `lib/settings/sections/appearance_section.dart`（文案：拖即生效 / 保存才落盘）
- `lib/settings/sections/dev_tools_section.dart`（文案：临时覆盖的优先级）
- `lib/settings/mods/memory_panel.dart`（摘要状态块 + 回滚按钮 + 纯函数）
- `test/action_scales_preview_test.dart`（**新增** 12 条）
- `test/memory_panel_test.dart`（+5 条摘要回归）
- `pubspec.yaml`（dev_dependency `fake_async`——防抖行为用假时钟测，不 sleep）

**文档**

- `docs/architecture/memory-mod-v0.md`（状态头 + §3 配置表 + §9 `summary` 子对象 +
  §10 门禁数字 + §11 已知缺口 + **§15 真摘要**）
- `docs/plans/HANDOFF-2026-09-20-memory-summary-and-amplitude-preview.md`（本文）

---

## 8. 诚实未完项

1. **摘要不含助手回复**（15.1 / §4.1）：Mod 看不到 messages，只能压用户话；要含助手
   回复得扩主链，本波不做；
2. **真机肉眼验收待做**：Windows 浏览器拖三滑条看舞台、看摘要在真实 LLM 下是否
   改变回答——本机只能给到「请求体原文 + 单测」；
3. **Flutter 产物未重建**：`shell/flutter/build/web` 仍是旧产物，本波 Dart 改动**不在**
   产物里；要在 18080 的界面上手验，先 `flutter build web --release --base-href /app/
   --no-web-resources-cdn`；
4. **摘要线程不可取消**（§4.4）：停用时在飞的那次请求会跑完（结果不写盘）；
5. **`summary` / `summary_rollback` 还没有 host 侧的白名单**：命令走的是通用
   `POST /api/v1/mods/{id}/command`（未知命令由 Mod 自己回 409），所以**不需要**
   改 host——这一点在活服务冒烟里验过（回滚 200）。

