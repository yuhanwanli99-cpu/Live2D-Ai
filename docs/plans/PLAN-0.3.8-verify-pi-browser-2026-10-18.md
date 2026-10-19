# 执行单：用本机 Pi 打真 A2A，并用浏览器看隐藏（2026-10-18）

> 交给测试执行 agent。本单只验证，不改产品。
> 树：`/home/skystar/Live2D-Ai`，分支 `main`。已验收 HEAD `16bbfad`（`v0.3.8-rc.3`，未 push）。
> 本机没有 Pi。Pi 从 GitHub 下载，不要假设 `pi` 已在 PATH 里。
> 真 Ollama **不做**。标成 3.0 遗留，留到 4.0 再看。`0.4.0` 仍只做 Windows，不要把拉模型写进 `0.4.0`。

## 0. 做完是什么样

1. Pi 源码来自 `https://github.com/earendil-works/pi`，克隆在仓库**外面**（`/home/skystar/pi`）。交付写下克隆到的 commit。
2. Pi 自己的对话模型与本项目 `[llm]` 一致：`base_url = https://api.deepseek.com/v1`，`model = deepseek-flash`，密钥只来自变量名 `DEEPSEEK_API_KEY`。不另配一把钥匙。
3. Pi 装上 A2A 客户端，向 `http://127.0.0.1:18080` 发一条 `message/send`，拿回非空正文。这条调用不进皮套聊天。
4. 浏览器打开 `http://127.0.0.1:18080/app/`，设置 → 扩展里没有 `local-engine`，也没有「本地引擎（仅拉起进程）」。其余已注册 Mod 仍在。`dev_mode` 开着也不显示这个 id。
5. 不安装 Ollama，不 `ollama pull`。

## 1. 禁止

- 不改产品代码，不 commit，不 tag，不 push，不改 `Cargo.toml` / `pubspec.yaml`。
- 不改 `.env`、`mods.json`、`live2d-ai.toml`。不把密钥写进仓库、计划、日志、`auth.json` 明文。
- 不提交、不移动 `official_live2d_samples.7z`。
- 不把 Pi 克隆进本仓库。不在本仓库目录里启动 Pi（它有写文件工具）。工作目录用 `/tmp/pi-a2a-probe`。
- 不杀已有的 `ignite.sh` / `live2d-ai-desktop`。18080 上已有进程就用它。没有进程才允许按下面第 4 节启动一次。
- 不给 A2A 请求带上 DeepSeek 密钥。密钥只给 Pi 自己的模型供应商。本服务的 `/api/v1/a2a` 不鉴权这把钥匙。
- 不改 A2A 协议去迁就客户端。`tasks/cancel`、`message/stream`、`tasks/list` 返回错误是既有行为。
- 不下载模型权重，不把 `[llm].mode` 改成 local。
- 不提高 `DOCS_BUDGET_LINES`。

## 2. Pi：从 GitHub 取得

本机没有 Pi。只从这一处克隆：

```bash
git clone --depth 1 https://github.com/earendil-works/pi.git /home/skystar/pi
```

目录已存在且是这份仓库时，在里面 `git pull --ff-only`，不要重新克隆到仓库内。`badlogic/pi-mono` 与包名 `@mariozechner/pi-coding-agent` 是旧位置，不要用。

按克隆下来的 README 构建，跑出 `pi --version`。需要 Node.js 22.19 或更新。本机没有合格的 `node` 时停下来写明版本，不要改用 `curl | sh` 安装器代替这次 GitHub 克隆。

A2A 扩展不是 Pi 本体，用 Pi 自己的安装命令（在 `pi` 可用之后）：

```bash
pi install npm:pi-a2a-adaptor
```

这个扩展的发现路径是 `GET /.well-known/agent-card.json`，part 带 `kind`，POST 的 URL 用卡片里的 `url`。这与本服务的 0.3.0 形状一致。不要改用官方 `a2a` CLI（那是 1.0，线格式不同）。

## 3. 令牌与模型跟项目一致

读 `live2d-ai.toml` 的 `[llm]`，只核对这三项，不要把值写进交付以外的文件：

- `base_url` 应为 `https://api.deepseek.com/v1`
- `model` 应为 `deepseek-flash`
- `api_key_env` 应为 `DEEPSEEK_API_KEY`

Pi 启动前让进程环境里已有 `DEEPSEEK_API_KEY`（与 `./scripts/ignite.sh` 同一来源，即仓库 `.env`）。检查只允许 `test -n "$DEEPSEEK_API_KEY"`，禁止 `echo` 这个值。

Pi 的模型选 `deepseek-flash`，基址用上面的 `base_url`。内置 deepseek 目录里没有这个 id 时，只在 `~/.pi/agent/models.json` 加一条自定义项：协议 `openai-completions`，`baseUrl` 与 `model` 同上，密钥环境变量名 `DEEPSEEK_API_KEY`。文件模式 `0600`。不要把密钥正文写进 json。

先做一次不涉及 A2A 的短问（打印模式，一句「只回一个字：好」）。有非空回复才算令牌接通。失败就停，不要改 `.env`。

## 4. 服务

先看 `18080` 是否已有进程。

- 已有：`GET http://127.0.0.1:18080/.well-known/agent-card.json`。200 且 `skills[0].id == llm-chat`、`url` 以 `/api/v1/a2a` 结尾，才继续。404 或没有这个字段：写明「当前进程不是 0.3.8 二进制」，停。不要杀进程。
- 没有：在仓库根执行 `./scripts/ignite.sh --build`。`index.html` 已在时它不会重编 Flutter；隐藏那次构建已经进过 `shell/flutter/build/web`。不要另起第二份。

## 5. Pi 调用 A2A

对 `http://127.0.0.1:18080` 做 `/a2a-discover`，再 `/a2a-send` 一句短中文，例如「用一句话回答：今天星期几」。只要 `message/send` 的正文。

- A2A 客户端认证用 `none`。不要设 `bearerToken` / `apiKey` 去打本机 18080。
- 客户端若不带 `Origin`，POST 会 403。先给请求加上 `Origin: http://127.0.0.1:18080`。扩展做不到这点：停，并写明 403。不要开 `LIVE2D_AI_ALLOW_NO_ORIGIN`，不要为此重启服务。
- 成功：回复是非空文本。思考链不应当作正文（服务端已丢弃 `ReasoningDelta`）。
- 扩展若顺手调用 `tasks/cancel`、`message/stream`、`tasks/list` 并因 `-32004` / `-32601` 把整次算失败：记为客户端多调用了本版拒绝的方法。`message/send` 已有正文则 A2A 仍算通过。不要改服务器去实现这些方法。
- 用打印模式或有超时的一次性会话。结束要退出 Pi，不要留后台进程。

## 6. 浏览器

执行 agent 用浏览器，不要只截一张图。

1. 打开 `http://127.0.0.1:18080/app/`，确认舞台或聊天壳出现。
2. 进入设置 → 扩展。断言页面文字里没有 `local-engine`，也没有「本地引擎（仅拉起进程）」。
3. `external-input`、`persona`、`voice-input`、`memory`、`local-tts-melo` 仍各有一张卡（名字可能是中文界面名，id 在开发模式下会显示；空名字时标题就是 id）。
4. 本机 `dev_mode = true`。开发模式下也不出现 `local-engine`。
5. 做完第 5 节之后看聊天记录：Pi 那句 A2A 提问不在皮套对话里，也不出声。不要按「保存并重载」，不要改开关。

页面打不开或扩展区仍有这张卡：停，写明 URL 和看见的卡片标题。不要改 Dart。

## 7. Ollama（本单不做）

WSL2 内存不足。显存预算只有大约 4–8 GB。即便只拉一个 2–4B，本轮也不做。

遗留写进交付，不要写进 `0.4.0` 的功能范围：真 Ollama / 真本地权重仍是 3.0 未验项，带到 4.0 规划时再决定。到那时至多拉一个 2–4B，并且先确认内存放得下。

## 8. 交付

写明：Pi 的 commit、`pi --version`、Node 版本；令牌短问是否有正文（不贴密钥）；agent card 的 HTTP 状态；`/a2a-send` 是否有正文、有没有误伤 `tasks/cancel`；浏览器四步各看见什么；18080 是原进程还是本单拉起的。Ollama 写「未做，3.0 遗留」。不要贴 `.env` 或回复里的密钥。
