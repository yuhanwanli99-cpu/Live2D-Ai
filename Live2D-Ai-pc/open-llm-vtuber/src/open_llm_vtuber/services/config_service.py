import os
import tempfile
from typing import Any

from ruamel.yaml import YAML


class ConfigService:
    """conf.yaml / persona.yaml 的原子读写服务。"""

    def __init__(self, config_abs: str, persona_path: str):
        self.config_abs = config_abs
        self.persona_path = persona_path

    def load_yaml(self) -> dict[str, Any]:
        yaml = YAML(typ="rt")
        yaml.preserve_quotes = True
        with open(self.config_abs, "r", encoding="utf-8") as f:
            return yaml.load(f) or {}

    def save_yaml(self, data) -> None:
        yaml = YAML(typ="rt")
        yaml.preserve_quotes = True
        yaml.allow_unicode = True
        fd, tmp_path = tempfile.mkstemp(
            prefix="conf.", suffix=".yaml.tmp", dir=os.path.dirname(self.config_abs)
        )
        try:
            with os.fdopen(fd, "w", encoding="utf-8") as f:
                yaml.dump(data, f)
                f.flush()
                os.fsync(f.fileno())
            os.replace(tmp_path, self.config_abs)
        finally:
            if os.path.exists(tmp_path):
                os.unlink(tmp_path)

    def write_raw_atomic(self, content: str) -> None:
        """把用户原始 YAML 文本原子写入 config_abs。"""
        fd, tmp_path = tempfile.mkstemp(
            prefix="conf.", suffix=".yaml.tmp", dir=os.path.dirname(self.config_abs)
        )
        try:
            with os.fdopen(fd, "w", encoding="utf-8") as f:
                f.write(content)
                f.flush()
                os.fsync(f.fileno())
            os.replace(tmp_path, self.config_abs)
        finally:
            if os.path.exists(tmp_path):
                os.unlink(tmp_path)

    def load_persona(self) -> dict[str, Any]:
        if not os.path.exists(self.persona_path):
            return {}
        yaml = YAML(typ="rt")
        yaml.preserve_quotes = True
        with open(self.persona_path, "r", encoding="utf-8") as f:
            return yaml.load(f) or {}

    def save_persona(self, data) -> None:
        yaml = YAML(typ="rt")
        yaml.preserve_quotes = True
        yaml.allow_unicode = True
        fd, tmp_path = tempfile.mkstemp(
            prefix="persona.", suffix=".yaml.tmp", dir=os.path.dirname(self.persona_path)
        )
        try:
            with os.fdopen(fd, "w", encoding="utf-8") as f:
                yaml.dump(data, f)
                f.flush()
                os.fsync(f.fileno())
            os.replace(tmp_path, self.persona_path)
        finally:
            if os.path.exists(tmp_path):
                os.unlink(tmp_path)
