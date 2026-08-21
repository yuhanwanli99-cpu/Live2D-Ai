# -*- coding: utf-8 -*-
"""PC 端 Live2D 模型 zip 上传导入接口。

流程：
  上传 zip → 读入内存 → zip slip / 绝对路径 / 符号链接校验 →
  找到唯一 model3.json → 安全解压到临时目录 → 移动到 live2d-models/<id>/ →
  更新 shared/model_registry.json → 可选热切换所有在线前端。
"""

from __future__ import annotations

import io
import json
import os
import re
import shutil
import tempfile
import zipfile
from pathlib import Path
from typing import Any

from fastapi import APIRouter, File, Form, UploadFile
from loguru import logger
from starlette.responses import JSONResponse

_MODEL_ID_RE = re.compile(r"^[a-z0-9_]{1,64}$")
_MAX_ZIP_BYTES = 200 * 1024 * 1024  # 200MB，防止把内存打爆


def _repo_root() -> Path:
    """由本文件位置推导仓库根（.../Live2D-Ai-pc/open-llm-vtuber/src/open_llm_vtuber）。"""
    return Path(__file__).resolve().parents[4]


def _default_live2d_dir() -> Path:
    return _repo_root() / "Live2D-Ai-pc" / "open-llm-vtuber" / "live2d-models"


def _default_registry_path() -> Path:
    return _repo_root() / "shared" / "model_registry.json"


def _safe_extract_members(zf: zipfile.ZipFile) -> list[zipfile.ZipInfo]:
    """返回可安全解压的 entries；发现 zip slip / 绝对路径 / 符号链接时抛 ValueError。"""
    members: list[zipfile.ZipInfo] = []
    for info in zf.infolist():
        name = info.filename.replace("\\", "/")
        if name.startswith("/") or re.match(r"^[A-Za-z]:", name):
            raise ValueError(f"非法绝对路径: {info.filename}")
        if ".." in Path(name).parts:
            raise ValueError(f"非法路径穿越: {info.filename}")
        if (info.external_attr >> 16) & 0o170000 == 0o120000:
            raise ValueError(f"zip 内不允许符号链接: {info.filename}")
        members.append(info)
    return members


def _find_model3(zf: zipfile.ZipFile, model_id: str | None) -> tuple[str, str]:
    """从 zip 中挑选 model3.json，返回 (zip 内 posix 路径, 实际 model_id)。"""
    candidates: list[str] = []
    for info in zf.infolist():
        name = info.filename.replace("\\", "/")
        if name.endswith(".model3.json"):
            candidates.append(name)

    if not candidates:
        raise ValueError("zip 内没有找到 .model3.json")

    if model_id:
        preferred = [p for p in candidates if Path(p).name == f"{model_id}.model3.json"]
        if preferred:
            chosen = preferred[0]
        else:
            # 用户指定了 id，但 zip 内没有同名文件：按最短路径选一个并保留用户 id
            chosen = min(candidates, key=lambda p: (len(p), p))
        return chosen, model_id

    # 未指定 id：取最短路径的 model3，用其 basename 作为 id
    chosen = min(candidates, key=lambda p: (len(p), p))
    derived = Path(chosen).stem  # x.model3.json -> x.model3
    if derived.endswith(".model3"):
        derived = derived[: -len(".model3")]
    if not _MODEL_ID_RE.match(derived):
        raise ValueError(
            f"无法从文件名 '{Path(chosen).name}' 推导合法 model_id，请显式提供 model_id"
        )
    return chosen, derived


def _atomic_write_json(path: Path, data: dict[str, Any]) -> None:
    directory = path.parent
    directory.mkdir(parents=True, exist_ok=True)
    fd, tmp = tempfile.mkstemp(prefix=".model_registry.", suffix=".tmp", dir=directory)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as f:
            json.dump(data, f, ensure_ascii=False, indent=2)
            f.write("\n")
            f.flush()
            os.fsync(f.fileno())
        os.replace(tmp, path)
    finally:
        if os.path.exists(tmp):
            os.unlink(tmp)


def _load_registry(registry_path: Path) -> dict[str, Any]:
    if not registry_path.exists():
        return {"version": "1.0", "models": []}
    with registry_path.open("r", encoding="utf-8") as f:
        data = json.load(f)
    if not isinstance(data, dict):
        raise ValueError("model_registry.json 顶层必须是 JSON 对象")
    data.setdefault("models", [])
    if not isinstance(data["models"], list):
        raise ValueError("model_registry.json models 必须是数组")
    return data


def _upsert_registry(
    registry_path: Path,
    model_id: str,
    model3_json_path: str,
    display_name: str | None,
) -> None:
    registry = _load_registry(registry_path)
    entry = {
        "id": model_id,
        "name": display_name or model_id,
        "description": f"Imported Live2D model '{model_id}'",
        "model3JsonPath": model3_json_path,
        "thumbnailPath": None,
        "available": True,
        "default": False,
        "restrictPlatform": None,
        "metadata": {"creator": "user", "license": "user", "version": "1.0"},
    }
    models = registry["models"]
    existing_idx = next((i for i, m in enumerate(models) if m.get("id") == model_id), None)
    if existing_idx is None:
        models.append(entry)
    else:
        models[existing_idx] = entry
    _atomic_write_json(registry_path, registry)


def init_model_upload_routes(
    ws_handler=None,
    live2d_models_dir: str | None = None,
    registry_path: str | None = None,
    plugin_manager=None,
) -> APIRouter:
    """注册模型上传接口；测试可注入临时目录/注册表路径。"""
    router = APIRouter()
    live2d_root = Path(live2d_models_dir or _default_live2d_dir()).resolve()
    registry_file = Path(registry_path or _default_registry_path()).resolve()

    def _plugin_reason() -> str | None:
        if plugin_manager is None or plugin_manager.is_enabled("model-assets"):
            return None
        return "model-assets 插件未启用，模型导入不可用"

    @router.post("/api/models/upload")
    async def model_upload(
        file: UploadFile = File(...),
        model_id: str | None = Form(None),
        display_name: str | None = Form(None),
        overwrite: bool = Form(False),
    ):
        reason = _plugin_reason()
        if reason:
            return JSONResponse({"ok": False, "error": reason}, status_code=403)

        if not file.filename or not file.filename.lower().endswith(".zip"):
            return JSONResponse(
                {"ok": False, "error": "只接受 .zip 模型压缩包"}, status_code=400
            )

        if model_id is not None:
            model_id = model_id.strip()
            if not _MODEL_ID_RE.match(model_id):
                return JSONResponse(
                    {"ok": False, "error": "model_id 只能包含小写字母/数字/下划线，长度 1-64"},
                    status_code=400,
                )

        raw = await file.read()
        if len(raw) == 0:
            return JSONResponse({"ok": False, "error": "zip 文件为空"}, status_code=400)
        if len(raw) > _MAX_ZIP_BYTES:
            return JSONResponse(
                {"ok": False, "error": "zip 文件超过 200MB 上限"}, status_code=413
            )

        try:
            zf = zipfile.ZipFile(io.BytesIO(raw))
            members = _safe_extract_members(zf)
            model3_in_zip, final_model_id = _find_model3(zf, model_id)
        except ValueError as e:
            return JSONResponse({"ok": False, "error": str(e)}, status_code=400)
        except zipfile.BadZipFile:
            return JSONResponse({"ok": False, "error": "不是合法的 zip 文件"}, status_code=400)

        target_dir = live2d_root / final_model_id
        if target_dir.exists() and not overwrite:
            return JSONResponse(
                {
                    "ok": False,
                    "error": f"模型 '{final_model_id}' 已存在；要覆盖请传 overwrite=true",
                },
                status_code=409,
            )

        backup_dir: Path | None = None
        tmp_extract: Path | None = None
        try:
            tmp_extract = Path(tempfile.mkdtemp(prefix=".live2d-upload-", dir=live2d_root))
            # 只解压安全成员；目录项 zipfile 会自动创建
            for info in members:
                zf.extract(info, tmp_extract)

            # 处理常见的「zip 内有一层同名目录」：把实际内容根提出来，
            # 避免解压成 live2d-models/<id>/<id>/runtime/...。
            parts = Path(model3_in_zip).parts
            content_root = tmp_extract
            if len(parts) > 1 and (tmp_extract / parts[0]).is_dir():
                content_root = tmp_extract / parts[0]
                model3_rel = Path(*parts[1:]).as_posix()
            else:
                model3_rel = Path(*parts).as_posix()

            if not (content_root / Path(*Path(model3_rel).parts)).is_file():
                return JSONResponse(
                    {"ok": False, "error": "解压后未找到 model3.json"}, status_code=400
                )

            registry_model3_path = f"{final_model_id}/{model3_rel}"

            if target_dir.exists():
                backup_dir = target_dir.with_name(f".{final_model_id}.backup-{os.getpid()}")
                if backup_dir.exists():
                    shutil.rmtree(backup_dir, ignore_errors=True)
                shutil.move(str(target_dir), str(backup_dir))

            shutil.move(str(content_root), str(target_dir))
            # 如果 content_root 是 tmp_extract 的子目录，外层空目录需要清掉
            if content_root != tmp_extract and tmp_extract.exists():
                shutil.rmtree(tmp_extract, ignore_errors=True)
            tmp_extract = None  # 已移走/清理

            # 若 model3 不在最终目录中（理论不可能），拒绝
            if not (target_dir / Path(*Path(model3_rel).parts)).is_file():
                raise RuntimeError("解压后 model3 文件缺失，已回滚")

            _upsert_registry(
                registry_file,
                final_model_id,
                registry_model3_path,
                display_name or final_model_id,
            )

            applied = False
            if ws_handler is not None:
                try:
                    clients = await ws_handler.broadcast_model_change(final_model_id)
                    applied = bool(clients and clients > 0)
                except Exception as e:  # noqa: BLE001 - 热切换失败不否定导入成功
                    logger.warning(f"模型导入成功，但热切换失败（重启后仍可用）: {e}")

            if backup_dir is not None and backup_dir.exists():
                shutil.rmtree(backup_dir, ignore_errors=True)

            return {
                "ok": True,
                "model_id": final_model_id,
                "model3_json_path": registry_model3_path,
                "applied": applied,
            }

        except Exception as e:  # noqa: BLE001 - 任何异常都要恢复现场
            logger.error(f"模型导入失败: {e}")
            # 恢复旧目录
            if backup_dir is not None and backup_dir.exists() and not target_dir.exists():
                shutil.move(str(backup_dir), str(target_dir))
            elif backup_dir is not None and backup_dir.exists() and target_dir.exists():
                shutil.rmtree(backup_dir, ignore_errors=True)
            # 清理半成品
            if tmp_extract is not None and tmp_extract.exists():
                shutil.rmtree(tmp_extract, ignore_errors=True)
            return JSONResponse(
                {"ok": False, "error": f"模型导入失败: {e}"}, status_code=500
            )

    return router
