# -*- coding: utf-8 -*-
"""SLOP 台词治理——运行时规则加载与应用（PLAN-V3 §P2 台词质量治理）。

管线：离线 scripts/slop_miner.py 扫聊天历史挖出口头禅候选 → 人工策展 shared/slop_rules.json
→ 本模块在 tts_filter 尾部做正则治理。规则**只作用于 TTS 文本**（不影响字幕与记忆，
与 tts_filter 既有语义一致）。

失败语义（永不静默崩溃）：
  - 规则文件缺失 → 空规则，debug 可见，链路照常；
  - 单条正则非法 / 应用异常 → warning 可见并跳过该条，绝不打断语音输出。
"""

from __future__ import annotations

import json
import os
import re
from pathlib import Path

from loguru import logger

from .repo_paths import find_repo_root

ENV_RULES_PATH = "SLOP_RULES_PATH"


class SlopRule:
    """一条策展规则：pattern 命中的片段替换为 replacement。"""

    __slots__ = ("pattern", "replacement", "note")

    def __init__(self, pattern: re.Pattern[str], replacement: str, note: str) -> None:
        self.pattern = pattern
        self.replacement = replacement
        self.note = note


def repo_root() -> Path:
    """仓库根——已收敛至 utils.repo_paths 单源（AUDIT P-1）。保留旧名兼容。"""
    return find_repo_root()


def default_rules_path() -> Path:
    """规则表路径：环境变量 SLOP_RULES_PATH 优先，缺省 <repo>/shared/slop_rules.json。"""
    env = os.environ.get(ENV_RULES_PATH)
    if env:
        return Path(env)
    return repo_root() / "shared" / "slop_rules.json"


def load_slop_rules(path: Path | str | None = None) -> tuple[SlopRule, ...]:
    """读取并预编译规则表。

    - 文件缺失 → 空元组（debug 日志可见）；
    - JSON 非法 → 空元组 + warning；
    - 单条缺 pattern / 正则编译失败 → 跳过该条 + warning（其余条目不受影响）。
    """
    p = Path(path) if path is not None else default_rules_path()
    if not p.is_file():
        logger.debug(f"[slop] 规则表不存在（跳过治理）: {p}")
        return ()

    try:
        raw = json.loads(p.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as e:
        logger.warning(f"[slop] 规则表解析失败（跳过治理）: {p} — {e}")
        return ()

    entries = raw.get("rules") if isinstance(raw, dict) else None
    if not isinstance(entries, list):
        logger.warning(f"[slop] 规则表格式非法（缺少 rules 数组）: {p}")
        return ()

    rules: list[SlopRule] = []
    for i, entry in enumerate(entries):
        if not isinstance(entry, dict) or not isinstance(entry.get("pattern"), str):
            logger.warning(f"[slop] 规则 #{i} 缺少 pattern 字符串，跳过: {p}")
            continue
        try:
            compiled = re.compile(entry["pattern"])
        except re.error as e:
            logger.warning(f"[slop] 规则 #{i} 正则非法，跳过: {entry['pattern']!r} — {e}")
            continue
        replacement = entry.get("replacement", "")
        note = entry.get("note", "")
        if not isinstance(replacement, str):
            replacement = ""
        rules.append(SlopRule(compiled, replacement, str(note)))
    logger.debug(f"[slop] 已加载 {len(rules)}/{len(entries)} 条规则: {p}")
    return tuple(rules)


# ── 运行时缓存：进程内只读盘一次；测试可用 reset/reload 强制刷新 ──
_runtime_rules: tuple[SlopRule, ...] | None = None


def reload_runtime_rules(path: Path | str | None = None) -> tuple[SlopRule, ...]:
    """强制重读规则表并刷新缓存（服务启动后改了 rules.json 也无需重启）。"""
    global _runtime_rules
    _runtime_rules = load_slop_rules(path)
    return _runtime_rules


def reset_runtime_rules() -> None:
    """清空缓存（下次应用时重新按默认路径懒加载；测试隔离用）。"""
    global _runtime_rules
    _runtime_rules = None


def _get_runtime_rules() -> tuple[SlopRule, ...]:
    if _runtime_rules is None:
        return reload_runtime_rules()
    return _runtime_rules


def apply_slop_rules(text: str, rules: tuple[SlopRule, ...]) -> str:
    """顺序应用规则；单条异常只影响该条（warning 可见），最后清理残留的行首标点。"""
    for rule in rules:
        try:
            text = rule.pattern.sub(rule.replacement, text)
        except Exception as e:  # pragma: no cover — re.sub 对已编译 pattern 极少抛错  # noqa: BLE001
            logger.warning(f"[slop] 规则应用异常（跳过该条）: {rule.note or rule.pattern.pattern!r} — {e}")
    # 治理后可能留下孤立的行首逗号/分号，TTS 会读成怪异停顿
    return re.sub(r"^[，,、；;]+\s*", "", text)


def apply_runtime_slop(text: str) -> str:
    """tts_filter 接线入口：用缓存的运行时规则治理文本。"""
    rules = _get_runtime_rules()
    if not rules:
        return text
    return apply_slop_rules(text, rules)
