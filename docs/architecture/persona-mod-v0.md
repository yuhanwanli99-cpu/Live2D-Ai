# 角色卡 Mod v0（`live2d-ai-mod-persona`）

> **状态**：2026-09-15 **L1 产品级 · persona 轨**更新——**会话绑定**：
> `import_card` / `clear_import` 接受可选 `session_id`，卡按会话绑定
>（`persona-mod-cards.json`），换会话不串、回原会话仍在、停用清空宿主会话表；
> 不带 `session_id` 时仍是本文档前半部分的**全局**语义。见 §8。
> 上一版：2026-09-14 **产品级加强波次 · persona 轨**更新——新增两条一次性命令
>（`import_card` / `clear_import`）与 `state_json`（`GET /mods/persona/state`
> **从恒 503 升为 200**），面板补齐导入 UI、失败提示，以及与 memory 共存的说明。
> 上一版：rc.4 M5（`docs/releases/v0.1.0-rc.4.md`）把酒馆角色卡从主链抽成这条 Mod。
> 通用契约：[mod-product-chain.md](mod-product-chain.md)；
> 与记忆的边界：[memory-mod-v0.md](memory-mod-v0.md) §5.1；
> 代码：`crates/live2d-ai-mod-persona/`、`shell/flutter/lib/settings/mods/persona_panel.dart`。

## 1. 它做什么（一句话）

**把一张酒馆角色卡变成对话的人设**——默认绑定**当前会话**（L1），也可以
写入全局 `persona.system_prompt`；停用时**会话表清空 + 主链还原基线**。
卡只认两种载体：SillyTavern V1/V2 JSON，或内嵌 `chara` 文本块（base64 JSON）的 PNG。
**零网络**：解析、落盘、写回全在本机。

主链侧只有一处 system 入口（`live2d-ai-runtime` 的 `ConversationConfig`）——
这条 Mod 不新开对话通道，与 memory 是同一条纪律。

## 2. 卡从哪儿来（三条来源，优先级从高到低）

| 来源 | 谁写的 | 说明 |
| --- | --- | --- |
| `imported`：`persona-mod-card.json` | 本界面「导入角色卡」 | 与 `live2d-ai.toml` **同目录**；**优先级最高** |
| `config_json`：配置项 `card_json` | 「Mod 管理」的配置表单 / `POST /mods/persona/config` | 粘贴的卡 JSON 文本 |
| `config_path`：配置项 `card_path` | 同上 | 磁盘上的 `.json` / `.png` 路径 |
| `overrides` | 配置项 `name` / `description` / `personality` / `scenario` | 非空字段**压过卡里的同名字段** |

**为什么导入卡不写回 config**：`config` 住在 host 的 `mods.json` 里，原子写回的唯一入口
在 host 侧（`mod_registry::persist_manifest`），runtime 没有写自己 config 的通道；硬去改
`mods.json` 就是在 host 之外开第二个真源。所以导入卡落成 Mod 自己的状态文件，
**优先级高于** config 的就地替换语义——「导入」是一次显式动作，它必须能压过配置文件。
要回到配置里的卡，用 `clear_import`（面板上的「清除导入卡」）。

导入时会**规范化**成一张 V2 JSON（PNG 卡也落成 JSON）：于是「从 PNG 导入」与
「从 JSON 导入」之后的行为完全一致，落盘文件本身也可读、可再导入。

上面的优先级是**全局作用域**的；L1 起还有**会话作用域**——导入到某个会话的卡
住 `persona-mod-cards.json`，**不进这张表、不参与全局主链的接管**，见 §8。

## 3. 导入 → 启用 → 人设变 → 停用还原（逐步可复制）

下面每一条都能直接贴到 WSL2 的终端（服务已由 `./scripts/ignite.sh` 起在 `18080`）。
**mutating 请求必须带 `Origin`**（loopback 白名单）与 `Content-Type`，
否则会被 security 层回 403 `origin_required` / `bad_content_type`——
`curl` 不会自己带 `Origin`，所以下面每条 POST 都显式给了。
（工具脚本想省掉 `Origin`：启动时设 `LIVE2D_AI_ALLOW_NO_ORIGIN=1`。）

~~~bash
BASE=http://127.0.0.1:18080
ORIGIN='Origin: http://127.0.0.1:18080'
JSON='Content-Type: application/json'
~~~

### 3.1 先看当前状态（未启用时 503 是**预期**）

~~~bash
curl -s "$BASE/api/v1/mods" | python3 -m json.tool | grep -A4 '"persona"'
~~~

期望：`persona` 一行 `enabled=false, status=disabled`，且**带** `settings_spec`
（未启用也能拿到表单——先填卡、再启用）。

~~~bash
curl -s "$BASE/api/v1/mods/persona/state"
~~~

期望（未启用）：`503` + `{"error":{"code":"state_unavailable",...}}`。
**这不是缺陷**：没有 runtime 实例，命令通道同理。

### 3.2 打开开关（先启用、后导入）

~~~bash
curl -s -X POST "$BASE/api/v1/mods/persona/enable" -H "$ORIGIN"
~~~

期望：`200 {"ok":true,...}`。此时若 config 里没有卡，是**合法 no-op**：
主链 `persona.system_prompt` 一字不动（先启用、后填卡是正常流程）。

### 3.3 导入一张卡（界面同款命令）

~~~bash
curl -s -X POST "$BASE/api/v1/mods/persona/command" -H "$ORIGIN" -H "$JSON" \
  -d '{"command":"import_card","args":{"card_json":"{\"spec\":\"chara_card_v2\",\"data\":{\"name\":\"NEKO\",\"description\":\"猫娘\"}}"}}'
~~~

期望：`200 {"ok":true,"result":{...}}`，其中
`card_source:"imported"`、`card_name:"NEKO"`、`card_format:"v2"`、`bytes` > 0。
**导入即生效**：命令内部先落盘、再写回主链，无需再启停一次。

从 PNG 导入同一张卡（面板上的「选择 PNG 角色卡文件」走的就是这条）：

~~~bash
B64=$(base64 -w0 card.png)
curl -s -X POST "$BASE/api/v1/mods/persona/command" -H "$ORIGIN" -H "$JSON" \
  -d "{\"command\":\"import_card\",\"args\":{\"data_base64\":\"$B64\"}}"
~~~

`data_base64` 也接受浏览器 `readAsDataURL` 的原样字符串
（`data:image/png;base64,…`，前缀由 Mod 剥掉）。两者都给时 `card_json` 优先。

**按会话导入**（面板默认那条）只是多传一个 `session_id`——它**不写主链**，
落的是 `persona-mod-cards.json` + 宿主会话表（见 §8）：

~~~bash
curl -s -X POST "$BASE/api/v1/mods/persona/command" -H "$ORIGIN" -H "$JSON" \
  -d '{"command":"import_card","args":{"card_json":"{\"spec\":\"chara_card_v2\",\"data\":{\"name\":\"NEKO\"}}","session_id":"1757890123456789-3"}}'
~~~

期望返回里 `scope:"session"`、`session_id` 原样回显；`GET /api/v1/settings` 的
`persona.system_prompt` **不变**，而那个会话的下一轮对话已经用上这张卡。
清除它：`{"command":"clear_import","args":{"session_id":"1757890123456789-3"}}`。

### 3.4 看运行态（本轨升级为 200）

~~~bash
curl -s "$BASE/api/v1/mods/persona/state"
~~~

期望：`200` + 一份**脱敏**快照（形状由回归钉死）：

~~~json
{"id":"persona","enabled":true,"state":{
  "ready":true, "card_source":"imported", "card_name":"NEKO", "card_format":"v2",
  "applied_chars":12, "has_base_snapshot":true,
  "include_discipline":true, "say_first_mes":false, "config_has_card":false}}
~~~

字段语义：`ready` = 已把一张可用人设写进主链；`card_source` ∈
`imported`/`config_json`/`config_path`/`overrides`/`none`；
`applied_chars` 只是**长度**，**不含卡正文**。

### 3.5 确认人设真的变了

~~~bash
curl -s "$BASE/api/v1/settings" | python3 -c 'import json,sys; print(json.load(sys.stdin)["persona"]["system_prompt"])'
~~~

期望：开头是 `[角色] NEKO`，后面跟着描述、以及（缺省打开的）`【对话纪律】` 模板。
在界面上对一句话提问，回答应当按这张卡的人设走。

### 3.6 停用 = 还原基线

~~~bash
curl -s -X POST "$BASE/api/v1/mods/persona/disable" -H "$ORIGIN"
curl -s "$BASE/api/v1/settings" | python3 -c 'import json,sys; print(json.load(sys.stdin)["persona"]["system_prompt"])'
~~~

期望：`system_prompt` 回到**首次成功接管那一刻**的快照。该快照落在与
`live2d-ai.toml` 同目录的 `persona-mod-base.txt`，只在首次接管成功后写一次——
所以「启用 → 停用」来回几次都不会把基线污染成上一条人设，
期间用户手改的主链提示词也不会被停用动作吃掉（第二次启停仍以磁盘快照为锚）。

### 3.7 清除导入卡（回到 config 的卡 / 回到基线）

~~~bash
curl -s -X POST "$BASE/api/v1/mods/persona/command" -H "$ORIGIN" -H "$JSON" \
  -d '{"command":"clear_import","args":{}}'
~~~

期望：`200` + `{"ok":true,"result":{"cleared":true,"card_source":…}}`。
config 里还有卡 → 立即改成它；config 也没卡 → 主链**还原基线**（Mod 仍在运行，
但人设已撤销，`state_json.ready` 变回 `false`）。本来就没有导入卡时也回 200
（`cleared:false`）——幂等，不报错。

### 3.8 界面内同一件事

「设置 → Mod → 角色卡」展开卡片：

1. 打开开关（停用时「导入并生效」是禁用的，页面就地说明原因——命令通道要 Mod 在跑）；
2. 顶部「会话绑定」块先告诉你**卡会落到哪**：有活动会话时显示「当前会话 <id>：
   导入的卡只对它生效」；一条消息都没发过时显示降级语义，并且「导入并生效」
   是禁用的（想对所有会话生效就走「导入为全局人设（所有会话）」）；
3. 把卡 JSON 粘进「粘贴角色卡 JSON（主路径）」→「导入并生效」；
   （宿主接好文件读取器时，还会多一个「选择 PNG 角色卡文件」按钮，同样绑定当前会话。）
4. 卡片顶部的「运行态（只读）」块会多出 `卡来源` / `角色卡名称`，
   以及 L1 的 `会话绑定可用` / `已绑定会话` / `当前活动会话` / `当前作用域`；
   面板自己那一行显示「当前全局角色卡：…」或（只绑了会话时）「会话绑定见上方」；
5. 停用 → **会话表清空 + 主链还原**；再打开 → 会话的卡还在（档案落盘）。
   想删掉某个会话的卡用「清除当前会话的卡」，删全局导入卡用「清除全局导入卡」。

## 4. 坏卡路径（**一律显式失败，不静默**）

「没真的接管主链，就不许留下『接管了』的痕迹」——这是本 Mod 的第一条纪律。

| 现象 | 结局 | 用户看到 |
| --- | --- | --- |
| 坏 JSON / 不是角色卡（数组、`{"foo":1}`） | `import_card` 回 `Err` → 409 `command_failed` | 面板上带错误码的一句话 + Mod 侧可读原因 |
| PNG 无 `chara` 块 / 截断 / 压缩 iTXt | 同上（错误里说明是哪一种） | 同上；**不 panic**、不落盘 |
| `data_base64` 不是 base64 | 同上 | 同上 |
| 卡文件 / `card_json` 超过上限 | 读盘/解析**之前**就拒（`metadata` 先量） | 错误里给出实际大小与上限 |
| 合成结果为空（字段全空 + 关掉纪律模板） | `Err` | 同上；主链提示词**一字不动** |
| `apply_settings` 写盘被拒 | `Err`，且**回滚**导入卡 | 同上；不出现「导入成功但重启就没了」 |
| 启用时 config 里的卡是坏的 | `start` 回 `Err` → `ModStatus::Failed` | Mod 状态为失败；主链继续跑；面板给「修好再打开开关」+ 看日志 |
| 导入卡文件本身损坏 / 超大 | `start` 回 `Err` | 面板提示，处置是「清除导入卡」 |

**失败态在界面上的可读性**：面板在 Mod 状态为 `failed` / `error` 时给一句
**可处置**的话——「角色卡没被接受，修好再打开开关」，并指向日志里 `mod: persona`
的行（诊断 → 日志，需先打开开发者模式）。状态字段两处都认：`GET /mods` 序列化的是
`failed`，而前端历史写法是 `error`，只认一个会让提示永不出现。

## 5. 与 memory 共存：last-writer-wins（**没有仲裁**）

`persona` 与 memory（`live2d-ai-mod-memory`）都可能写 `persona.system_prompt`。
规则与 [memory-mod-v0.md](memory-mod-v0.md) §5.1 **逐字一致**：

| 事件顺序 | 结果 |
| --- | --- |
| `persona` 后写 | 它的合成结果覆盖整个 `system_prompt`，**记忆块被冲掉** |
| memory 后写（下一轮） | 它在 persona 的合成结果之上重新拼上记忆块（base 一字不改） |
| memory 停用 | `shutdown` 检查 marker，有就剥掉写回——**不留残留** |

这是**已知取舍**，不是 bug：主链只有一个 system 入口，加一套优先级表就是在核心里
埋第二个产品（memory 文档 §8 与本文件 §9 同一条）。
**L1 起这条规则只对全局模式相关**：会话绑定的卡进的是宿主会话表，不进主链
（见 §8.6）。

**可执行建议**（面板里也写着同样的话）：

- 想**稳定用人设**：先开 `persona`、把卡导入好，**别同时开 memory 的注入开关**
  （`enabled_injection`）；配好之后再按需要开记忆，并知道下一轮它会叠在人物设定之上；
- 反过来，只想让记忆生效：关掉 `persona`（它会顺手把主链还原成基线）；
- **不要**指望「两个都开、各写一半」——persona 的 `start` 是**整段替换**，
  它不解析 memory 的 marker。

persona 的停用会把**启用那一刻的整段** `system_prompt` 还回去：当时若含记忆块就原样还回。
回归在 `src/tests_e2e.rs`，用的是 memory crate 的**真实**纯函数
（`strategy::compose_injection` / `strip_memory_block`），不是本地复刻。

## 6. 命令与运行态契约（给实现者）

| 命令 | 入参 | 成功 | 失败 |
| --- | --- | --- | --- |
| `import_card` | `{"card_json":"…"}` 或 `{"data_base64":"…"}`（都给时前者优先）+ 可选 `session_id` | 规范化 JSON 落盘 + 立刻生效 + 脱敏摘要；`session_id` 合法时 `scope:"session"`（只写会话表），否则 `scope:"global"`（写回主链） | 409 `command_failed`（其它 `ModError`）/ 400 坏 body / 404 不在注册表 / 503 未启用或正忙 |
| `clear_import` | 可选 `session_id`（幂等） | 有：清该会话 + 从档案删；无：删导入卡 → 按 config 重生效；config 没卡 → 还原基线 | 同上 |
| （其它字符串） | — | — | 409 `unsupported_command` |

会话作用域的文件、上限、坏文件策略与降级语义见 §8。

**顺序**：解析 → 落盘（tmp + rename）→ 写主链；写主链失败就**回滚**导入卡。
反过来会留下「人设换了、重启就丢了」的哑巴状态。

**`state_json`**：快照在 `start` / `import_card` / `clear_import` 时就地更新，
`GET …/state` 只读内存（**零 IO**）——它跑在 web_api 线程上，不许读盘 / 阻塞。
仍会 503 的只剩「未启用 / 已 Failed」（没有 runtime 实例）。

## 7. 回归（改这条链之前先看这里）

| 文件 | 守什么 |
| --- | --- |
| `crates/live2d-ai-mod-persona/src/tests.rs` | 卡解析、启停三条入口、坏输入的单点 `Err`、基线快照语义 |
| `crates/live2d-ai-mod-persona/src/tests_e2e.rs` | 坏卡 enable→Failed→修好→接管→还原；与 memory 的 §5.1 三行；**§8 会话绑定全套**（A/B 不串、清 A 不动 B、档案 round-trip 重启仍在、shutdown 全清、非法 id 不写任何地方、无宿主/无 config_path 可读失败、坏档案只 warn、50 上限） |
| `crates/live2d-ai-mod-persona/src/tests_command.rs` | `state_json` 形状/脱敏（含 L1 四个新键）、`import_card` 成功与各类坏输入、导入卡优先级与重启、`clear_import`、回滚、未知命令 |
| `shell/flutter/test/persona_panel_test.dart` | 面板注册与中文标签、粘贴/选文件两条导入路径、失败码上屏、失败态提示、与 memory 的文案；**§8 面板侧**（null 降级禁用、args 带 session_id、全局按钮不带、notifyChanged、会话清除） |

## 8. 会话绑定（L1，2026-09-15）

### 8.1 一句话

**带 `session_id` 的导入把卡写进「当前会话」这个桶，不碰全局
`persona.system_prompt`。** 换会话不串卡；回到原会话人设仍在（档案落盘 +
`start` 时灌回宿主会话表）；停用 Mod 清空全部会话绑定。

两条作用域是**两条不同的路**，界面上也是两个按钮：

| 动作 | 入参 | 落点 | 影响面 |
| --- | --- | --- | --- |
| 导入并生效（默认） | `session_id` = 当前会话 | `persona-mod-cards.json` + 宿主 `SessionPromptSink` | **只**这个会话 |
| 导入为全局人设（所有会话） | 不带 `session_id` | 全局导入卡 + `apply_settings` 写主链 | 所有会话（旧行为） |

宿主侧能力由 `live2d-ai-mod-system::session` 提供：一张「会话 id →
`system_prompt`」的**内存表**（进程级、不写 `live2d-ai.toml`、不触发
`reload()`）。表只存字符串，合法性由本 Mod 负责——所以**跨重启**那半条必须
本 Mod 自己落盘（见 8.2）。

### 8.2 档案文件 `persona-mod-cards.json`

与 `live2d-ai.toml` 同目录，**原子写**（tmp + rename）：

~~~json
{"version":1,"sessions":{"1757890123456789-3":{"spec":"chara_card_v2","data":{…}}}}
~~~

- 值是**规范化后的 V2 卡**（`PersonaCard::to_json()`），可读、可再解析；
- 会话 id 先过 `sanitize_session_id`（ASCII 字母数字与 `-` `_` `.` `:`，
  ≤128 字符）——它将来会被用作文件名/存储键，规则现在钉死；
- 上限 **50 个会话**；单文件大小沿用 `MAX_CARD_FILE_BYTES`（16 MiB）。
  超限 → 导入回**可读错误**（不静默丢最旧的一条）；
- **坏文件 = warn + 忽略整份**（版本不认 / 不是 JSON / 会话数超限），
  单个会话的卡坏则只跳过那一条。这与「config 里的卡坏了 → Mod Failed」是
  **两种输入、两种结局**：档案是**派生缓存**，用户显式配的才是真相；
- **优先级**：档案只服务会话作用域；全局作用域仍按 §2 的
  导入卡 > `card_json` > `card_path`。两者互不覆盖（一个在会话表，一个在主链）。

`start` 时载入档案并把每个会话的卡**灌回宿主会话表**——宿主的表是内存态，
这就是「进程重启后回到会话 A 人设还在」的落点。

### 8.3 命令契约（增量）

| 命令 | 新增入参 | 结果 |
| --- | --- | --- |
| `import_card` | 可选 `session_id` | 合法 id → `scope:"session"` + `session_id`，写档案 + 会话表，**不写全局**；缺省/空串 → 老全局语义（`scope:"global"`）；非空非法 → 可读错误，**不写任何地方** |
| `clear_import` | 可选 `session_id` | 有 → 从档案删 + 清该会话（其余会话与主链不受影响，幂等）；无 → 老语义（删全局导入卡 → 按 config 重生效 / 还原基线） |

失败码不变：`command_failed`（可读中文原因）/ `unsupported_command` /
`command_unavailable`（未启用或正忙）。本环境没有会话能力（host 未注入
`session_prompts`）或没有 `config_path`（档案无处落）→ `command_failed`，
**明确失败，不静默降级成全局写回**。

`state_json` 新增四个键（既有九个语义不变）：

| 键 | 含义 |
| --- | --- |
| `session_bound` | 本进程是否有会话绑定能力（`session_prompts.enabled()`） |
| `sessions` | 已绑定的会话 id（已排序；宿主表可用时以它为准） |
| `active_session` | 宿主记录的当前活动会话（没有 = `null`） |
| `scope` | 当前人设作用域：活动会话有绑定 → `"session"`，否则 `"global"` |

### 8.4 降级语义（必须写在界面上）

- 有活动会话：面板显示「当前会话 <id>：导入的卡只对它生效」；
- **没有活动会话**（首次打开、一条消息都没发）：面板显示「先发一条消息（会自动
  建会话），或用『导入为全局人设』对所有会话生效」，且「导入并生效」**禁用**
  ——**绝不**偷偷改成全局导入（那正是这会话绑定要根除的串人设）；
- 想对所有会话生效：走显式的「导入为全局人设（所有会话）」按钮（旧行为）。

### 8.5 停用还原

`shutdown` 做两件事，缺一不可：

1. `session_prompts.clear_owner("persona")`——**只清 persona 自己的来源槽**；

   > 为什么不是 `clear_all()`：宿主会话表是**共享的**（memory 也往里写自己的槽）。
   > `clear_all()` 会把 memory 的贡献一起清掉，而 memory 不会自动重写——表现为
   > 「关掉角色卡，记忆注入也一起消失」。L1 收束时改成了按来源槽清理
   > （`session_prompts` 的组合顺序与 owner 常量见
   > [session-scope-l1.md](session-scope-l1.md)）。
2. 老的全局基线还原（§3.6）——主链 `system_prompt` 回到启用前的快照。

档案**不删**：停用只是让覆盖不生效，下次启用 `start` 会重新灌回；
真正的删除只有 `clear_import`。

### 8.6 与 memory 的 last-writer-wins：现在只在**全局**模式下相关

§5 的三行规则描述的是「两个 Mod 都写主链 `persona.system_prompt`」。会话绑定
**不进主链**（写的是宿主会话表），因此：

- **会话绑定 + memory 同时开**：**不冲突也不覆盖**。宿主会话表按来源槽存：
  persona 写 `persona` 槽（整段卡），memory 写 `memory` 槽（只有记忆块），
  读时按固定顺序 `匿名 → persona → memory` 用空行连接。因此两者是**叠加**关系，
  与谁先写无关；停用任一方只清自己那一槽。
- **全局导入 + memory**：仍是 §5 的 last-writer-wins（后写覆盖、无仲裁）。

## 9. 非目标（刻意不做）

1. **不做仲裁**：persona 与 memory 的优先级表不存在，将来要加必须先论证（§5）；
2. **不回写 config**：导入卡住 Mod 自己的状态文件，不去改 `mods.json`（§2）；
3. **不做动态加载**：不新增「运行时下载卡 / 在线卡库」——卡只在用户本机；
4. **不碰主链皮肤**：LLM / TTS / 口型 / Live2D 一行不改，本 Mod 只写 `persona.system_prompt`；
5. **不复活动作**：本 Mod 不请求任何动作 / 表演（`action_tx` 与本链无关）；
6. **不做卡编辑**：不提供「在界面里改卡字段并保存回卡」的编辑器，改字段请用配置里的覆盖项；
7. **不上传**：面板只把文本 / 字节交给**本机**服务，不出网。
