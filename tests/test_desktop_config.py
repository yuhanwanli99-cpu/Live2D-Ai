"""Desktop config tests — updated for direct-API mode (independent LLM, no gateway)."""

import os
import unittest

import yaml


class TestDesktopConfig(unittest.TestCase):
    def setUp(self):
        self.conf_path = os.path.join(
            os.path.dirname(__file__), "..", "Live2Dai-pc", "open-llm-vtuber", "conf.yaml"
        )
        with open(self.conf_path, "r", encoding="utf-8") as f:
            self.config = yaml.safe_load(f)
        # Persona is the single source of truth in shared/persona.yaml,
        # referenced by conf.yaml via load_persona_from_file.
        self.persona_path = os.path.join(
            os.path.dirname(__file__), "..", "shared", "persona.yaml"
        )
        with open(self.persona_path, "r", encoding="utf-8") as f:
            self.persona_config = yaml.safe_load(f)

    def test_conf_yaml_loading(self):
        self.assertIsNotNone(self.config)
        self.assertIn("system_config", self.config)
        self.assertIn("character_config", self.config)

    def test_deepseek_llm_configuration(self):
        llm_configs = self.config["character_config"]["agent_config"]["llm_configs"]
        deepseek_config = llm_configs["deepseek_llm"]

        self.assertEqual(
            deepseek_config["llm_api_key"],
            r"${DEEPSEEK_API_KEY}",
        )
        self.assertEqual(deepseek_config["model"], "deepseek-v4-flash")
        self.assertEqual(deepseek_config["temperature"], 0.8)

    def test_zhipu_free_llm_configuration(self):
        # 零配置默认 Provider：免费 GLM-4.7-Flash 兜底
        llm_configs = self.config["character_config"]["agent_config"]["llm_configs"]
        free_cfg = llm_configs["zhipu_free_llm"]
        self.assertEqual(free_cfg["model"], "glm-4.7-flash")
        self.assertEqual(free_cfg["temperature"], 0.8)
        # Key 使用环境变量占位符（真实共享 Key 在代码层 free_glm_defaults.py，不进 yaml/断言）
        self.assertEqual(free_cfg["llm_api_key"], r"${ZHIPU_API_KEY}")

    def test_zhipu_llm_configuration(self):
        llm_configs = self.config["character_config"]["agent_config"]["llm_configs"]
        zhipu_config = llm_configs["zhipu_llm"]

        self.assertEqual(
            zhipu_config["llm_api_key"],
            r"${ZHIPU_API_KEY}",
        )
        self.assertEqual(zhipu_config["model"], "glm-4.6v")

    def test_edge_tts_configuration(self):
        tts_config = self.config["character_config"]["tts_config"]
        self.assertEqual(tts_config["tts_model"], "cosyvoice_cloud")
        cosyvoice_config = tts_config["cosyvoice_cloud"]
        self.assertEqual(cosyvoice_config["model"], "cosyvoice-v1")
        edge_tts_config = tts_config["edge_tts"]
        self.assertEqual(edge_tts_config["voice"], "zh-CN-XiaoxiaoNeural")

    def test_asr_config(self):
        asr_config = self.config["character_config"]["asr_config"]
        self.assertEqual(asr_config["asr_model"], "faster_whisper")

    def test_agent_config(self):
        ag = self.config["character_config"]["agent_config"]
        self.assertEqual(ag["conversation_agent_choice"], "basic_memory_agent")
        bma = ag["agent_settings"]["basic_memory_agent"]
        # 默认 DeepSeek（本地 .env 配置 DEEPSEEK_API_KEY；密钥不提交，spoq-fix M-SEC）
        self.assertEqual(bma["llm_provider"], "deepseek_llm")
        self.assertFalse(bma["use_mcpp"])
        self.assertEqual(bma["mcp_enabled_servers"], [])

    def test_persona_prompt_content(self):
        persona = self.persona_config["system_prompt"]
        self.assertTrue(len(persona) > 50)
        self.assertIn("凛", persona)
        self.assertIn("傲娇", persona)
        self.assertIn("天才少女", persona)

    def test_expression_tags_in_persona(self):
        persona = self.persona_config["system_prompt"]
        self.assertIn("[joy]", persona)
        self.assertIn("[neutral]", persona)

    def test_system_config(self):
        sc = self.config["system_config"]
        self.assertEqual(sc["host"], "0.0.0.0")
        self.assertEqual(sc["port"], 12393)
        self.assertEqual(sc["live2d_fps"], 120)

    def test_tts_preprocessor_config(self):
        tts_pre = self.config["character_config"]["tts_preprocessor_config"]
        self.assertTrue(tts_pre["ignore_brackets"])
        self.assertTrue(tts_pre["ignore_parentheses"])

    def test_live2d_expression_prompt(self):
        tool_prompts = self.config["system_config"]["tool_prompts"]
        self.assertEqual(
            tool_prompts["live2d_expression_prompt"], "live2d_expression_prompt"
        )

    def test_mcp_prompt_enabled(self):
        tool_prompts = self.config["system_config"]["tool_prompts"]
        self.assertEqual(tool_prompts["mcp_prompt"], "mcp_prompt")


if __name__ == "__main__":
    unittest.main()
