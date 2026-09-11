#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""SLOP 口头禅挖掘器（离线）——扫聊天历史 → 高频候选 → 人工策展 shared/slop_rules.json。

PLAN-V3 §P2 台词质量治理的第一环：本脚本只做统计挖掘，不改任何数据；
产出候选清单供人工策展（策展产物才是运行时生效的 slop_rules.json）。

用法：
  python scripts/slop_miner.py chat_history/ --role ai --min-count 3 --top 40
  python scripts/slop_miner.py a.json b.jsonl --json candidates.json

支持格式：
  - chat_history_manager 落盘的 .json（消息对象数组，元素含 role/content）；
  - 通用 .jsonl（每行一个 JSON 消息）。
目录会递归收集上述两类文件。

退出码：正常 0；输入路径全部不存在 2。
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections import Counter
from pathlib import Path

# 内置高置信套话家族：只用于报告分桶提示，不自动写进规则表（策展是人工环节）。
CLICHE_FAMILIES: dict[str, str] = {
    "总结腔开头": r"^(?:总而言之|总的来说|综上所述|总之)(?:[，,]|$)",
    "论文腔插入": r"(?:值得注意的是|值得一提的是)[，,:：]?",
    "时代背景": r"在这个[^，。！？!?\n]{0,10}的时代[，,]?",
    "AI自指": r"作为一个?(?:AI|ai|人工智能)(?:助手|助理|语言模型)",
    "客服式收尾": r"(?:希望|愿)(?:这些|这个|以上)?(?:回答|内容|建议|分享)(?:能)?对你有所帮助",
}

# 子句切分：中英句子终止符 + 常见停顿（逗号/顿号/分号）+ 换行
_CLAUSE_SPLIT = re.compile(r"[。！？!?；;\n，,]+")
_LEAD_GRAM = re.compile(r"^[\u4e00-\u9fff]{2,6}")  # 句首中文 2-6 字片段


def iter_messages(path: Path) -> list[dict]:
    """读单个文件里的消息对象列表（.json 数组 / .jsonl 行）。解析失败的行跳过且可见。"""
    messages: list[dict] = []
    if path.suffix == ".jsonl":
        for lineno, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            line = line.strip()
            if not line:
                continue
            try:
                obj = json.loads(line)
            except json.JSONDecodeError as e:
                print(f"[warn] {path.name}:{lineno} 非法 JSON 跳过: {e}", file=sys.stderr)
                continue
            if isinstance(obj, dict):
                messages.append(obj)
        return messages
    if path.suffix == ".json":
        try:
            data = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as e:
            print(f"[warn] {path.name} 解析失败跳过: {e}", file=sys.stderr)
            return []
        return [m for m in data if isinstance(m, dict)]
    return []


def collect_files(inputs: list[str]) -> list[Path]:
    """把输入路径展开为 .json/.jsonl 文件列表（目录递归）。"""
    files: list[Path] = []
    for raw in inputs:
        p = Path(raw)
        if p.is_dir():
            files.extend(sorted(f for f in p.rglob("*") if f.suffix in (".json", ".jsonl")))
        elif p.is_file():
            files.append(p)
        else:
            print(f"[warn] 路径不存在跳过: {p}", file=sys.stderr)
    return files


def mine(
    inputs: list[str],
    role: str = "ai",
    min_count: int = 3,
    top: int = 40,
) -> dict:
    """统计 AI 台词的高频子句 / 句首片段 / 套话家族命中。"""
    clause_counts: Counter[str] = Counter()
    lead_counts: Counter[str] = Counter()
    family_hits: Counter[str] = Counter()
    total_msgs = 0

    families = {name: re.compile(pat) for name, pat in CLICHE_FAMILIES.items()}
    for path in collect_files(inputs):
        for msg in iter_messages(path):
            if msg.get("role") != role:
                continue
            content = msg.get("content")
            if not isinstance(content, str) or not content.strip():
                continue
            total_msgs += 1
            clauses = [c.strip() for c in _CLAUSE_SPLIT.split(content) if len(c.strip()) >= 2]
            clause_counts.update(clauses)
            for c in clauses:
                m = _LEAD_GRAM.match(c)
                if m:
                    lead_counts[m.group(0)] += 1
            for name, rx in families.items():
                if any(rx.search(c) for c in clauses):
                    family_hits[name] += 1

    return {
        "scanned_messages": total_msgs,
        "clauses": [
            {"text": t, "count": n}
            for t, n in clause_counts.most_common(top)
            if n >= min_count
        ],
        "lead_fragments": [
            {"text": t, "count": n}
            for t, n in lead_counts.most_common(top)
            if n >= min_count
        ],
        "cliche_family_hits": dict(sorted(family_hits.items(), key=lambda kv: -kv[1])),
    }


def main() -> int:
    ap = argparse.ArgumentParser(description="SLOP 口头禅离线挖掘器")
    ap.add_argument("inputs", nargs="+", help="聊天历史文件或目录（.json / .jsonl）")
    ap.add_argument("--role", default="ai", help="只统计该 role 的消息（默认 ai）")
    ap.add_argument("--min-count", type=int, default=3, help="候选最低出现次数（默认 3）")
    ap.add_argument("--top", type=int, default=40, help="每类最多展示条数（默认 40）")
    args = ap.parse_args()

    report = mine(args.inputs, role=args.role, min_count=args.min_count, top=args.top)
    if report["scanned_messages"] == 0:
        print("未扫到任何匹配消息——确认路径与 --role 是否正确。", file=sys.stderr)
        return 2

    print(f"扫描消息数（role={args.role}）: {report['scanned_messages']}")
    print(f"\n== 高频子句（≥{args.min_count} 次，策展重点）==")
    for item in report["clauses"]:
        print(f"{item['count']:>5}  {item['text']}")
    print("\n== 高频句首片段（口头禅高发位）==")
    for item in report["lead_fragments"]:
        print(f"{item['count']:>5}  {item['text']}")
    print("\n== 内置套话家族命中 ==")
    if report["cliche_family_hits"]:
        for name, n in report["cliche_family_hits"].items():
            print(f"{n:>5}  {name}")
    else:
        print("（无）")
    print("\n下一步：人工筛选上表 → 编辑 shared/slop_rules.json（运行时 TTS 生效）。")
    return 0


if __name__ == "__main__":
    sys.exit(main())
