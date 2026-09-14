#!/usr/bin/env python3
"""B 站直播弹幕 / 礼物 -> Live2D-Ai 本地注入端点（Windows sidecar 最小示例）。

职责边界（**很重要**）：
  本脚本住在 **Windows sidecar**，不属于 Live2D-Ai 主仓的 Rust 核心。
  主仓只提供一个 loopback 的 HTTP 注入端点 POST /api/v1/external/chat；
  B 站协议（blivedm / WebSocket / 长连接鉴权）**不编进 Rust**，也不在
  Live2D-Ai 进程内运行。sidecar 把弹幕清洗成一行纯文本后 POST 过去即可。

数据流：
  B 站 -> blivedm(本脚本) -> clean_text() -> 节流 -> POST /api/v1/external/chat
       -> Live2D-Ai supervisor.say -> LLM -> TTS -> 口型 -> 皮套

只处理两类事件：
  - DANMU_MSG（普通弹幕）  -> blivedm 回调 _on_danmaku
  - SEND_GIFT / SEND_GIFT_V2（礼物，见 README §5）-> blivedm 回调 _on_gift
其它 cmd 一律只 log、不注入。当前 blivedm 的 BaseHandler.handle 遇到未知 cmd 会
**自己** log 一次并 return（不会调用 _on_unknown_cmd），所以 SEND_GIFT_V2 的兜底
检测写在 handle 覆盖里：只有「本地 blivedm 的 cmd 表里没有 SEND_GIFT_V2」时才计数
v2_ignored，并随每次注入上报给服务端（见 README §5 与 docs/external-input.md）。

命令行：
  --min-interval-ms N  两条注入之间的最小间隔（毫秒，缺省 1000；0 = 关闭节流）
  --dry-run            只打印不发送（**仍会连 B 站收弹幕**；完全离线请用 --selftest）
  --selftest           跑离线自检（清洗 / 节流 / 干跑）后退出；**不需要**
                       blivedm / aiohttp / 网络 / Live2D-Ai 进程

环境变量：
  BILI_ROOM_ID          必填，直播间**真实房间号**（长号；短号请先解析成真实号）
  BILI_SESSDATA         强烈建议。登录态 cookie，缺它时部分弹幕会被限流/收不到
  LIVE2D_AI_URL         可选，默认 http://127.0.0.1:18080/api/v1/external/chat
  EXTERNAL_INPUT_TOKEN  可选；若 Live2D-Ai 侧设了 token，这里必须一致
  SIDECAR_PREFIX        可选，拼在文本前的本地前缀（默认空；也可用服务端 Mod 的 prefix）
  SIDECAR_DRY_RUN=1     可选，只打印不发送
  SIDECAR_MIN_INTERVAL_MS  可选，节流缺省值（命令行 --min-interval-ms 覆盖）

依赖：pip install -r requirements.txt（blivedm 走 GitHub，见 requirements 注释）
"""

from __future__ import annotations

import argparse
import asyncio
import json
import os
import sys
import time
import urllib.error
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from typing import Optional

# ---------------------------------------------------------------- 配置

DEFAULT_URL = "http://127.0.0.1:18080/api/v1/external/chat"

ROOM_ID = int(os.environ.get("BILI_ROOM_ID", "0") or "0")
SESSDATA = os.environ.get("BILI_SESSDATA", "").strip()
TARGET = os.environ.get("LIVE2D_AI_URL", DEFAULT_URL).strip()
TOKEN = os.environ.get("EXTERNAL_INPUT_TOKEN", "").strip()
PREFIX = os.environ.get("SIDECAR_PREFIX", "")
DRY_RUN = os.environ.get("SIDECAR_DRY_RUN", "") == "1"
MIN_INTERVAL_MS = int(os.environ.get("SIDECAR_MIN_INTERVAL_MS", "1000") or "0")

# 单条注入文本上限（字符）。服务端渲染后还会再判一次 2000，这里先做一道。
MAX_TEXT_CHARS = 500

# 注入用线程池：HTTP POST 是阻塞的，不能卡住 blivedm 的 asyncio 心跳。
_EXECUTOR = ThreadPoolExecutor(max_workers=2, thread_name_prefix="l2d-inject")

# SEND_GIFT_V2 落到 unknown-cmd 的次数（仅当 blivedm 不支持该结构时才会 > 0）。
# 随每次注入作为可选字段 v2_ignored 上报，服务端放进 state_json。
V2_IGNORED = 0
_THROTTLE: Optional["Throttle"] = None

# ---------------------------------------------------------------- 节流

class Throttle:
    """最小节流：两条注入之间至少间隔 min_interval_ms（0 = 关闭）。"""

    def __init__(self, min_interval_ms: int):
        self.min_interval_ms = max(0, int(min_interval_ms))
        self._last_ms: Optional[float] = None
        self.dropped = 0

    def allow(self, now_ms: float) -> bool:
        if self.min_interval_ms <= 0:
            return True
        if self._last_ms is None or now_ms - self._last_ms >= self.min_interval_ms:
            self._last_ms = now_ms
            return True
        self.dropped += 1
        return False

def _throttle() -> "Throttle":
    global _THROTTLE
    if _THROTTLE is None:
        _THROTTLE = Throttle(MIN_INTERVAL_MS)
    return _THROTTLE

# ---------------------------------------------------------------- 清洗 / 注入

def clean_text(raw: str, limit: int = MAX_TEXT_CHARS) -> str:
    """把一条外部文本清洗成**单行安全**文本。

    - 去掉换行 / 制表 / 控制字符（它们会破坏日志与气泡排版）；
    - 压缩连续空白；
    - 截断到 limit 字符（按字符，不按字节；中文也算 1）。
    """
    if not raw:
        return ""
    # 保留可打印字符，其余（含换行/制表及 C0/C1 控制符）替换为空格。
    cleaned = "".join(ch if ch.isprintable() else " " for ch in raw)
    cleaned = " ".join(cleaned.split())
    if len(cleaned) > limit:
        cleaned = cleaned[:limit]
    return cleaned

def build_body(text: str) -> dict:
    """构造注入请求体（纯函数；--selftest 据此断言 v2_ignored 确实被上报）。"""
    body = {"text": text, "v2_ignored": V2_IGNORED}
    if TOKEN:
        body["token"] = TOKEN
    return body

def _post(text: str) -> None:
    """阻塞 POST 到本机 Live2D-Ai 端点。失败只 log，不抛（sidecar 不能因一次失败退出）。"""
    data = json.dumps(build_body(text), ensure_ascii=False).encode("utf-8")
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
    """清洗后按节流策略异步提交（在线程池里发 HTTP，不阻塞事件循环）。"""
    text = clean_text(text)
    if not text:
        return
    line = PREFIX + text
    if DRY_RUN:
        print(f"[sidecar][dry-run] {line}")
        return
    throttle = _throttle()
    if not throttle.allow(time.monotonic() * 1000.0):
        print(
            f"[sidecar] 节流丢弃（min_interval_ms={throttle.min_interval_ms}，"
            f"累计丢弃 {throttle.dropped}）：{line}",
            file=sys.stderr,
        )
        return
    loop = asyncio.get_running_loop()
    loop.run_in_executor(_EXECUTOR, _post, line)


# ---------------------------------------------------------------- blivedm 回调

def _is_unknown_gift_v2(cmd: str, cmd_table) -> bool:
    """纯逻辑：这条 cmd 是不是「本机 blivedm 不认的 SEND_GIFT_V2」。

    - cmd_table 是 blivedm 的 cmd -> 回调表；表里有 SEND_GIFT_V2 的版本会把它
      路由到 _on_gift（经典 + v2 同一个回调），不算「忽略」；
    - 表里没有（旧版）才是真忽略，需要计数 + log。
    """
    return cmd.split(":")[0] == "SEND_GIFT_V2" and "SEND_GIFT_V2" not in cmd_table

def _note_v2_ignored(cmd: str) -> None:
    """SEND_GIFT_V2 落到「本地 blivedm 不认」的兜底：计数 + 一行日志，不注入。"""
    global V2_IGNORED
    V2_IGNORED += 1
    print(
        f"[sidecar] 忽略 cmd={cmd}（本地 blivedm 未支持 SEND_GIFT_V2；"
        f"v2_ignored={V2_IGNORED}）",
        file=sys.stderr,
    )

def _make_handler(blivedm_mod):
    """构造 blivedm handler（延迟到运行时才需要 blivedm，--selftest 不必安装它）。"""

    class Live2DHandler(blivedm_mod.BaseHandler):
        """只把弹幕 / 礼物送进 Live2D-Ai；其余 cmd 仅 log。"""

        # DANMU_MSG
        def _on_danmaku(self, client, message) -> None:
            # message.content = 弹幕原文；message.uname = 发送者昵称。
            submit(f"弹幕 {message.uname}：{message.content}")

        # SEND_GIFT（经典结构）；blivedm 支持 SEND_GIFT_V2 时也会走这里（batch 展开后逐条）。
        def _on_gift(self, client, message) -> None:
            if getattr(message, "num", 0) <= 0:
                return
            submit(f"{message.uname} 送出 {message.gift_name} x{message.num}")

        # 明确忽略的高频事件（不 log，避免刷屏）。
        def _on_heartbeat(self, client, message) -> None:
            return

        # 当前 blivedm 的 BaseHandler.handle 对未知 cmd 只内部 log 一次就 return，
        # **不会**调用 _on_unknown_cmd——所以 v2 兜底必须在这里看 cmd 表：
        # 表里有 SEND_GIFT_V2 的版本由 super().handle() 复用 _on_gift（经典 + v2
        # 同一个回调）；表里没有的旧版才计数 + log，且不注入。
        def handle(self, client, command) -> None:
            cmd = command.get("cmd") or ""
            table = getattr(type(self), "_CMD_CALLBACK_DICT", {})
            if _is_unknown_gift_v2(cmd, table):
                _note_v2_ignored(cmd)
                return
            return super().handle(client, command)

        # 旧版 / 分支版 blivedm 若仍把未知 cmd 派发到这里，兜住 log（不注入）。
        def _on_unknown_cmd(self, client, command, message) -> None:
            if command == "SEND_GIFT_V2":
                _note_v2_ignored(command)
            else:
                print(f"[sidecar] 忽略 cmd={command}", file=sys.stderr)

    return Live2DHandler()

# ---------------------------------------------------------------- 入口

async def _run() -> None:
    if ROOM_ID <= 0:
        print("[sidecar] 请设置 BILI_ROOM_ID（真实房间号）", file=sys.stderr)
        raise SystemExit(2)

    try:
        import blivedm  # noqa: PLC0415 - 延迟导入：--selftest 不需要它
        import aiohttp  # noqa: PLC0415
    except ImportError as e:
        print(f"[sidecar] 缺少依赖：{e}；请 pip install -r requirements.txt", file=sys.stderr)
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
    client.set_handler(_make_handler(blivedm))
    client.start()
    print(
        f"[sidecar] 已连接房间 {ROOM_ID} -> {TARGET}"
        f"（dry_run={DRY_RUN}, min_interval_ms={MIN_INTERVAL_MS}）"
    )
    try:
        await client.join()
    finally:
        await client.stop_and_close()
        await session.close()
        _EXECUTOR.shutdown(wait=False)

def selftest() -> int:
    """离线自检：清洗 / 节流 / 干跑。不需要 blivedm / aiohttp / 网络 / 服务端。"""
    global DRY_RUN, MIN_INTERVAL_MS, _THROTTLE, V2_IGNORED
    checks = []

    def check(cond: bool, label: str) -> None:
        if not cond:
            print(f"[sidecar][selftest] FAIL: {label}", file=sys.stderr)
            raise SystemExit(1)
        checks.append(label)

    # 清洗
    check(clean_text("a\nb\tc") == "a b c", "clean_text 换行/制表 -> 空格并压空白")
    check(clean_text("  x   y  ") == "x y", "clean_text 压空白 / 去首尾")
    check(clean_text("啊" * 600) == "啊" * MAX_TEXT_CHARS, "clean_text 截断到上限")
    check(clean_text("") == "", "clean_text 空串 -> 空串")

    # 节流（缺省 1000ms：首条放行，窗口内拒绝，满窗放行）
    t = Throttle(1000)
    check(t.allow(0.0) is True, "节流 1000ms：首条放行")
    check(t.allow(500.0) is False, "节流 1000ms：500ms 内拒绝")
    check(t.allow(999.0) is False, "节流 1000ms：999ms 拒绝")
    check(t.allow(1000.0) is True, "节流 1000ms：满 1000ms 放行")
    check(t.dropped == 2, "节流：丢弃计数 = 2")
    t0 = Throttle(0)
    check(t0.allow(0.0) and t0.allow(0.0), "节流 0 = 关（连发放行）")

    # 上报体：v2_ignored 与 text 都在（服务端据此写 state_json）
    V2_IGNORED = 3
    body = build_body("hi")
    check(body["v2_ignored"] == 3, "上报体带 v2_ignored 累计值")
    check(body["text"] == "hi", "上报体 text 原样")
    V2_IGNORED = 0

    # SEND_GIFT_V2 兜底判定：本地 blivedm 的 cmd 表里有没有它
    check(_is_unknown_gift_v2("SEND_GIFT_V2", {}), "旧版 blivedm：SEND_GIFT_V2 算忽略")
    check(
        not _is_unknown_gift_v2("SEND_GIFT_V2", {"SEND_GIFT_V2": None}),
        "支持 v2 的 blivedm：不忽略（走 _on_gift）",
    )
    check(
        _is_unknown_gift_v2("SEND_GIFT_V2:abc", {}),
        "带 : 后缀的 cmd 同样识别",
    )
    check(not _is_unknown_gift_v2("DANMU_MSG", {}), "其它 cmd 不算 v2 忽略")

    # 干跑 submit：只打印，不发 HTTP
    DRY_RUN = True
    MIN_INTERVAL_MS = 0
    _THROTTLE = None
    submit("selftest 弹药\n第二行")

    print(f"[sidecar] selftest OK（{len(checks)} 项断言）")
    return 0

def main(argv=None) -> None:
    global MIN_INTERVAL_MS, DRY_RUN, _THROTTLE
    parser = argparse.ArgumentParser(description="B 站弹幕 -> Live2D-Ai 注入 sidecar")
    parser.add_argument(
        "--min-interval-ms",
        type=int,
        default=MIN_INTERVAL_MS,
        help="两条注入之间的最小间隔（毫秒）；0 = 关闭节流（缺省 1000）",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="只打印不发送（仍会连 B 站收弹幕；完全离线请用 --selftest）",
    )
    parser.add_argument(
        "--selftest",
        action="store_true",
        help="跑离线自检（清洗 / 节流 / 干跑）后退出；不需要依赖与网络",
    )
    args = parser.parse_args(argv)
    MIN_INTERVAL_MS = max(0, args.min_interval_ms)
    _THROTTLE = None
    DRY_RUN = DRY_RUN or args.dry_run
    if args.selftest:
        raise SystemExit(selftest())
    try:
        asyncio.run(_run())
    except KeyboardInterrupt:
        print("[sidecar] 退出")

if __name__ == "__main__":
    main()
