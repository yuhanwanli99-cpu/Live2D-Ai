"""R1-persona 残留检查 + 新值命中检查（contract-R1-persona.md §3 P5/P6/P13）

本文件只做 **grep / 文件内容断言**（机械），不触碰任何实现文件——
PersonaConfig.kt / SettingsRepository.kt / ProactiveGreetingClient.kt / conf.yaml /
start.ps1 / index.html x2 / chat-ui.ts / persona.yaml 的内容修改由 C5（coder）执行。

这些断言在 C5 未落地前对现状 = RED（属预期，见 contract-R1-persona.md §5 风险：漏同步/漏改）。
P13 是批次3 联合 grep 门（与 R1-ui 联合），本测试忠实编码该批门的路径清单与『零命中』判定。

路径清单严格来自 contract §3 P13（grep -rn "凛" ...）；判定 token 仅用『凛』（与契约命令一致，
避免 'LinearBurn'/'Link'/'Lines' 等非人设英文子串误命中）。
"""
import os
import re
import unittest

ROOT = os.path.normpath(os.path.join(os.path.dirname(__file__), ".."))

# P13 目标：目录（递归）+ 单文件（contract §3 P13 路径清单）
P13_DIRS = [
    os.path.join(ROOT, "Live2Dai-Android", "app", "src"),
    os.path.join(ROOT, "Live2Dai-pc", "open-llm-vtuber", "renderer", "src"),
]
P13_FILES = [
    os.path.join(ROOT, "Live2Dai-Android", "app", "src", "main", "assets", "persona.yaml"),
    os.path.join(ROOT, "Live2Dai-pc", "open-llm-vtuber", "conf.yaml"),
    os.path.join(ROOT, "Live2Dai-pc", "open-llm-vtuber", "renderer", "index.html"),
    os.path.join(ROOT, "Live2Dai-pc", "open-llm-vtuber", "frontend", "index.html"),
    os.path.join(ROOT, "Live2Dai-pc", "start.ps1"),
    os.path.join(ROOT, "shared", "persona.yaml"),
    os.path.join(ROOT, "shared", "emotion-protocol.md"),
]


def _iter_p13_files():
    """逐文件产出 P13 目标清单（目录内递归 + 单文件）。"""
    for d in P13_DIRS:
        for base, _, names in os.walk(d):
            for n in names:
                yield os.path.join(base, n)
    for f in P13_FILES:
        if os.path.isfile(f):
            yield f


def _read_text(path):
    with open(path, "r", encoding="utf-8", errors="replace") as f:
        return f.read()


class TestPersonaResidual(unittest.TestCase):
    """P13 联合 grep 门：目标清单内『凛』零命中。"""

    def test_p13_no_rin_residual(self):
        hits = []
        for p in _iter_p13_files():
            txt = _read_text(p)
            if "凛" in txt:
                hits.append((os.path.relpath(p, ROOT), "凛"))
        self.assertEqual(hits, [], f"P13 联合 grep 门残留『凛』：{hits}")


class TestPersonaNewValuePresent(unittest.TestCase):
    """P5/P6 新值命中检查（机械 grep，C5 实现后应全绿）。"""

    # ---- P5 · PC 配置/显示 ----
    def test_p5_conf_character_name(self):
        p = os.path.join(ROOT, "Live2Dai-pc", "open-llm-vtuber", "conf.yaml")
        self.assertIn("character_name: '白'", _read_text(p), "conf.yaml character_name 应为 '白'")

    def test_p5_start_ps1_banner(self):
        p = os.path.join(ROOT, "Live2Dai-pc", "start.ps1")
        self.assertIn("人设: 白", _read_text(p), "start.ps1 横幅应为人设: 白")

    def test_p5_renderer_and_frontend_title(self):
        for f in ("renderer", "frontend"):
            p = os.path.join(ROOT, "Live2Dai-pc", "open-llm-vtuber", f, "index.html")
            self.assertIn("白", _read_text(p), f"{f}/index.html <title> 应含 白")

    def test_p5_chat_ui_title(self):
        p = os.path.join(ROOT, "Live2Dai-pc", "open-llm-vtuber", "renderer", "src", "chat-ui.ts")
        self.assertIn("白 · Live2Dai", _read_text(p), "chat-ui.ts 标题应为 白 · Live2Dai")

    # ---- P6 · Android 代码 ----
    def test_p6_persona_config_fallback_prompt(self):
        p = os.path.join(ROOT, "Live2Dai-Android", "app", "src", "main", "java",
                         "com", "live2dai", "android", "PersonaConfig.kt")
        txt = _read_text(p)
        self.assertIn("你是白", txt, "PersonaConfig FALLBACK_PROMPT 应含 你是白")
        self.assertNotIn("凛", txt, "PersonaConfig 不得残留 凛")

    def test_p6_persona_config_default_name_and_entry(self):
        p = os.path.join(ROOT, "Live2Dai-Android", "app", "src", "main", "java",
                         "com", "live2dai", "android", "PersonaConfig.kt")
        txt = _read_text(p)
        self.assertIn('personaName: String = "白"', txt, "PersonaConfig personaName 默认应为 \"白\"")
        self.assertIn("白 (默认)", txt, "PersonaConfig 人设列表应含 \"白 (默认)\" 条目")

    def test_p6_settings_repository_default_persona(self):
        p = os.path.join(ROOT, "Live2Dai-Android", "app", "src", "main", "java",
                         "com", "live2dai", "android", "data", "SettingsRepository.kt")
        self.assertIn('DEFAULT_PERSONA = "白 (Bai)"', _read_text(p),
                      "SettingsRepository DEFAULT_PERSONA 应 == \"白 (Bai)\"")

    def test_p6_proactive_greeting_removed_from_core(self):
        # Wave 5 已把主动陪伴/ProactiveGreetingClient 移出核心；不应再作为核心文件存在。
        p = os.path.join(ROOT, "Live2Dai-Android", "app", "src", "main", "java",
                         "com", "live2dai", "android", "ProactiveGreetingClient.kt")
        self.assertFalse(os.path.isfile(p), "ProactiveGreetingClient 已随 Wave5 移出核心，不应存在")


if __name__ == "__main__":
    unittest.main()
