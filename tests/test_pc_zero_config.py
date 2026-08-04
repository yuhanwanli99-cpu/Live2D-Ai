"""PC 端零配置 GLM Provider 测试（Live2Dai-pc 与 Android 端对齐）。

覆盖：
- conf.yaml 默认 provider 为 zhipu_free_llm（免费 GLM-4.7-Flash 兜底）
- zhipu_free_llm 配置结构（model / temperature / key 占位符）
- resolve_llm_api_key()：空值/占位符回退到内置共享 Key；用户 Key 原样保留。
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

    def test_default_provider_is_free_glm(self):
        """零配置默认 provider 为 zhipu_free_llm（无需用户先填 API Key）。"""
        bma = self.config["character_config"]["agent_config"]["agent_settings"]["basic_memory_agent"]
        self.assertEqual(bma["llm_provider"], "zhipu_free_llm")

    def test_zhipu_free_llm_config_entry(self):
        llm_configs = self.config["character_config"]["agent_config"]["llm_configs"]
        free_cfg = llm_configs["zhipu_free_llm"]
        self.assertEqual(free_cfg["model"], "glm-4.7-flash")
        self.assertEqual(free_cfg["temperature"], 0.8)
        # Key 为环境变量占位符；真实共享 Key 在代码层，不在 yaml/断言中
        self.assertEqual(free_cfg["llm_api_key"], "${ZHIPU_API_KEY}")

    def test_deepseek_upgrade_path_still_present(self):
        """DeepSeek 仍是可选的升级路径（用户自备 Key）。"""
        llm_configs = self.config["character_config"]["agent_config"]["llm_configs"]
        self.assertIn("deepseek_llm", llm_configs)
        self.assertEqual(llm_configs["deepseek_llm"]["model"], "deepseek-v4-flash")

    def test_resolver_falls_back_to_builtin_for_empty_or_placeholder(self):
        """空值 / 未解析的 ${VAR} 占位符 → 回退到内置共享 Key（零配置开箱即用）。"""
        for raw in ("", "${ZHIPU_API_KEY}"):
            resolved = resolve_llm_api_key(raw)
            # 只断言结构特征，不打印/不对比真实 Key 值
            self.assertTrue(resolved, "resolved key must not be empty")
            self.assertNotIn("${", resolved)
            self.assertNotEqual(resolved, raw)
            self.assertGreater(len(resolved), 20)  # 智谱 key 形如 id.secret（较长）
            self.assertIn(".", resolved)

    def test_resolver_keeps_user_key(self):
        """用户显式提供的 Key 原样保留（如环境变量 ZHIPU_API_KEY 覆盖值 / 手改 conf.yaml）。"""
        self.assertEqual(resolve_llm_api_key("sk-user-supplied-key"), "sk-user-supplied-key")
        self.assertEqual(resolve_llm_api_key("  12345.secret  "), "12345.secret")

    def test_resolver_does_not_overwrite_user_key_with_shared_default(self):
        """用户已配置自定义 Key（真实智谱格式 id.secret）时，绝不能被内置共享 Key 覆盖。
        这是「零配置兜底不覆盖用户已有配置」的边界：仅当 Key 为空/占位符时才启用内置回退。"""
        user_key = "abc123def456.ghi789jkl012"
        resolved = resolve_llm_api_key(user_key)
        self.assertEqual(resolved, user_key)
        self.assertNotEqual(resolved, resolve_llm_api_key("${ZHIPU_API_KEY}"))

    def test_resolver_distinguishes_deepseek_key_too(self):
        """DeepSeek 用户 Key（非智谱）同样原样保留，避免升级路径被误伤。"""
        deepseek_key = "sk-ds-user-key-abcdef123456"
        self.assertEqual(resolve_llm_api_key(deepseek_key), deepseek_key)

    def test_conf_yaml_keeps_placeholder_not_real_key(self):
        """conf.yaml 中零配置条目保持 ${ZHIPU_API_KEY} 占位符（真实 Key 在代码层），
        保证 yaml 永不包含/泄露真实 Key。"""
        llm_configs = self.config["character_config"]["agent_config"]["llm_configs"]
        self.assertEqual(llm_configs["zhipu_free_llm"]["llm_api_key"], "${ZHIPU_API_KEY}")
        self.assertEqual(llm_configs["deepseek_llm"]["llm_api_key"], "${DEEPSEEK_API_KEY}")


if __name__ == "__main__":
    unittest.main()
