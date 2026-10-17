"""共享表情协议文件存在性测试（M1）。

覆盖:
- C22a: shared/emotion-protocol.md 存在
- C22b: 含 B/C/D/E/F 全部段落标题

（C-p2 的 persona.yaml 校验已随该文件于 d69d45f 删除。）

风格: unittest + 直读文件。
"""
import os
import re
import unittest


class TestEmotionProtocol(unittest.TestCase):

    @classmethod
    def setUpClass(cls):
        cls.repo_root = os.path.normpath(
            os.path.join(os.path.dirname(__file__), "..")
        )
        cls.protocol_path = os.path.join(cls.repo_root, "shared", "emotion-protocol.md")

    # ---- C22a: 协议文件存在 ----
    def test_c22a_protocol_file_exists(self):
        """C22a: shared/emotion-protocol.md 存在。"""
        self.assertTrue(
            os.path.isfile(self.protocol_path),
            f"协议文件不存在: {self.protocol_path}",
        )

    # ---- C22b: 含 B/C/D/E/F 段落 ----
    def test_c22b_contains_required_sections(self):
        """C22b: emotion-protocol.md 含 B/C/D/E/F 全部段落标题。"""
        if not os.path.isfile(self.protocol_path):
            self.skipTest("协议文件不存在，跳过段落检查")
        with open(self.protocol_path, "r", encoding="utf-8") as f:
            content = f.read()

        required = [
            "## B. JSON schema",
            "## C. 字段约束",
            "## D. 示例",
            "## E. 降级规则",
            "## F. motion 映射表",
        ]
        for sec in required:
            self.assertIn(
                sec,
                content,
                f"{self.protocol_path} 缺少段落标题: {sec}",
            )


if __name__ == "__main__":
    unittest.main()
