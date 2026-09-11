#!/usr/bin/env python3
"""核心链路端到端验收 - 2026-09-11 基线。

核心链路（用户裁决，2026-09-11）：
    文本 -> LLM 纯对话 -> TTS -> 驱动口型 -> Live2D 皮套渲染（+ 前端 UI）

本脚本**只驱动公开 HTTP/WS 接口**，不读数据库、不看内存，因此它验证的是
「用户真能走通的那条路」，而不是「代码里存在这些函数」。它同时是**回归**：
任何一环断掉（TTS 不再返音频、音量字段丢失、动作投影又冒出来）都会非 0 退出。

每一跳的判据与它防的是什么：

  1. 服务活着             GET /api/v1/app/status -> 200，llm/tts 均已配置
  2. 前端在             GET /app/ -> 200（Flutter Web 产物，唯一入口）
  3. 渲染面在             GET /render -> 200（Live2D 渲染 iframe）
  4. 模型资产在          GET 模型 runtime 资产 -> 200
  5. 对话受理             POST /api/v1/chat -> 200 accepted + epoch
  6. LLM 有正文          WS text_delta 带非空 text（**纯对话**：没有工具调用）
  7. TTS 有产物          WS audio 帧 >= 1，且 sampleRate / epoch 齐全
  8. 口型有电平源        WS audio 帧带 data.volume（口型的驱动源；缺了就只能
                         靠客户端自己从 PCM 算，服务端静音时还会归零）
  9. 本轮收口            WS turn_state{status} 到达（前端解锁输入框的唯一信号）
 10. 没有动作投影        WS 全程**不得**出现 action_state（2026-09-11 用户裁决：
                         LLM 不暴露任何工具，动作系统已从前后端抹去）
 11. 句子边界配对        每句恰好一个 start（首块首片）与一个 end（末块末片）。
                         这是前端「按句封 WAV 交给 <audio>」的前提；旧实现按
                         epoch 记账推 start，实测 0 start / 13 end，会把一句
                         切成十几段播（断续）。
                         （附带诊断：空末块的句界帧——句子样本数是
                         `audio_chunk_samples` 整数倍时，末块 0 样本，
                         必须仍有一帧把 end 送到；实测相当常见。）

用法：
    python3 scripts/verify_core_chain.py
    python3 scripts/verify_core_chain.py --base http://127.0.0.1:18080 \\
        --text "你好" --timeout 90

退出码：全部通过 0；任一判据失败 1；连不上服务 2。
"""

from __future__ import annotations

import argparse
import base64
import json
import os
import socket
import struct
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

# ---------------------------------------------------------------- 输出小工具

GREEN = "\033[32m"
RED = "\033[31m"
DIM = "\033[2m"
RESET = "\033[0m"


class Report:
    """逐跳记录结果；最后统一打印，失败时也把已跑到的部分打全。"""

    def __init__(self) -> None:
        self.rows: list[tuple[str, bool, str]] = []

    def ok(self, hop: str, detail: str = "") -> None:
        self.rows.append((hop, True, detail))

    def fail(self, hop: str, detail: str) -> None:
        self.rows.append((hop, False, detail))

    @property
    def failed(self) -> list[tuple[str, bool, str]]:
        return [r for r in self.rows if not r[1]]

    def dump(self) -> None:
        print()
        print("核心链路逐跳结果")
        print("-" * 72)
        for hop, good, detail in self.rows:
            mark = f"{GREEN}OK  {RESET}" if good else f"{RED}FAIL{RESET}"
            print(f"  {mark}  {hop:<22} {DIM}{detail}{RESET}")
        print("-" * 72)
        if self.failed:
            print(f"{RED}{len(self.failed)} 跳失败{RESET}")
        else:
            print(f"{GREEN}核心链路闭环：{len(self.rows)} 跳全部通过{RESET}")


# ---------------------------------------------------------------- HTTP

def http_get(base: str, path: str, timeout: float = 10.0):
    req = urllib.request.Request(base + path, method="GET")
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            return resp.status, resp.read()
    except urllib.error.HTTPError as e:
        return e.code, e.read()
    except Exception as e:  # noqa: BLE001 - 连不上与超时都归为不可达
        return None, str(e).encode()


def http_post_json(base: str, path: str, payload: dict, timeout: float = 15.0):
    data = json.dumps(payload).encode()
    req = urllib.request.Request(
        base + path,
        data=data,
        method="POST",
        headers={
            "Content-Type": "application/json",
            # mutating 请求需要 Origin（与浏览器同源行为一致；否则 403
            # origin_required——那是安全设计，不是缺陷）。
            "Origin": base,
        },
    )
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            return resp.status, resp.read()
    except urllib.error.HTTPError as e:
        return e.code, e.read()
    except Exception as e:  # noqa: BLE001
        return None, str(e).encode()


# ---------------------------------------------------------------- 极简 WS 客户端

class WsClient:
    """stdlib 的最小 WebSocket 客户端（只读文本帧）。

    为什么自己写：本仓库运行时依赖受严格限制，而验收脚本不该为了一次读取
    引入第三方包。服务端 -> 客户端的方向是**不需要掩码**的，解析很简单；
    客户端 -> 服务端只发一个空帧（本协议是单向事件流，服务端不读客户端帧）。
    """

    def __init__(self, host: str, port: int, path: str, origin: str) -> None:
        self.sock = socket.create_connection((host, port), timeout=10)
        self.sock.settimeout(0.5)
        key = base64.b64encode(os.urandom(16)).decode()
        req = (
            f"GET {path} HTTP/1.1\r\n"
            f"Host: {host}:{port}\r\n"
            "Upgrade: websocket\r\n"
            "Connection: Upgrade\r\n"
            f"Sec-WebSocket-Key: {key}\r\n"
            "Sec-WebSocket-Version: 13\r\n"
            # 服务端要求 Origin（否则 403 origin_required）。
            f"Origin: {origin}\r\n"
            "\r\n"
        )
        self.sock.sendall(req.encode())
        self.buf = b""
        # 读完握手响应头
        while b"\r\n\r\n" not in self.buf:
            chunk = self.sock.recv(4096)
            if not chunk:
                raise ConnectionError("WS 握手期间连接被关闭")
            self.buf += chunk
        head, self.buf = self.buf.split(b"\r\n\r\n", 1)
        status_line = head.split(b"\r\n", 1)[0].decode(errors="replace")
        if "101" not in status_line:
            raise ConnectionError(f"WS 升级失败：{status_line}")

    def _recv_exact(self, n: int) -> bytes:
        while len(self.buf) < n:
            try:
                chunk = self.sock.recv(65536)
            except socket.timeout:
                continue
            if not chunk:
                raise ConnectionError("连接关闭")
            self.buf += chunk
        out, self.buf = self.buf[:n], self.buf[n:]
        return out

    def read_frame(self, deadline: float):
        """读一帧文本；超时返回 None。控制帧（ping）就地回 pong。"""
        while time.time() < deadline:
            try:
                header = self._recv_exact(2)
            except (socket.timeout, ConnectionError):
                if time.time() >= deadline:
                    return None
                continue
            fin = header[0] & 0x80
            opcode = header[0] & 0x0F
            masked = header[1] & 0x80
            length = header[1] & 0x7F
            if length == 126:
                length = struct.unpack(">H", self._recv_exact(2))[0]
            elif length == 127:
                length = struct.unpack(">Q", self._recv_exact(8))[0]
            mask = self._recv_exact(4) if masked else b""
            payload = self._recv_exact(length) if length else b""
            if masked:
                payload = bytes(b ^ mask[i % 4] for i, b in enumerate(payload))
            if opcode == 0x9:  # ping -> pong（服务端会周期性 ping）
                self._send_control(0xA, payload)
                continue
            if opcode in (0x8, 0xA):  # close / pong
                continue
            if opcode == 0x1 and fin:  # 完整文本帧
                return payload.decode("utf-8", errors="replace")
        return None

    def _send_control(self, opcode: int, payload: bytes = b"") -> None:
        mask = os.urandom(4)
        masked = bytes(b ^ mask[i % 4] for i, b in enumerate(payload))
        header = bytes([0x80 | opcode, 0x80 | len(payload)])
        try:
            self.sock.sendall(header + mask + masked)
        except OSError:
            pass

    def close(self) -> None:
        try:
            self._send_control(0x8)
            self.sock.close()
        except OSError:
            pass


# ---------------------------------------------------------------- 主流程

def parse_ws_url(base: str) -> tuple[str, int, str]:
    u = urllib.parse.urlparse(base)
    host = u.hostname or "127.0.0.1"
    port = u.port or (443 if u.scheme == "https" else 80)
    return host, port, "/ws/state"


def run(base: str, text: str, timeout: float) -> int:
    rep = Report()
    host, port, ws_path = parse_ws_url(base)

    # 1. 服务活着 + LLM/TTS 已配置
    status, body = http_get(base, "/api/v1/app/status")
    if status != 200:
        rep.fail("服务可达", f"/api/v1/app/status -> {status}")
        rep.dump()
        return 2
    try:
        info = json.loads(body)
    except ValueError:
        rep.fail("服务可达", "status 不是合法 JSON")
        rep.dump()
        return 2
    llm_ok = bool(info.get("llm", {}).get("configured"))
    tts_ok = bool(info.get("tts", {}).get("configured"))
    rep.ok("服务可达", f"uptime={info.get('uptime_s')}s dev_mode={info.get('dev_mode')}")
    if llm_ok:
        rep.ok("LLM 已配置", f"{info['llm'].get('model')} @ {info['llm'].get('base_url')}")
    else:
        rep.fail("LLM 已配置", "llm.configured=false —— 没有 LLM 就没有链路")
    if tts_ok:
        rep.ok("TTS 已配置", f"{info['tts'].get('base_url')} voice={info['tts'].get('voice')}")
    else:
        rep.fail("TTS 已配置", "tts.configured=false —— 没有 TTS 就没有语音与口型")
    model_id = info.get("active_model_id")
    if model_id:
        rep.ok("皮套已装载", f"active_model_id={model_id}")
    else:
        rep.fail("皮套已装载", "active_model_id 为空")

    # 2/3. 前端与渲染面
    for hop, path in (("前端 UI 产物", "/app/"), ("渲染面", "/render")):
        st, _ = http_get(base, path)
        if st == 200:
            rep.ok(hop, f"GET {path} -> 200")
        else:
            rep.fail(hop, f"GET {path} -> {st}")

    # 4. 模型资产（注意真实路径是 /models/<id>/runtime/...）
    if model_id:
        asset = f"/models/{model_id.split('_')[0]}/runtime/{model_id.split('_')[0]}.model3.json"
        st, _ = http_get(base, asset)
        if st == 200:
            rep.ok("模型资产可达", f"GET {asset} -> 200")
        else:
            rep.fail("模型资产可达", f"GET {asset} -> {st}")

    # 5. 开 WS（先连再发，避免漏掉首帧）
    try:
        ws = WsClient(host, port, ws_path, base)
    except Exception as e:  # noqa: BLE001
        rep.fail("WS 已连接", f"{ws_path} 升级失败：{e}")
        rep.dump()
        return 2
    rep.ok("WS 已连接", ws_path)

    try:
        status, body = http_post_json(base, "/api/v1/chat", {"text": text})
        if status == 200:
            accepted = json.loads(body)
            rep.ok("对话已受理", f"accepted={accepted.get('accepted')} epoch={accepted.get('epoch')}")
        else:
            rep.fail("对话已受理", f"POST /api/v1/chat -> {status} {body[:120]!r}")
            rep.dump()
            return 1

        # 6~10. 收帧
        deadline = time.time() + timeout
        got_text: list[str] = []
        audio_frames = 0
        audio_rate: int | None = None
        audio_volume_seen = False
        turn_status: str | None = None
        action_frames = 0
        audio_b64_ok = False
        audio_muted = False
        boundary_only_frames = 0
        # 句子边界：每片记 (start, end)。**句界正确性是前端能否按句播放的前提**
        # （前端要以 end 为闸门把这一句的片封成一个 WAV 交给 <audio>）。
        boundaries: list[tuple[bool, bool]] = []
        while time.time() < deadline:
            raw = ws.read_frame(deadline)
            if raw is None:
                break
            try:
                frame = json.loads(raw)
            except ValueError:
                continue
            ftype = frame.get("type")
            data = frame.get("data") or {}
            if ftype == "text_delta":
                t = data.get("text")
                if isinstance(t, str) and t:
                    got_text.append(t)
            elif ftype == "audio":
                audio_frames += 1
                boundaries.append((data.get("start") is True, data.get("end") is True))
                # 真实帧字段是 snake_case（`sample_rate`），与 Rust 侧
                # `serde` 序列化一致；`audio` 是 base64 PCM 本体。
                if data.get("sample_rate"):
                    audio_rate = data["sample_rate"]
                if isinstance(data.get("audio"), str) and data["audio"]:
                    audio_b64_ok = True
                # 空音频体的**句界帧**：只带 start/end 标记、不带 PCM。
                # 引擎允许某句末块 0 样本（句子样本数是 `audio_chunk_samples`
                # 整数倍时必然出现），此时必须仍有一帧把 `end` 送到——否则
                # 前端永远等不到句尾闸门，那一句播不出来（2026-09-11 修）。
                if data.get("audio") == "":
                    boundary_only_frames += 1
                if data.get("muted") is True:
                    audio_muted = True
                if data.get("volume") is not None:
                    audio_volume_seen = True
            elif ftype == "turn_state":
                turn_status = data.get("status")
                break
            elif ftype == "action_state":
                action_frames += 1
    finally:
        ws.close()

    joined = "".join(got_text)
    if joined:
        rep.ok("LLM 有正文", f"{len(joined)} 字：{joined[:40]!r}")
    else:
        rep.fail("LLM 有正文", "整轮没有收到任何 text_delta 文字")

    if audio_frames:
        rep.ok("TTS 有产物", f"{audio_frames} 帧音频")
    else:
        rep.fail("TTS 有产物", "整轮没有收到任何 audio 帧（TTS 未产出）")

    if audio_b64_ok:
        rep.ok("音频有本体", "audio 帧带非空 base64 PCM")
    elif audio_frames:
        rep.fail("音频有本体", "有 audio 帧但没有 PCM 本体（帧形状变了？）")

    # 空末块的句界帧（诊断，不是判据）：它只在「句子样本数正好是
    # 2400 字节整数倍」时出现，所以不该拿它当通过条件——但一旦出现，就证明
    # 修复的那条路径真的在跑。
    if boundary_only_frames:
        rep.ok(
            "空末块句界帧",
            f"{boundary_only_frames} 帧只有边界、没有 PCM（修复的空末块路径生效）",
        )

    # 采样率是**必查项**：它一旦与 TTS 实际采样率不符，浏览器就要逐片重采样，
    # 听感是每 20ms 一次的咔哒（2026-09-11 修过的那个 bug）。缺字段也要判失败，
    # 不允许静默跳过——「跳过」等于假装验过了。
    if audio_frames == 0:
        pass  # 上面已判 TTS 无产物
    elif audio_rate is None:
        rep.fail("音频带采样率", "audio 帧缺 sample_rate —— 播放侧无从判断要不要重采样")
    else:
        want = info.get("tts", {}).get("sample_rate")
        if want and audio_rate != want:
            rep.fail(
                "采样率一致",
                f"音频 {audio_rate} != TTS 配置 {want}（非整数倍会逐片重采样 → 咔哒声）",
            )
        elif want:
            rep.ok("采样率一致", f"音频 {audio_rate} == TTS 配置 {want}")
        else:
            rep.ok("音频带采样率", f"sample_rate={audio_rate}")

    if audio_muted:
        rep.ok("静音开关", "服务端处于静音（PCM 置零、volume 仍为真实值）")

    # 句子边界：**每句一对 start/end**，且首片是 start、末片是 end。
    #
    # 为什么这条必须查：引擎明确「每句音频恰好一个 final_chunk=true」，前端据此
    # 把一句的片封成一个 WAV 交给 <audio>。而**旧实现里 `start` 是轮级的**
    # （`broadcast_audio_frames` 用 `previous != token` 推导，整轮只出现一次），
    # `end` 却是每次广播调用的末片——两者不对称，前端按「start→end」攒句会攒错。
    if not boundaries:
        pass  # 上面已判 TTS 无产物
    else:
        starts = sum(1 for s, _ in boundaries if s)
        ends = sum(1 for _, e in boundaries if e)
        first_is_start = boundaries[0][0]
        last_is_end = boundaries[-1][1]
        if starts == ends and starts >= 1 and first_is_start and last_is_end:
            rep.ok("句子边界配对", f"{ends} 句，每句 start/end 各一，首片 start、末片 end")
        else:
            rep.fail(
                "句子边界配对",
                f"start={starts} end={ends} 首片start={first_is_start} 末片end={last_is_end}"
                f" —— 句界不对称，前端无法按句封 WAV（会攒错或断续）",
            )

    if audio_volume_seen:
        rep.ok("口型有电平源", "audio 帧带 data.volume")
    else:
        rep.fail("口型有电平源", "audio 帧没有 data.volume —— 口型只能退化到客户端估算")

    if turn_status:
        rep.ok("本轮已收口", f"turn_state status={turn_status}")
    else:
        rep.fail("本轮已收口", "超时未收到 turn_state（前端输入框会锁死）")

    if action_frames == 0:
        rep.ok("无动作投影", "全程 0 个 action_state（LLM 纯对话）")
    else:
        rep.fail("无动作投影", f"出现 {action_frames} 个 action_state —— 动作系统又接线了")

    rep.dump()
    return 0 if not rep.failed else 1


def main() -> int:
    ap = argparse.ArgumentParser(description="核心链路端到端验收")
    ap.add_argument(
        "--base",
        default=os.environ.get("LIVE2D_AI_BASE", "http://127.0.0.1:18080"),
        help="服务基址（默认取环境变量 LIVE2D_AI_BASE 或 127.0.0.1:18080）",
    )
    ap.add_argument("--text", default="用一句话打个招呼。", help="发给 LLM 的探测文本")
    ap.add_argument("--timeout", type=float, default=90.0, help="等待本轮收口的秒数")
    args = ap.parse_args()
    return run(args.base.rstrip("/"), args.text, args.timeout)


if __name__ == "__main__":
    sys.exit(main())
