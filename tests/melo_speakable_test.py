"""MeloTTS 垫片 `melo_speakable` 的纯函数测试（2026-10-15 执行单 §1）。

不 `import melo`、不载模型、不占端口：按**文件路径**加载 serve.py，只测那个
纯函数——送 MeloTTS 前只删 ASCII 下划线 U+005F，其余字符一个不动。
"""

import importlib.util
import os
import unittest

SERVE_PY = os.path.join(
    os.path.normpath(os.path.join(os.path.dirname(__file__), "..")),
    "crates",
    "live2d-ai-mod-local-tts",
    "melo",
    "serve.py",
)


def _load_serve():
    """按路径加载 serve.py（模块名不是 `__main__`，不会跑起服务）。"""
    spec = importlib.util.spec_from_file_location("melo_serve_for_test", SERVE_PY)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class TestMeloSpeakable(unittest.TestCase):

    @classmethod
    def setUpClass(cls):
        # 存**模块**而不是裸函数：函数挂到类上会变成绑定方法，多收一个 self。
        cls.serve = _load_serve()

    def test_underscore_is_deleted_not_replaced(self):
        """只删下划线：不换成 `-`、也不换成空格。"""
        for raw, want in (
            ("御寒_v4p", "御寒v4p"),
            ("a_b", "ab"),
            ("喵_喵", "喵喵"),
        ):
            self.assertEqual(self.serve.melo_speakable(raw), want)

    def test_only_underscores_becomes_empty(self):
        """`___` 洗完为空串——synth 随后照旧抛 ValueError("input 为空")。"""
        self.assertEqual(self.serve.melo_speakable("___"), "")

    def test_other_characters_are_untouched(self):
        for text in ("你好。", "#测试", "a-b c", "喵！"):
            self.assertEqual(self.serve.melo_speakable(text), text)


if __name__ == "__main__":
    unittest.main()
