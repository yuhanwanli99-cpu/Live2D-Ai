import os
import yaml
import unittest
from unittest.mock import patch, mock_open

class TestPersonaShared(unittest.TestCase):

    def setUp(self):
        self.persona_path = os.path.join(os.path.dirname(__file__), "..", "shared", "persona.yaml")
        self.desktop_conf_path = os.path.join(os.path.dirname(__file__), "..", "Live2Dai-pc", "open-llm-vtuber", "conf.yaml")
        self.android_assets_path = os.path.join(os.path.dirname(__file__), "..", "Live2Dai-Android", "app", "src", "main", "assets", "persona.yaml")

    def test_persona_yaml_format(self):
        """Test that shared/persona.yaml has correct format"""
        with open(self.persona_path, 'r', encoding='utf-8') as f:
            persona = yaml.safe_load(f)
        
        self.assertIsNotNone(persona)
        self.assertIn('name', persona)
        self.assertIn('role', persona)
        self.assertIn('system_prompt', persona)
        self.assertIn('model', persona)
        self.assertIn('temperature', persona)
        self.assertIn('max_tokens', persona)
        self.assertIn('vision_model', persona)

    def test_system_prompt_non_empty(self):
        """Test that system_prompt is not empty"""
        with open(self.persona_path, 'r', encoding='utf-8') as f:
            persona = yaml.safe_load(f)
        
        system_prompt = persona['system_prompt']
        self.assertTrue(len(system_prompt.strip()) > 0)
        self.assertIn('你是凛', system_prompt)

    def test_desktop_and_android_consistency(self):
        """Test that desktop and android share the same persona configuration"""
        # Read shared persona
        with open(self.persona_path, 'r', encoding='utf-8') as f:
            persona = yaml.safe_load(f)
        
        # Read android assets persona
        with open(self.android_assets_path, 'r', encoding='utf-8') as f:
            android_persona = yaml.safe_load(f)
        
        # Compare key attributes between shared and android persona
        self.assertEqual(persona['name'], android_persona['name'])
        self.assertEqual(persona['role'], android_persona['role'])
        self.assertEqual(persona['model'], android_persona['model'])
        self.assertEqual(persona['temperature'], android_persona['temperature'])
        self.assertEqual(persona['max_tokens'], android_persona['max_tokens'])
        self.assertEqual(persona['vision_model'], android_persona['vision_model'])

    def test_persona_name_consistency(self):
        """Test that persona name is consistent across platforms"""
        with open(self.persona_path, 'r', encoding='utf-8') as f:
            persona = yaml.safe_load(f)
        
        name = persona['name']
        self.assertEqual(name, "凛 (Lin)")

    def test_model_consistency(self):
        """Test that model configuration is consistent"""
        with open(self.persona_path, 'r', encoding='utf-8') as f:
            persona = yaml.safe_load(f)
        
        model = persona['model']
        self.assertEqual(model, "deepseek-v4-flash")

    def test_temperature_consistency(self):
        """Test that temperature configuration is consistent"""
        with open(self.persona_path, 'r', encoding='utf-8') as f:
            persona = yaml.safe_load(f)
        
        temperature = persona['temperature']
        self.assertEqual(temperature, 0.8)

    def test_max_tokens_consistency(self):
        """Test that max_tokens configuration is consistent"""
        with open(self.persona_path, 'r', encoding='utf-8') as f:
            persona = yaml.safe_load(f)
        
        max_tokens = persona['max_tokens']
        self.assertEqual(max_tokens, 300)

    def test_vision_model_consistency(self):
        """Test that vision model configuration is consistent"""
        with open(self.persona_path, 'r', encoding='utf-8') as f:
            persona = yaml.safe_load(f)
        
        vision_model = persona['vision_model']
        self.assertEqual(vision_model, "glm-4.6v")

    def test_role_consistency(self):
        """Test that role configuration is consistent"""
        with open(self.persona_path, 'r', encoding='utf-8') as f:
            persona = yaml.safe_load(f)
        
        role = persona['role']
        self.assertEqual(role, "tsundere genius assistant companion")

    def test_system_prompt_structure(self):
        """Test that system_prompt has the required structure"""
        with open(self.persona_path, 'r', encoding='utf-8') as f:
            persona = yaml.safe_load(f)
        
        system_prompt = persona['system_prompt']
        self.assertIn('性格特点', system_prompt)
        self.assertIn('说话风格', system_prompt)
        self.assertIn('行为准则', system_prompt)
        self.assertIn('使用示例', system_prompt)
        self.assertIn('工具使用', system_prompt)

    def test_persona_file_exists(self):
        """Test that persona.yaml file exists and is accessible"""
        self.assertTrue(os.path.exists(self.persona_path))
        self.assertTrue(os.path.isfile(self.persona_path))

    def test_persona_file_readable(self):
        """Test that persona.yaml can be read without errors"""
        try:
            with open(self.persona_path, 'r', encoding='utf-8') as f:
                yaml.safe_load(f)
        except Exception as e:
            self.fail(f"Failed to read persona.yaml: {e}")

    # ---- C-p1: 双端 persona 字节一致（M1 §4.2/7.3，一次性同步后验证） ----
    def test_shared_and_android_persona_byte_identical(self):
        """C-p1: shared/persona.yaml 与 Android assets/persona.yaml 字节一致。

        验证 M1 一次性同步生效（强度区块错位修复），diff 为空。
        """
        if not os.path.isfile(self.android_assets_path):
            self.skipTest("Android assets/persona.yaml 不存在，跳过字节一致检查")

        with open(self.persona_path, 'rb') as f:
            shared_bytes = f.read()
        with open(self.android_assets_path, 'rb') as f:
            android_bytes = f.read()

        self.assertEqual(
            shared_bytes,
            android_bytes,
            "shared/persona.yaml 与 Android assets/persona.yaml 字节不一致！"
            " 请用 M1 一次性同步步骤: cp shared/persona.yaml Live2Dai-Android/app/src/main/assets/persona.yaml",
        )

        # 额外验证：Android 副本也能被 yaml.safe_load 正常解析（ScannerError 消除）
        try:
            with open(self.android_assets_path, 'r', encoding='utf-8') as f:
                parsed = yaml.safe_load(f)
            self.assertIsNotNone(parsed, "Android persona.yaml 解析为 None")
        except Exception as e:
            self.fail(f"Android persona.yaml 解析失败（可能强度区块仍错位）: {e}")

if __name__ == '__main__':
    unittest.main()