"""共享表情协议文件存在性 + persona YAML 合法性测试（M1）。

覆盖:
- C22a: shared/emotion-protocol.md 存在
- C22b: 含 B/C/D/E/F 全部段落标题
- C-p2: shared/persona.yaml YAML 合法（含 name/role/system_prompt/profile；model 等已收敛进 LLM 设置）

风格: unittest + 直读文件（对齐 test_persona_shared.py）。
"""
import os
import re
import unittest

import yaml


class TestEmotionProtocol(unittest.TestCase):

    @classmethod
    def setUpClass(cls):
        cls.repo_root = os.path.normpath(
            os.path.join(os.path.dirname(__file__), "..")
        )
        cls.protocol_path = os.path.join(cls.repo_root, "shared", "emotion-protocol.md")
        cls.persona_path = os.path.join(cls.repo_root, "shared", "persona.yaml")

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

    # ---- C-p2: 共享 persona YAML 合法 ----
    def test_cp2_shared_persona_yaml_valid(self):
        """C-p2: shared/persona.yaml 可解析且含必要字段。"""
        self.assertTrue(
            os.path.isfile(self.persona_path),
            f"共享 persona 不存在: {self.persona_path}",
        )
        with open(self.persona_path, "r", encoding="utf-8") as f:
            persona = yaml.safe_load(f)

        self.assertIsNotNone(persona, "persona.yaml 解析为 None")
        # 2026-08-22 人设收敛：model/temperature/max_tokens/vision_model 归 LLM 设置，
        # persona 顶层只承载 name/role/profile/system_prompt。
        for field in (
            "name",
            "role",
            "system_prompt",
            "profile",
        ):
            self.assertIn(field, persona, f"persona.yaml 缺少字段: {field}")
        self.assertNotIn("model", persona, "persona.yaml 不应再含 model（已收敛进 LLM 设置）")


if __name__ == "__main__":
    unittest.main()
