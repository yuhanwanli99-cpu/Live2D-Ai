"""shared/validate_registry.py 鲁棒性测试。

覆盖 Tester 报告的两处实现缺陷修复（`tester→developer`，缺陷 #1/#2）：
- available=true 条目缺失 model3JsonPath → 干净报错（exit 1 + 可读信息），绝不抛 TypeError
- models 非数组（如字典）→ 干净报错（exit 1 + 可读信息），绝不抛 AttributeError
- 顶层非对象（合法 JSON、非法 registry）→ 同样干净报错，不崩溃

核心断言：畸形输入必须输出可读错误并以非零退出码结束，输出中不得出现
"Traceback"/"TypeError"/"AttributeError" 等裸异常痕迹。
"""
import contextlib
import io
import json
import os
import sys
import tempfile
import unittest
from unittest.mock import patch

_SHARED_DIR = os.path.join(os.path.dirname(__file__), "..", "shared")
if _SHARED_DIR not in sys.path:
    sys.path.insert(0, _SHARED_DIR)

import validate_registry  # noqa: E402


def run_validator(registry_dict):
    """将畸形 registry 写入临时文件并运行 main()，返回 (exit_code, stdout)。"""
    with tempfile.TemporaryDirectory() as tmp:
        reg_path = validate_registry.Path(tmp) / "model_registry.json"
        with open(reg_path, "w", encoding="utf-8") as f:
            json.dump(registry_dict, f, ensure_ascii=False)
        buf = io.StringIO()
        with patch.object(validate_registry, "REGISTRY_PATH", reg_path):
            with contextlib.redirect_stdout(buf):
                code = validate_registry.main()
        return code, buf.getvalue()


class TestValidateRegistryRobustness(unittest.TestCase):
    def test_real_registry_passes(self):
        """正向：真实 registry → exit 0，全部检查通过。"""
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            code = validate_registry.main()
        out = buf.getvalue()
        self.assertEqual(code, 0, out)
        self.assertIn("All checks passed", out)

    def test_available_entry_missing_model3_json_path_clean_error(self):
        """缺陷 #1：available=true 条目缺 model3JsonPath → exit 1 + 可读信息，无 traceback。"""
        registry = {
            "version": "1.0",
            "models": [
                {"id": "bai", "name": "白-免费版 (Bai Free)", "available": True, "default": True}
            ],
        }
        code, out = run_validator(registry)
        self.assertEqual(code, 1)
        self.assertIn("FAILED", out)
        self.assertIn("model3JsonPath", out)
        self.assertNotIn("Traceback", out)
        self.assertNotIn("TypeError", out)

    def test_models_not_a_list_clean_error(self):
        """缺陷 #2：models 为字典 → exit 1 + 可读信息，无 traceback。"""
        registry = {"version": "1.0", "models": {"a": 1}}
        code, out = run_validator(registry)
        self.assertEqual(code, 1)
        self.assertIn("FAILED", out)
        self.assertNotIn("Traceback", out)
        self.assertNotIn("AttributeError", out)

    def test_top_level_not_an_object_clean_error(self):
        """顶层为数组（合法 JSON、非法 registry）→ exit 1 + 可读信息，无 traceback。"""
        code, out = run_validator([1, 2, 3])
        self.assertEqual(code, 1)
        self.assertIn("FAILED", out)
        self.assertNotIn("Traceback", out)
        self.assertNotIn("AttributeError", out)


if __name__ == "__main__":
    unittest.main()
