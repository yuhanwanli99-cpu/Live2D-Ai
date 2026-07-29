import os
import yaml
import unittest
from unittest.mock import patch, mock_open

class TestPersonaShared(unittest.TestCase):

    def setUp(self):
        self.persona_path = os.path.join(os.path.dirname(__file__), "..", "shared", "persona.yaml")
        self.desktop_conf_path = os.path.join(os.path.dirname(__file__), "..", "desktop", "open-llm-vtuber", "conf.yaml")
        self.android_assets_path = os.path.join(os.path.dirname(__file__), "..", "android-native", "app", "src", "main", "assets", "persona.yaml")

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
        self.assertIn('你是小喵', system_prompt)

    def test_desktop_and_android_consistency(self):
        """Test that desktop and android share the same persona configuration"""
        # Read desktop config
        with open(self.desktop_conf_path, 'r', encoding='utf-8') as f:
            desktop_config = yaml.safe_load(f)
        
        # Read android assets persona (simulated)
        with open(self.android_assets_path, 'r', encoding='utf-8') as f:
            android_persona = yaml.safe_load(f)
        
        # Verify key persona attributes match
        desktop_persona = desktop_config['character_config']['load_persona_from_file']
        self.assertEqual(desktop_persona, 'shared/persona.yaml')
        
        # Compare key attributes
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
        self.assertEqual(name, "小喵 (Xiao Miao)")

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
        self.assertEqual(vision_model, "glm-4v")

    def test_role_consistency(self):
        """Test that role configuration is consistent"""
        with open(self.persona_path, 'r', encoding='utf-8') as f:
            persona = yaml.safe_load(f)
        
        role = persona['role']
        self.assertEqual(role, "cat-girl maid companion")

    def test_system_prompt_structure(self):
        """Test that system_prompt has the required structure"""
        with open(self.persona_path, 'r', encoding='utf-8') as f:
            persona = yaml.safe_load(f)
        
        system_prompt = persona['system_prompt']
        self.assertIn('性格特点', system_prompt)
        self.assertIn('说话风格', system_prompt)
        self.assertIn('行为准则', system_prompt)
        self.assertIn('示例对话', system_prompt)
        self.assertIn('重要', system_prompt)

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

if __name__ == '__main__':
    unittest.main()