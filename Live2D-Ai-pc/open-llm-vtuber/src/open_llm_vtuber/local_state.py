# -*- coding: utf-8 -*-
"""本机运行态 JSON 状态（原子写 + 损坏兜底）。

用途：存放"属于这台机器、不属于 conf.yaml 语义"的开关与选择，例如
  - `external_input.json`：外部文字接入开关 / 访问令牌（见 external_input.py）
  - `background_state.json`：当前角色背景图（见 backgrounds.py）

为什么不塞进 conf.yaml：
  - conf.yaml 走 Pydantic 全量校验 + 热重载链路，加字段的代价和风险都更高；
  - 这些状态需要**即时生效**（点一下开关就生效，不重启、不重连）；
  - 它们含本机私有信息（访问令牌、本机图片文件名），本就该 gitignore。

统一放这里的收益：原子写（临时文件 + os.replace + fsync）只实现一次，
所有本机状态共享同一份"写盘不会写出半个文件"的保证。
"""

from __future__ import annotations

import json
import os
import tempfile
from typing import Any

from loguru import logger


class JsonState:
    """一个 JSON 状态文件的读写封装。

    Args:
        path: 状态文件路径（相对路径按进程 cwd 解析，即 open-llm-vtuber/）。
    """

    def __init__(self, path: str):
        self.path = path

    def load(self, default: dict[str, Any] | None = None) -> dict[str, Any]:
        """读取状态；文件不存在/损坏/非对象 → 返回 default 的浅拷贝（不抛错）。"""
        fallback = dict(default or {})
        try:
            if not os.path.exists(self.path):
                return fallback
            with open(self.path, "r", encoding="utf-8") as f:
                raw = json.load(f)
            if not isinstance(raw, dict):
                logger.warning(f"状态文件 {self.path} 不是 JSON 对象，按默认值处理")
                return fallback
            merged = fallback
            merged.update(raw)
            return merged
        except Exception as e:  # noqa: BLE001 - 状态文件损坏不该阻断启动
            logger.warning(f"状态文件 {self.path} 读取失败，按默认值处理: {e}")
            return fallback

    def save(self, data: dict[str, Any]) -> None:
        """原子写入状态（临时文件 → fsync → os.replace）。

        Raises:
            OSError: 目录不可写等落盘失败；调用方决定是否降级为"仅本次运行生效"。
        """
        directory = os.path.dirname(os.path.abspath(self.path)) or "."
        os.makedirs(directory, exist_ok=True)
        prefix = f".{os.path.basename(self.path)}."
        fd, tmp_path = tempfile.mkstemp(prefix=prefix, suffix=".tmp", dir=directory)
        try:
            with os.fdopen(fd, "w", encoding="utf-8") as f:
                json.dump(data, f, ensure_ascii=False, indent=2)
                f.flush()
                os.fsync(f.fileno())
            os.replace(tmp_path, self.path)
        finally:
            if os.path.exists(tmp_path):
                os.unlink(tmp_path)
