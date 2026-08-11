"""PC 端 LLM key 本地化测试（spoq-fix M-SEC：密钥不提交，本地配置）。

覆盖：
- conf.yaml 默认 provider 为 deepseek_llm（用户本地 .env 配置 DEEPSEEK_API_KEY）
- zhipu_free_llm / deepseek_llm 配置结构（model / temperature / key 占位符）
- resolve_llm_api_key()：空值/占位符 → 抛 ValueError（带 .env/conf.yaml 指引）；用户 Key 原样保留。
  注意：不在断言中写入或打印真实 Key 值（仅断言结构特征）。
"""
import os
import sys
import unittest

import yaml

_PC_SRC = os.path.join(
    os.path.dirname(__file__), "..", "Live2Dai-pc", "open-llm-vtuber", "src"
)
if _PC_SRC not in sys.path:
    sys.path.insert(0, _PC_SRC)

from open_llm_vtuber.free_glm_defaults import resolve_llm_api_key  # noqa: E402


class TestPcZeroConfig(unittest.TestCase):
    def setUp(self):
        self.conf_path = os.path.join(
            os.path.dirname(__file__), "..", "Live2Dai-pc", "open-llm-vtuber", "conf.yaml"
        )
        with open(self.conf_path, "r", encoding="utf-8") as f:
            self.config = yaml.safe_load(f)

    def test_default_provider_is_deepseek(self):
        """默认 provider 为 deepseek_llm（用户本地 .env 配置 DEEPSEEK_API_KEY）。"""
        bma = self.config["character_config"]["agent_config"]["agent_settings"]["basic_memory_agent"]
        self.assertEqual(bma["llm_provider"], "deepseek_llm")

    def test_zhipu_free_llm_config_entry(self):
        llm_configs = self.config["character_config"]["agent_config"]["llm_configs"]
        free_cfg = llm_configs["zhipu_free_llm"]
        self.assertEqual(free_cfg["model"], "glm-4.7-flash")
        self.assertEqual(free_cfg["temperature"], 0.8)
        # Key 为环境变量占位符；真实 Key 只在本地 .env / local.properties（gitignored）
        self.assertEqual(free_cfg["llm_api_key"], "${ZHIPU_API_KEY}")

    def test_deepseek_entry_still_present(self):
        """DeepSeek 默认 provider 配置结构完整（用户本地 key 驱动）。"""
        llm_configs = self.config["character_config"]["agent_config"]["llm_configs"]
        self.assertIn("deepseek_llm", llm_configs)
        self.assertEqual(llm_configs["deepseek_llm"]["model"], "deepseek-v4-flash")

    def test_resolver_raises_for_empty_or_placeholder(self):
        """空值 / 未解析的 ${VAR} 占位符 → 抛 ValueError（含 .env/conf.yaml 指引，不内置共享 key）。"""
        for raw in ("", None, "${ZHIPU_API_KEY}"):
            with self.assertRaises(ValueError) as ctx:
                resolve_llm_api_key(raw)
            msg = str(ctx.exception)
            self.assertIn("API key", msg)
            self.assertTrue(".env" in msg or "conf.yaml" in msg,
                            f"错误消息应含配置指引（.env/conf.yaml），实际: {msg}")

    def test_resolver_keeps_user_key(self):
        """用户显式提供的 Key 原样保留（如环境变量覆盖值 / 手改 conf.yaml）。"""
        self.assertEqual(resolve_llm_api_key("sk-user-supplied-key"), "sk-user-supplied-key")
        self.assertEqual(resolve_llm_api_key("  12345.secret  "), "12345.secret")

    def test_resolver_does_not_share_any_key(self):
        """无任何内置共享 key：空/占位符一律抛错，绝不返回某把内置 key。"""
        with self.assertRaises(ValueError):
            resolve_llm_api_key("")
        with self.assertRaises(ValueError):
            resolve_llm_api_key("${ZHIPU_API_KEY}")

    def test_resolver_distinguishes_deepseek_key_too(self):
        """DeepSeek 用户 Key 同样原样保留，不被误伤。"""
        deepseek_key = "sk-ds-user-key-abcdef123456"
        self.assertEqual(resolve_llm_api_key(deepseek_key), deepseek_key)

    def test_conf_yaml_keeps_placeholder_not_real_key(self):
        """conf.yaml 保持 ${VAR} 占位符（真实 Key 只在本地 .env / local.properties）。"""
        llm_configs = self.config["character_config"]["agent_config"]["llm_configs"]
        self.assertEqual(llm_configs["zhipu_free_llm"]["llm_api_key"], "${ZHIPU_API_KEY}")
        self.assertEqual(llm_configs["deepseek_llm"]["llm_api_key"], "${DEEPSEEK_API_KEY}")


if __name__ == "__main__":
    unittest.main()
