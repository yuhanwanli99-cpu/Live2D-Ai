# 会话级作用域（L1 基座，2026-09-15）

> **状态**：本轮（L1 产品化波次）新增的**最小宿主能力**。它不改变主链皮肤
> （LLM/TTS/口型/Live2D 一行未动），只在「本轮请求用哪段 system_prompt」上
> 加了一层**按会话取值**的覆盖。
> **范围真源**：[PRODUCT-L1-GOALS-2026-09-15](../plans/PRODUCT-L1-GOALS-2026-09-15.md)。

---

## 1. 为什么需要它（问题是什么）

在它之前，产品的「人设」只有**一处**入口：`live2d-ai.toml` 的
`[persona] system_prompt`——**进程级、单例、全局**。

于是：

- persona Mod 只能**整段覆写**它 → 那是「全局人设」，**不是**「会话人设」。
  用户在会话 A 导入一张卡，切到会话 B，B 用上的还是 A 的卡（**串卡**）。
- memory Mod 的检索注入也只能写同一处 → 会话 A 记住的东西会在会话 B 被提起。

L1 验收要求 persona **绑定当前会话**（A/B 不串卡、停用还原），memory **尽量**
按会话分桶。两者都需要一个共同的东西：**会话 id**。

## 2. 最小宿主能力（做了什么）

| 层 | 落点 | 作用 |
| --- | --- | --- |
| 协议 | `POST /api/v1/chat` 新增可选 `session_id` | 这句用户输入属于哪个会话 |
| 新只读/写路由 | `GET|POST /api/v1/chat/session` | 读「宿主当前活动会话 + 已绑定会话列表」；写活动会话（前端切会话时同步，供 external/voice 注入跟随） |
| 宿主表 | `crates/live2d-ai-desktop/src/session_scope.rs` | `会话 id → system_prompt` 的**进程内**表 + 活动会话游标 |
| Mod 能力 | `ModServices.session_prompts`（`live2d_ai_mod_system::ModSessionPrompts`） | Mod 按会话写人设/记忆，不再动全局 toml |
| 事件 | `ModRuntime::on_scoped_event(topic, payload, session)` | TurnStarted / TurnPrompt / TurnEnded 带会话投递；**缺省实现原样转发**给 `on_event`，不关心会话的 Mod 一行不用改 |
| 引擎 | `ConversationEngine::set_system_prompt_override` | 本轮 system_prompt 的**唯一**决议点 |

### 2.1 一轮的时序（实际发生的事）

```text
POST /api/v1/chat {text, session_id}
  └► SupervisorHandle::say_scoped(text, session_id)
       ├─ 记下活动会话（供不带会话的注入路径跟随）
       └─ 入队 SayRequest{text, session}
  └► supervisor 出队（run_forever）
       ├─ engine.set_system_prompt_override(session_scopes.get(session))
       │     └─ 表里没有 → None → 回落全局 persona.system_prompt（降级语义）
       ├─ 发 Mod 事件 TurnStarted / TurnPrompt（**带 session**）
       └─ run_one_turn（请求体在这里构建，用的就是刚决议好的 system）
```

决议**每轮重做一次**（而不是切会话时改一次），因此：

- 热重载整体重建引擎后，覆盖槽自动回到 `None`，下一轮会重新决议；
- 会话表是「谁后写谁覆盖」，每轮重读保证看到最新值。

### 2.2 明确**不做**的事

- **不落 `live2d-ai.toml`**、**不触发 `supervisor.reload()`**：会话覆盖与全局
  配置是两套东西，互相不污染（这也是它能与既有 Mod 写回并存的前提）。
- **不做仲裁 / 合并**：同一会话里谁后写谁覆盖，与全局 `persona.system_prompt` 的
  既有口径逐字一致。宿主**不提供**优先级表——那等于在核心里埋第二个人设来源。
- **不解释内容**：宿主只存字符串；「是不是一张合法角色卡」由写入方（Mod）负责。
- **不持久化会话表**：它只活在进程内存里。要跨重启还在，由 Mod 自己落盘并在
  `start` 时灌回来（persona 就是这么做的：`persona-mod-cards.json`）。

## 3. 会话 id 的规则（**未来兼容闸**）

`live2d_ai_mod_system::sanitize_session_id(raw) -> Option<String>`：

- `trim` 后非空、长度 ≤ `MAX_SESSION_ID_CHARS`（128 **字符**）；
- 只允许 ASCII 字母数字与 `-` `_` `.` `:`。

**为什么现在就从紧**：会话 id 会被**用作文件名 / 存储键的一部分**——
memory 已经把桶落到 `sessions/<session>.memory.jsonl`。等真的落盘了再补闸，
就是在已经存在路径穿越风险的代码上事后打补丁。落盘方**必须**复用这个函数，
不要各写一份。

**不合法的 id 不是错误**：`POST /api/v1/chat` 收到非法 `session_id` 时按
「**不带会话**」处理（走全局桶），不返回 4xx。理由：会话 id 是前端本地生成的，
形状异常属于「客户端版本不匹配」，不该让用户发不出消息。

## 4. 降级语义（当前产品只有一个会话 / 调用方不带会话）

前端（Flutter）**本来就有多会话**（`ChatSessionStore`，上限 50，
localStorage 键 `live2d-ai.chat-sessions`），所以「会话 id」是既存事实；聊天发送
会自动带上当前会话 id。

但以下调用方**不带**会话 id，它们走 **`None` 桶**（= 全局 `persona.system_prompt`）：

| 调用方 | 行为 |
| --- | --- |
| 裸 `POST /api/v1/chat`（curl / 第三方客户端） | 全局桶 |
| `--chat` 终端壳 | 全局桶 |
| external-input（弹幕）/ voice-input（转写） | **跟随宿主活动会话**（前端切会话时会同步）；从未设过 → 全局桶 |
| memory（无会话时） | 落 `memory.jsonl`（旧路径），注入写全局 |

**UI 必须写明这一点**：persona / memory 面板在 `activeSessionId == null` 时要
显式说「还没有会话 → 会落到全局桶（与所有会话共享）」，**不能**假装已经绑好了。
这是 L1 明令的一条（「若当前产品只有单会话，须在 UI 写清降级语义」）。

## 5. 存储键的可扩展性

| 数据 | 键 | 桶规则 |
| --- | --- | --- |
| 会话记录（壳内） | localStorage `live2d-ai.chat-sessions` | 一份 JSON，内含 `sessions[]`（**已支持多会话**） |
| persona 卡 | `persona-mod-cards.json` | `{"version":1,"sessions":{"<id>":{…}}}`——加会话 = 加一个键，**不动格式** |
| memory 记忆 | `sessions/<session>.memory.jsonl` | 一个会话一个文件；无会话沿用旧的 `memory.jsonl` |

三处都以**会话 id**为键，且 id 已过同一道归一化闸——将来做成「服务端会话表」
不需要改键的形状，只需要换实现。

## 6. 相关文件

- 宿主表：[session_scope.rs](../../crates/live2d-ai-desktop/src/session_scope.rs)
- 能力契约：[session.rs](../../crates/live2d-ai-mod-system/src/session.rs)
- 路由：[chat_routes.rs](../../crates/live2d-ai-desktop/src/web_api/chat_routes.rs)
- 决议点：[supervisor.rs](../../crates/live2d-ai-desktop/src/supervisor.rs)（`run_forever` 出队处）
- 引擎覆盖槽：[engine.rs](../../crates/live2d-ai-runtime/src/conversation/engine.rs)
- 前端：`ApiClient.sendChat/setActiveSession`、`ChatController`、`ModPanelContext.activeSessionId`
