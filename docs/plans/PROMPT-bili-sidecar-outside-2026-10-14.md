# 执行单：仓外 B 站弹幕 sidecar（2026-10-14）

> 交给工作 agent。本单就是范围。不要再调研产品边界，不要扩大。
> 代码写在 **Live2D-Ai 仓库外面**。仓库只许读，一行都不要改。
> 不要改 `.env`、`mods.json`、`live2d-ai.toml`。不要点火，不要启停 18080，不要 commit / tag / push。
> 不要把 B 站协议、blivedm、cookie 写进 Rust。不要把 `SESSDATA` 或 token 写进任何文件、日志、提交。

## 0. 做完是什么样

本机有一个独立目录，连上 B 站直播间，把弹幕洗成一行文字，POST 到本仓库已经在听的外部事件入口。调试要留下两份证据：抓到了弹幕原文；服务端计数 `accepts` 增加了。

目录：`/home/skystar/live2d-bili-sidecar/`（不在 git 仓库里）。可参考仓库里的示例 `docs/examples/bilibili-sidecar/`（`bilibili_sidecar.py`、`README.md`、`requirements.txt`），把它的行为搬出去。示例本身不要改。

## 1. 接到哪

唯一入口是已经在跑的 Live2D-Ai，不是新开一个端口。

| 项 | 值 |
|---|---|
| 方法 / 路径 | `POST /api/v1/external/chat` |
| 默认地址 | `http://127.0.0.1:18080/api/v1/external/chat` |
| 端口 | 进程实际监听的 `[web].port`。`./scripts/ignite.sh` 默认 **18080**。环境变量 `LIVE2D_AI_URL` 可整段覆盖 |
| 正文 | `{"text":"…"}`，`Content-Type: application/json` |
| 成功 | HTTP 200，`{"ok":true,"endpoint":"external.chat"}` |
| 忙碌 | HTTP 200，`ok:false`，`error.code=busy`。本条已丢，打印后继续，不要退出 |
| 令牌 | 仅当环境变量 `EXTERNAL_INPUT_TOKEN` 非空时，正文带 `token`，并加 `Authorization: Bearer`。不要读 `.env` 文件，不要打印令牌 |

Mod 配置里的 `listen_port` **只是界面提示**，本 Mod 不开 socket。不要往那个数字上发，也不要自己 bind 一个端口。

**必须带 Origin。** 无 Origin 且服务端没开 `LIVE2D_AI_ALLOW_NO_ORIGIN` 时，返回 `403 origin_required`。请求头：

`Origin: http://127.0.0.1:<与 URL 相同的端口>`

只接受 `http://127.0.0.1:<port>` 或 `http://localhost:<port>`，端口必须等于服务端正在听的端口。不要去改服务端的允许无 Origin 开关。

其它错误照契约处理，不要重试风暴：`401 unauthorized`（令牌不一致）、`403 mod_disabled`（Mod 停了）、`400`、`415`、`503`。打印状态码和 `error.code` 后继续跑。不要调用 enable / disable / 重置计数。

文本：`clean_text` 去掉换行和控制符、压空白、按字符截到 500。服务端渲染后再限 2000。空串不发。

只注入两类：`DANMU_MSG` → `弹幕 {uname}：{content}`；礼物 `SEND_GIFT` / `SEND_GIFT_V2` → `{uname} 送出 {gift_name} x{num}`。其它 cmd 只打日志。节流缺省 1000ms，`--min-interval-ms 0` 关掉。HTTP 放在线程池里，不要堵住 blivedm 的心跳。

`blivedm` 从 GitHub 装（`git+https://github.com/xfgryujk/blivedm.git`），不要 `pip install blivedm`（PyPI 停在旧包）。依赖写在该目录的 `requirements.txt`。

## 2. 环境变量

| 变量 | 作用 |
|---|---|
| `BILI_ROOM_ID` | 必填。**真实房间号（长号）**，不是页面短号 |
| `BILI_SESSDATA` | 建议有。只放进 aiohttp 的 cookie jar，不落盘、不打印 |
| `LIVE2D_AI_URL` | 可选，默认上面的 18080 地址 |
| `EXTERNAL_INPUT_TOKEN` | 可选，与服务端一致才发 |

房间号没设置：`--selftest` 和下面的探测仍要做；直播那一步打印缺什么，以退出码 2 结束，不要猜一个房间。

sidecar 与 Live2D-Ai 跑在同一侧（WSL 的 `127.0.0.1`）。不要改到 Windows 上另起一套。

## 3. 调试（两段都要留下输出）

**甲、接入。** 先 `GET http://127.0.0.1:18080/api/v1/mods/external-input/state`，记下 `accepts`。再 POST 一条唯一探测句，正文 `text` 用 `sidecar-probe-` 加当前秒数。然后再 GET 一次。

通过的条件：探测那次 HTTP 200 且 `ok:true`，并且 `accepts` 比之前大 1。`token_set` 可以看，令牌明文不可以出现在输出里。

18080 没在听：不要启动 `ignite.sh`。把「接入未测，服务没在听」写进结果，退出码不要伪装成成功。`busy` 时隔 5 秒再探一次，仍忙就如实写下，不要连发。

**乙、抓取。** `--selftest` 必须离线通过（清洗、节流、请求体含 `text`，且请求会带 Origin；不需要 blivedm、不需要服务端）。然后在设了 `BILI_ROOM_ID` 时用 `--dry-run` 连房间，最多等 45 秒。通过的条件：日志里至少一行真实弹幕（含「弹幕 」和发言人）。45 秒内一条都没有：用公开的房间初始化接口核对这个号是不是短号，把「已连接 / 0 条 / 房间号是否像短号」写清楚，不要换一个别的房间去凑一条。

正式发送（去掉 `--dry-run`）只在甲、乙都通过之后跑。跑起来后看到一条 `已注入`，并且再 GET 一次 `accepts` 又增加，就可以停。不要为了刷计数一直挂着。

## 4. 验收

在 `/home/skystar/live2d-bili-sidecar` 里：

```bash
python3 bilibili_sidecar.py --selftest
```

退出码 0。把甲、乙的命令输出原文附在完成报告里（去掉 cookie 和令牌）。确认 `git -C /home/skystar/Live2D-Ai status` 没有因为本单产生新改动。

## 5. 明确不做

不改 Live2D-Ai 仓库。不把 sidecar 拷回 `docs/examples/`。不实现进场、点赞、上舰。不在 Rust 里解析 B 站。不新开监听端口。不提交。
