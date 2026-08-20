# -*- coding: utf-8 -*-
"""角色背景图：上传 / 列表 / 选择 / 删除。

背景图放在 `backgrounds/`（已由 server.py 挂成静态目录 `/bg`），前端把当前选中的
图片铺在 Live2D 画布**下面**（PIXI 画布本身 `backgroundAlpha: 0` 透明），
因此换背景不影响模型渲染，也不需要重启后端。

当前选择存 `background_state.json`（本机状态，见 local_state.py），字段：
    {"current": "my-room.png"}   # null / 缺省 = 透明无背景

安全边界（后端默认 host 0.0.0.0，必须自己把门）：
  - **写操作（上传/选择/删除）只允许本机回环调用**，避免同网段任何人往磁盘塞文件；
  - 扩展名白名单 + 文件头魔数校验（不认识的内容一律拒绝，防"改后缀的可执行文件"）；
  - 单文件 ≤ 12MB；文件名只取 basename 并清洗，杜绝 `../` 逃逸。
"""

from __future__ import annotations

import os
import re
import time
import unicodedata

from fastapi import APIRouter, File, Request, UploadFile
from loguru import logger
from starlette.responses import JSONResponse

from .local_state import JsonState

BACKGROUND_DIR = "backgrounds"
STATE_FILENAME = "background_state.json"

MAX_UPLOAD_BYTES = 12 * 1024 * 1024  # 12MB
ALLOWED_EXTENSIONS = (".png", ".jpg", ".jpeg", ".webp", ".gif")

# 文件头魔数 → 允许的扩展名集合（防止把别的东西改成 .png 传上来）
_MAGIC_SIGNATURES: tuple[tuple[bytes, tuple[str, ...]], ...] = (
    (b"\x89PNG\r\n\x1a\n", (".png",)),
    (b"\xff\xd8\xff", (".jpg", ".jpeg")),
    (b"GIF87a", (".gif",)),
    (b"GIF89a", (".gif",)),
)


def _is_loopback_request(request: Request) -> bool:
    """写操作限本机：与 external_input 共用同一判定语义。"""
    from .external_input import _is_loopback

    return _is_loopback(request.client.host if request.client else None)


def sanitize_filename(raw: str) -> str:
    """把用户文件名清洗成安全的 basename。

    - 只取 basename（去掉任何目录成分，杜绝 `../` 与绝对路径）；
    - NFC 归一化后剔除控制字符与 `/ \\ : * ? " < > |`；
    - 折叠空白为 `_`，长度截到 80；
    - 结果为空 → 用时间戳兜底（扩展名由调用方保证）。
    """
    name = os.path.basename((raw or "").strip().replace("\\", "/"))
    name = unicodedata.normalize("NFC", name)
    name = "".join(ch for ch in name if ch.isprintable())
    name = re.sub(r'[\\/:*?"<>|]+', "_", name)
    name = re.sub(r"\s+", "_", name).strip("._")
    if len(name) > 80:
        stem, ext = os.path.splitext(name)
        name = stem[: 80 - len(ext)] + ext
    return name


def sniff_extension_ok(head: bytes, ext: str) -> bool:
    """文件头是否与扩展名相符。

    `.webp` 的头为 `RIFF....WEBP`（第 8-12 字节），单独判定。
    未命中任何已知签名 → 视为不合法（宁可拒绝，也不落一个来路不明的文件）。
    """
    lowered = ext.lower()
    if lowered == ".webp":
        return head[:4] == b"RIFF" and head[8:12] == b"WEBP"
    for signature, exts in _MAGIC_SIGNATURES:
        if head.startswith(signature):
            return lowered in exts
    return False


def unique_path(directory: str, filename: str) -> str:
    """避免覆盖同名文件：`a.png` → `a-1.png` → `a-2.png`…"""
    stem, ext = os.path.splitext(filename)
    candidate = filename
    index = 1
    while os.path.exists(os.path.join(directory, candidate)):
        candidate = f"{stem}-{index}{ext}"
        index += 1
    return os.path.join(directory, candidate)


def list_background_files(directory: str = BACKGROUND_DIR) -> list[dict]:
    """列出可用背景图（按修改时间倒序，最新上传的排在最前）。"""
    if not os.path.isdir(directory):
        return []
    items: list[dict] = []
    for entry in os.scandir(directory):
        if not entry.is_file():
            continue
        if not entry.name.lower().endswith(ALLOWED_EXTENSIONS):
            continue
        stat = entry.stat()
        items.append(
            {
                "name": entry.name,
                "url": f"/bg/{entry.name}",
                "size": stat.st_size,
                "mtime": int(stat.st_mtime),
            }
        )
    items.sort(key=lambda x: x["mtime"], reverse=True)
    return items


class BackgroundStore:
    """背景图目录 + 当前选择（选择落盘到 background_state.json）。"""

    def __init__(self, directory: str = BACKGROUND_DIR, state_path: str = STATE_FILENAME):
        self.directory = directory
        self._state = JsonState(state_path)
        self.current: str | None = None

    def load(self) -> "BackgroundStore":
        raw = self._state.load({"current": None})
        current = raw.get("current")
        # 选中的文件可能已被用户手工删掉 → 静默回到"无背景"，不要留下坏引用
        if isinstance(current, str) and current:
            if os.path.isfile(os.path.join(self.directory, current)):
                self.current = current
            else:
                logger.info(f"背景图 {current} 已不存在，回退为无背景")
                self.current = None
        else:
            self.current = None
        return self

    def set_current(self, filename: str | None) -> None:
        """设置当前背景（None = 透明无背景），并落盘。"""
        self.current = filename or None
        try:
            self._state.save({"current": self.current})
        except OSError as e:
            logger.warning(f"背景选择落盘失败（本次运行仍生效）: {e}")

    def view(self) -> dict:
        return {
            "files": list_background_files(self.directory),
            "current": self.current,
            "current_url": f"/bg/{self.current}" if self.current else None,
        }


def init_background_routes(store: BackgroundStore) -> APIRouter:
    """构造背景图路由（列表公开只读；上传/选择/删除限本机）。"""
    router = APIRouter()

    def _forbidden() -> JSONResponse:
        return JSONResponse(
            {"ok": False, "error": "背景图管理只能从本机操作"}, status_code=403
        )

    @router.get("/api/background/list")
    async def background_list():
        return JSONResponse({"ok": True, **store.view()})

    @router.post("/api/background/upload")
    async def background_upload(request: Request, file: UploadFile = File(...)):
        if not _is_loopback_request(request):
            return _forbidden()

        raw_name = sanitize_filename(file.filename or "")
        ext = os.path.splitext(raw_name)[1].lower()
        if ext not in ALLOWED_EXTENSIONS:
            return JSONResponse(
                {
                    "ok": False,
                    "error": f"只支持 {'/'.join(e[1:] for e in ALLOWED_EXTENSIONS)} 格式",
                },
                status_code=400,
            )

        content = await file.read()
        if not content:
            return JSONResponse({"ok": False, "error": "文件内容为空"}, status_code=400)
        if len(content) > MAX_UPLOAD_BYTES:
            return JSONResponse(
                {
                    "ok": False,
                    "error": f"文件过大（{len(content) / 1048576:.1f}MB，上限 {MAX_UPLOAD_BYTES // 1048576}MB）",
                },
                status_code=413,
            )
        if not sniff_extension_ok(content[:16], ext):
            return JSONResponse(
                {"ok": False, "error": "文件内容不是有效图片（扩展名与实际格式不符）"},
                status_code=400,
            )

        stem = os.path.splitext(raw_name)[0] or f"bg-{int(time.time())}"
        os.makedirs(store.directory, exist_ok=True)
        target = unique_path(store.directory, f"{stem}{ext}")
        try:
            with open(target, "wb") as f:
                f.write(content)
        except OSError as e:
            logger.error(f"背景图写盘失败: {e}")
            return JSONResponse(
                {"ok": False, "error": f"保存失败: {e}"}, status_code=500
            )

        name = os.path.basename(target)
        store.set_current(name)  # 上传即选中：符合"换背景"的直觉
        logger.info(f"背景图已上传并选中: {name}（{len(content)} bytes）")
        return JSONResponse({"ok": True, "file": name, **store.view()})

    @router.post("/api/background/select")
    async def background_select(request: Request):
        if not _is_loopback_request(request):
            return _forbidden()
        try:
            payload = await request.json()
        except Exception:
            payload = {}
        if not isinstance(payload, dict):
            payload = {}

        raw = payload.get("file")
        if raw in (None, "", False):
            store.set_current(None)
            return JSONResponse({"ok": True, **store.view()})

        name = sanitize_filename(str(raw))
        if not name or not os.path.isfile(os.path.join(store.directory, name)):
            return JSONResponse(
                {"ok": False, "error": f"背景图不存在: {raw}"}, status_code=404
            )
        store.set_current(name)
        return JSONResponse({"ok": True, **store.view()})

    @router.post("/api/background/delete")
    async def background_delete(request: Request):
        if not _is_loopback_request(request):
            return _forbidden()
        try:
            payload = await request.json()
        except Exception:
            payload = {}
        if not isinstance(payload, dict):
            payload = {}

        name = sanitize_filename(str(payload.get("file") or ""))
        target = os.path.join(store.directory, name)
        if not name or not os.path.isfile(target):
            return JSONResponse(
                {"ok": False, "error": f"背景图不存在: {payload.get('file')}"},
                status_code=404,
            )
        try:
            os.remove(target)
        except OSError as e:
            return JSONResponse({"ok": False, "error": f"删除失败: {e}"}, status_code=500)
        if store.current == name:
            store.set_current(None)
        logger.info(f"背景图已删除: {name}")
        return JSONResponse({"ok": True, **store.view()})

    return router
