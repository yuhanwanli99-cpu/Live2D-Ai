# B 站直播弹幕 → Live2D-Ai 注入 sidecar（最小示例）

> **这是在 Windows 上跑的一个独立小脚本，不是主仓的一部分。**
> Live2D-Ai 主仓（Rust）**不实现** B 站协议 / blivedm / WSS / 开放平台 SDK；
> 主仓只暴露一个 loopback 的 HTTP 注入端点 `POST /api/v1/external/chat`。
> 本目录的脚本属于「第三方事件源」一侧，抓弹幕、清洗、POST 过去。

契约全文见 `docs/external-input.md`。

---

## 1. 它做什么

```
B 站直播间
   │  blivedm（WebSocket 长连接）
   ▼
bilibili_sidecar.py
   │  只处理 DANMU_MSG（弹幕）+ SEND_GIFT（礼物）；其它 cmd 只 log
   │  clean_text()：去换行/控制符、压空白、截断
   ▼
POST http://127.0.0.1:18080/api/v1/external/chat
   ▼
Live2D-Ai：supervisor.say → LLM → TTS → 口型 → 皮套开口
```

## 2. 快速开始（Windows）

前置：Live2D-Ai 已在本机跑起来（WSL2 里 `./scripts/ignite.sh`，端口 18080），
且 `external-input` Mod 处于启用状态（0.2.0-rc.1 起**缺省启用**）。

```powershell
# 1) 依赖（建议独立 venv）
py -3 -m venv .venv
.\.venv\Scripts\Activate.ps1
pip install -r requirements.txt

# 2) 先干跑，确认清洗/前缀逻辑（不会连 B 站、也不会发 HTTP）
$env:BILI_ROOM_ID = "123456"
$env:SIDECAR_DRY_RUN = "1"
python bilibili_sidecar.py

# 3) 正式跑
$env:BILI_SESSDATA = "<你的 SESSDATA>"
$env:EXTERNAL_INPUT_TOKEN = "<与 Live2D-Ai 侧一致的 token，可留空>"
Remove-Item Env:\SIDECAR_DRY_RUN
python bilibili_sidecar.py
```

---

## 3. 三个必须搞清楚的配置项

### 3.1 `BILI_ROOM_ID` —— 真实房间号，不是短号

- 填**长号**（真实房间号）。B 站页面上显示的短号（如 `123`）需要先解析成
  真实房间号（打开直播间看 URL / 用官方接口 `room_init` 转换）。
- 填错会连到一个不存在的房间：表现为一直收不到弹幕，脚本本身不报错。

### 3.2 `BILI_SESSDATA` —— 登录态 cookie（强烈建议）

- 不填也能连，但**部分房间 / 高峰时段会限流或收不到完整弹幕**。
- 从浏览器登录 B 站后，开发者工具 → Application → Cookies → `.bilibili.com` →
  `SESSDATA` 复制值即可。
- **它是账号凭据，等同于登录态**：只放在本机环境变量里，**不要**写进仓库、
  不要贴到 issue / 截图里。本脚本只把它塞进 aiohttp 的 cookie jar，不落盘、不打日志。
- 有时还需要 `buvid3`；若匿名/半登录连不上，可一并注入（按 blivedm 上游说明）。

### 3.3 `EXTERNAL_INPUT_TOKEN` —— 注入端点的可选令牌

- Live2D-Ai 侧 `external-input` Mod 可以配一个 token（env 或 Mod 设置里的
  secret 字段）。**两边必须一致**：sidecar 这头设 `EXTERNAL_INPUT_TOKEN`，
  服务端那头设 `EXTERNAL_INPUT_TOKEN` 或 Mod config `token`。
- 服务端**两者都空 = 不鉴权**（端点仍只监听 127.0.0.1）。仅本机使用时可以留空；
  若这台机器上还有别的可疑本地程序，建议设上。
- 注意：token 通过请求体 `token` 或 `Authorization: Bearer` 传递。本脚本两个都发。

## 4. 常用调整

| 想做的事 | 怎么做 |
|---|---|
| 给弹幕加统一前缀 | 服务端 Mod 设置 `prefix`（推荐，改一处即可）；或本地 `SIDECAR_PREFIX` |
| 用模板改写 | 服务端 Mod 设置 `text_template`（`{text}` 是占位符） |
| 换 Live2D-Ai 地址/端口 | `LIVE2D_AI_URL`（默认 18080，与 `ignite.sh --port` 一致） |
| 只测清洗不入链路 | `SIDECAR_DRY_RUN=1` |
| 少刷屏 | 在 `Live2DHandler` 里加节流（如每秒最多 1 条）；示例保持最小，不内置策略 |

---

## 5. 协议边界与已知缺口

- **只做两件事**：`DANMU_MSG` 与 `SEND_GIFT`。其它 cmd（进场、点赞、
  关注、上舰等）在 `_on_unknown_cmd` 里只打一行日志，**不注入**。
- **SEND_GIFT_V2（灰度）**：B 站正在灰度新的礼物推送结构。blivedm 当前版本通常
  只解析经典 `SEND_GIFT`；遇到 `SEND_GIFT_V2` 时会落到
  `_on_unknown_cmd`，**只 log 不注入**（宁可漏一条礼物，不可用错误字段
  拼出乱语）。后续兼容点：等 blivedm 上游支持该结构后，在 `Live2DHandler` 增加
  对应回调（如 `_on_send_gift_v2`）并复用 `submit()`；在那之前**不要**
  在 Rust 主仓里手写 B 站协议解析。
- **主仓禁项**：B 站协议、blivedm、WSS、开放平台 SDK、导演/动作、动态加载——
  都不进 Rust 核心（见 `AGENTS.md` 与 `docs/architecture/mod-product-chain.md`）。
- **这不是官方 SDK**：blivedm 是第三方库，B 站接口/风控随时可能变化；sidecar 挂了
  只影响弹幕注入，**不影响 Live2D-Ai 主链路**。

## 6. 故障排查

| 现象 | 先看 |
|---|---|
| `Connection refused` / 注入失败 | Live2D-Ai 没起？端口不对？(`LIVE2D_AI_URL`) |
| `HTTP 403 mod_disabled` | `external-input` Mod 被停用了：前端「Mod 管理」启用，或 `POST /api/v1/mods/external-input/enable` |
| `HTTP 401 unauthorized` | 服务端设了 token 但两边不一致 |
| `服务端忙碌，本条丢弃` | 角色这轮还没说完；正常退避，或降低弹幕注入频率 |
| 一直收不到弹幕 | 房间号是不是短号？SESSDATA 是否过期？ |
