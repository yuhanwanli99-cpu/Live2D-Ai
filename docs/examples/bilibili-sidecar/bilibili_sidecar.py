#!/usr/bin/env python3
"""B 站直播弹幕 / 礼物 -> Live2D-Ai 本地注入端点（Windows sidecar 最小示例）。

职责边界（**很重要**）：
  本脚本住在 **Windows sidecar**，不属于 Live2D-Ai 主仓的 Rust 核心。
  主仓只提供一个 loopback 的 HTTP 注入端点 POST /api/v1/external/chat；
  B 站协议（blivedm / WebSocket / 长连接鉴权）**不编进 Rust**，也不在
  Live2D-Ai 进程内运行。sidecar 把弹幕清洗成一行纯文本后 POST 过去即可。

数据流：
  B 站 -> blivedm(本脚本) -> clean_text() -> POST /api/v1/external/chat
       -> Live2D-Ai supervisor.say -> LLM -> TTS -> 口型 -> 皮套

只处理两类事件：
  - DANMU_MSG（普通弹幕）  -> blivedm 回调 _on_danmaku
  - SEND_GIFT（投喂礼物）  -> blivedm 回调 _on_gift
其它 cmd 一律只 log（_on_unknown_cmd），不注入。

环境变量：
  BILI_ROOM_ID          必填，直播间**真实房间号**（长号；短号请先解析成真实号）
  BILI_SESSDATA         强烈建议。登录态 cookie，缺它时部分弹幕会被限流/收不到
  LIVE2D_AI_URL         可选，默认 http://127.0.0.1:18080/api/v1/external/chat
  EXTERNAL_INPUT_TOKEN  可选；若 Live2D-Ai 侧设了 token，这里必须一致
  SIDECAR_PREFIX        可选，拼在文本前的本地前缀（默认空；也可用服务端 Mod 的 prefix）
  SIDECAR_DRY_RUN=1     可选，只打印不发送（无 Live2D-Ai 进程时自测用）

依赖：pip install -r requirements.txt

注意：SEND_GIFT_V2 灰度——B 站正在灰度新的礼物推送结构。blivedm 当前版本
      通常只解析经典 SEND_GIFT；若线上切到 SEND_GIFT_V2，本脚本会走
      _on_unknown_cmd 记一行日志而**不注入**（宁可漏，不可拼错）。后续需要
      按 blivedm 上游对新结构的支持再补一个 _on_send_gift_v2 回调，见 README。
"""

from __future__ import annotations

import asyncio
import json
import os
import sys
import urllib.error
import urllib.request
from concurrent.futures import ThreadPoolExecutor

try:
    import blivedm
except ImportError:  # pragma: no cover - 运行环境依赖
    print("[sidecar] 缺少依赖 blivedm：请先 `pip install -r requirements.txt`", file=sys.stderr)
    raise SystemExit(2)

try:
    import aiohttp
except ImportError:  # pragma: no cover
    print("[sidecar] 缺少依赖 aiohttp：请先 `pip install -r requirements.txt`", file=sys.stderr)
    raise SystemExit(2)


# ---------------------------------------------------------------- 配置

ROOM_ID = int(os.environ.get("BILI_ROOM_ID", "0") or "0")
SESSDATA = os.environ.get("BILI_SESSDATA", "").strip()
TARGET = os.environ.get(
    "LIVE2D_AI_URL", "http://127.0.0.1:18080/api/v1/external/chat"
).strip()
TOKEN = os.environ.get("EXTERNAL_INPUT_TOKEN", "").strip()
PREFIX = os.environ.get("SIDECAR_PREFIX", "")
DRY_RUN = os.environ.get("SIDECAR_DRY_RUN", "") == "1"

# 单条注入文本上限（字符）。服务端渲染后还会再判一次 2000，这里先做一道。
MAX_TEXT_CHARS = 500

# 注入用线程池：HTTP POST 是阻塞的，不能卡住 blivedm 的 asyncio 心跳。
_EXECUTOR = ThreadPoolExecutor(max_workers=2, thread_name_prefix="l2d-inject")


# ---------------------------------------------------------------- 清洗 / 注入


def clean_text(raw: str, limit: int = MAX_TEXT_CHARS) -> str:
    """把一条外部文本清洗成**单行安全**文本。

    - 去掉换行 / 制表 / 控制字符（它们会破坏日志与气泡排版）；
    - 压缩连续空白；
    - 截断到 limit 字符（按字符，不按字节；中文也算 1）。
    """
    if not raw:
        return ""
    # 保留可打印字符，其余（含 \\r \\n \\t 及 C0/C1 控制符）替换为空格。
    cleaned = "".join(ch if ch.isprintable() else " " for ch in raw)
    cleaned = " ".join(cleaned.split())
    if len(cleaned) > limit:
        cleaned = cleaned[:limit]
    return cleaned


def _post(text: str) -> None:
    """阻塞 POST 到本机 Live2D-Ai 端点。失败只 log，不抛（sidecar 不能因一次失败退出）。"""
    body = {"text": text}
    if TOKEN:
        body["token"] = TOKEN
    data = json.dumps(body, ensure_ascii=False).encode("utf-8")
    headers = {"Content-Type": "application/json"}
    if TOKEN:
        headers["Authorization"] = "Bearer " + TOKEN
    req = urllib.request.Request(TARGET, data=data, headers=headers, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=5) as resp:
            payload = resp.read().decode("utf-8", "replace")
            if resp.status != 200:
                print(f"[sidecar] 注入非 200：{resp.status} {payload}", file=sys.stderr)
            elif '"ok": false' in payload.replace(" ", ""):
                # 忙碌：supervisor pending 缓冲已满，本条被丢弃。退避即可。
                print(f"[sidecar] 服务端忙碌，本条丢弃：{payload}", file=sys.stderr)
            else:
                print(f"[sidecar] 已注入：{text}")
    except urllib.error.HTTPError as e:
        detail = e.read().decode("utf-8", "replace")
        print(f"[sidecar] 注入失败 HTTP {e.code}：{detail}", file=sys.stderr)
    except Exception as e:  # noqa: BLE001 - sidecar 顶层要吞掉网络异常
        print(f"[sidecar] 注入失败：{e}", file=sys.stderr)


def submit(text: str) -> None:
    """清洗后异步提交（在线程池里发 HTTP，不阻塞事件循环）。"""
    text = clean_text(text)
    if not text:
        return
    line = PREFIX + text
    if DRY_RUN:
        print(f"[sidecar][dry-run] {line}")
        return
    loop = asyncio.get_running_loop()
    loop.run_in_executor(_EXECUTOR, _post, line)


# ---------------------------------------------------------------- blivedm 回调


class Live2DHandler(blivedm.BaseHandler):
    """只把弹幕 / 礼物送进 Live2D-Ai；其余 cmd 仅 log。"""

    # DANMU_MSG
    def _on_danmaku(self, client, message) -> None:
        # message.content = 弹幕原文；message.uname = 发送者昵称。
        submit(f"弹幕 {message.uname}：{message.content}")

    # SEND_GIFT
    def _on_gift(self, client, message) -> None:
        # gift_name / num / uname 是经典 SEND_GIFT 的字段。
        if getattr(message, "num", 0) <= 0:
            return
        submit(f"{message.uname} 送出 {message.gift_name} x{message.num}")

    # 明确忽略的高频事件（不 log，避免刷屏）。
    def _on_heartbeat(self, client, message) -> None:
        return

    # 其它 cmd：只记一行，**不注入**（含 SEND_GIFT_V2 灰度结构）。
    def _on_unknown_cmd(self, client, command, message) -> None:
        print(f"[sidecar] 忽略 cmd={command}", file=sys.stderr)


# ---------------------------------------------------------------- 入口


async def _run() -> None:
    if ROOM_ID <= 0:
        print("[sidecar] 请设置 BILI_ROOM_ID（真实房间号）", file=sys.stderr)
        raise SystemExit(2)

    session = aiohttp.ClientSession()
    if SESSDATA:
        session.cookie_jar.update_cookies({"SESSDATA": SESSDATA})
    else:
        print(
            "[sidecar] 未设置 BILI_SESSDATA：匿名连接可能收不到全部弹幕，建议配置",
            file=sys.stderr,
        )

    client = blivedm.BLiveClient(ROOM_ID, session=session, uid=0)
    client.set_handler(Live2DHandler())
    client.start()
    print(f"[sidecar] 已连接房间 {ROOM_ID} -> {TARGET}（dry_run={DRY_RUN}）")
    try:
        await client.join()
    finally:
        await client.stop_and_close()
        await session.close()
        _EXECUTOR.shutdown(wait=False)


def main() -> None:
    try:
        asyncio.run(_run())
    except KeyboardInterrupt:
        print("[sidecar] 退出")


if __name__ == "__main__":
    main()
