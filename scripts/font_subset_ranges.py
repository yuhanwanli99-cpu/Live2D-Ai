#!/usr/bin/env python3
"""从自托管字体子集导出 **码点覆盖表**，供前端门禁测试消费。

## 为什么需要它（2026-09-11，无头浏览器真机点火时抓到）

项目的硬约束是「中文字体必须自托管，否则**断网即豆腐块**」——Flutter Web 的
CanvasKit 取不到设备字体，缺字时引擎会去 `fonts.gstatic.com` 下载回退字体，
**有网时完全看不出来**。

真机点火时实测到一个外部请求：

    GET https://fonts.gstatic.com/s/notosanssc/v37/…88.woff2  [200]

排查发现界面里有**两个字符不在自托管子集**（子集 22 036 码点）：

- `▍`(U+258D)：`ui/message_bubble.dart` 的**流式光标**——**每次流式回复都上屏**；
- `⌘`(U+2318)：`app/app_shortcuts.dart` 的 macOS 快捷键前缀。

两者都已改成不依赖字形的实现（竖条 / ASCII `Cmd`）。本脚本 + 它生成的
`.ranges.txt` + `test/font_subset_test.dart` 组成**门禁**：以后谁再往界面里写
子集外的字符，`flutter test` 会直接红，而不是等到断网才暴露。

## 为什么要有 FNV-1a 哈希

覆盖表是从字体文件**派生**的。若有人换了字体（重新子集化 / 换字族）却没重新
生成覆盖表，测试就会拿旧表放行——比没有门禁更危险。所以文件头记录字体字节数
与 FNV-1a(32) 哈希，测试会重新计算并比对，不一致就要求重新生成。

## 用法

    pip install fonttools brotli        # 仅本脚本需要，不是项目依赖
    python3 scripts/font_subset_ranges.py

    # 或指定文件
    python3 scripts/font_subset_ranges.py shell/flutter/assets/fonts/X.woff2
"""

from __future__ import annotations

import sys
from pathlib import Path

DEFAULT_FONT = "shell/flutter/assets/fonts/NotoSansSC-AiSubset-Regular.woff2"


def fnv1a32(data: bytes) -> int:
    """FNV-1a 32 位。Dart 侧有逐字节等价的实现（测试要重算同一个值）。"""
    h = 0x811C9DC5
    for b in data:
        h ^= b
        h = (h * 0x01000193) & 0xFFFFFFFF
    return h


def coalesce(codepoints: list[int]) -> list[tuple[int, int]]:
    """把有序码点合并成闭区间，压小文件体积（CJK 基本是连续块）。"""
    out: list[tuple[int, int]] = []
    for cp in codepoints:
        if out and cp == out[-1][1] + 1:
            out[-1] = (out[-1][0], cp)
        else:
            out.append((cp, cp))
    return out


def main() -> int:
    font_path = Path(sys.argv[1] if len(sys.argv) > 1 else DEFAULT_FONT)
    if not font_path.is_file():
        print(f"找不到字体：{font_path}", file=sys.stderr)
        return 2

    try:
        from fontTools.ttLib import TTFont
    except ImportError:
        print(
            "需要 fontTools（仅本脚本，非项目依赖）：\n"
            "  pip install fonttools brotli",
            file=sys.stderr,
        )
        return 2

    font = TTFont(str(font_path))
    cmap = font.getBestCmap()
    codepoints = sorted(cmap.keys())
    ranges = coalesce(codepoints)

    raw = font_path.read_bytes()
    out_path = font_path.with_suffix(".ranges.txt")

    lines = [
        "# 自托管字体子集的码点覆盖表 —— **生成文件，别手改**。",
        "#",
        "# 生成：python3 scripts/font_subset_ranges.py",
        "# 消费：shell/flutter/test/font_subset_test.dart（界面文案里出现子集外的",
        "#       字符会让 flutter test 直接红——理由见生成脚本头注）",
        "#",
        f"# font: {font_path.name}",
        f"# bytes: {len(raw)}",
        f"# fnv1a32: {fnv1a32(raw):08x}",
        f"# codepoints: {len(codepoints)}",
        f"# ranges: {len(ranges)}",
        "",
    ]
    lines += [f"{lo:04X}-{hi:04X}" if lo != hi else f"{lo:04X}" for lo, hi in ranges]
    out_path.write_text("\n".join(lines) + "\n", encoding="utf-8")

    print(f"字体      ：{font_path}")
    print(f"码点      ：{len(codepoints)}（{len(ranges)} 个区间）")
    print(f"字节/FNV  ：{len(raw)} / {fnv1a32(raw):08x}")
    print(f"已写出    ：{out_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
