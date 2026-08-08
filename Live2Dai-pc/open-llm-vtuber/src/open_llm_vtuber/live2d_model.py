import json
import os
import re

import chardet
from loguru import logger

from .model_registry import ModelEntry, ModelRegistry

# 协议 8 key 枚举（与 Android EMOTION_MAP 同集；与 model_dict emotionMap 解耦，裁决 2）
PROTOCOL_EMOTION_KEYS = (
    "neutral",
    "fear",
    "sadness",
    "anger",
    "disgust",
    "joy",
    "smirk",
    "surprise",
)

# 8 索引（裁决 1；与 Android EMOTION_MAP 值一致，joy→5 等）
PROTOCOL_EMOTION_INDEX = {name: i for i, name in enumerate(PROTOCOL_EMOTION_KEYS)}

# 表情 JSON 提取正则（与 Android parseStructured 同规则：FENCE 优先 → NAKED 兜底）
_JSON_FENCE_RE = re.compile(r"```(?:json)?\s*(\{.*?\})\s*```", re.DOTALL)
_JSON_NAKED_RE = re.compile(r"\{.*\}", re.DOTALL)

# motionLabel 别名表（M1 §5.3 F 段 mao_pro 别名 + shizuku 语义组名，大小写不敏感）
_MOTION_LABEL_ALIASES = frozenset(
    {
        "idle",
        "wave",
        "special_02",
        "special_03",
        "mtn_02",
        "mtn_03",
        "mtn_04",
        # shizuku 语义组名（F 段：可直接用组名）
        "flickup",
        "tap",
        "flick3",
    }
)

# This class will only prepare the payload for the live2d model
# the process of sending the payload should be done by the caller
# This class is **Not responsible** for sending the payload to the server


class Live2dModel:
    """
    A class to represent a Live2D model. This class only prepares and stores the information of the Live2D model. It does not send anything to the frontend or server or anything.

    Attributes:
        model_dict_path (str): The path to the model dictionary file.
        live2d_model_name (str): The name of the Live2D model.
        model_info (dict): The information of the Live2D model.
        emo_map (dict): The emotion map of the Live2D model.
        emo_str (str): The string representation of the emotion map of the Live2D model.
    """

    model_dict_path: str
    live2d_model_name: str
    model_info: dict
    emo_map: dict
    emo_str: str

    def __init__(
        self,
        live2d_model_name: str,
        model_dict_path: str = "model_dict.json",
        registry: ModelRegistry | None = None,
    ):
        self.model_dict_path: str = model_dict_path
        self.live2d_model_name: str = live2d_model_name
        # 可选：跨平台模型注册表。提供时用于 model_dict.json 未命中/缺失时的兜底
        self.registry: ModelRegistry | None = registry
        self.set_model(live2d_model_name)

    def set_model(self, model_name: str) -> None:
        """
        Set the model with its name and load the model information. This method will initialize the `self.model_info`, `self.emo_map`, and `self.emo_str` attributes.
        This method is called in the constructor.

        Parameters:
            model_name (str): The name of the live2d model.

            Returns:
            None
        """

        self.model_info: dict = self._lookup_model_info(model_name)
        self.emo_map: dict = {
            k.lower(): v for k, v in self.model_info["emotionMap"].items()
        }
        self.emo_str: str = " ".join([f"[{key}]," for key in self.emo_map.keys()])
        # emo_str is a string of the keys in the emoMap dictionary. The keys are enclosed in square brackets.
        # example: `"[fear], [anger], [disgust], [sadness], [joy], [neutral], [surprise]"`
        # cdi3.json 参数 ID 白名单（C3：parameterOverrides 运行时动态校验，换模型自动适配）
        self._cdi3_parameter_ids: set = self._load_cdi3_parameter_ids()

    def _cdi3_rel_path(self) -> str:
        """从 model_info.url 推导当前模型 cdi3.json 的相对路径。

        url 形如 `/live2d-models/mao_pro/runtime/mao_pro.model3.json`
        → `live2d-models/mao_pro/runtime/mao_pro.cdi3.json`（model3.json → cdi3.json）。
        url 缺失（registry 兜底异常）时按约定布局回退。
        """
        url = (self.model_info.get("url") or "").lstrip("/")
        if url.endswith("model3.json"):
            return url[: -len("model3.json")] + "cdi3.json"
        return f"live2d-models/{self.live2d_model_name}/runtime/{self.live2d_model_name}.cdi3.json"

    def _load_cdi3_parameter_ids(self) -> set:
        """运行时读取当前模型 cdi3.json Parameters[].Id 全集（C3 白名单，不硬编码）。

        候选根目录：① CWD；② 模块文件向上查找第一个含 `live2d-models/` 的目录
        （server 与测试的 CWD 可能不同，向上查找保证两处都能解析）。
        找不到/解析失败 → 空集（parameterOverrides 全部丢弃，fail-closed，C3 确定面）。
        """
        rel = self._cdi3_rel_path()
        roots: list[str] = [os.getcwd()]
        current = os.path.dirname(os.path.abspath(__file__))
        for _ in range(8):
            if os.path.isdir(os.path.join(current, "live2d-models")):
                roots.append(current)
                break
            parent = os.path.dirname(current)
            if parent == current:
                break
            current = parent

        seen: set[str] = set()
        for root in roots:
            if root in seen:
                continue
            seen.add(root)
            path = os.path.join(root, rel)
            if not os.path.isfile(path):
                continue
            try:
                with open(path, "r", encoding="utf-8") as f:
                    data = json.load(f)
                ids = {
                    p.get("Id")
                    for p in data.get("Parameters", [])
                    if isinstance(p, dict) and p.get("Id")
                }
                logger.debug(
                    f"Loaded cdi3 parameter whitelist for '{self.live2d_model_name}': "
                    f"{len(ids)} ids from {path}"
                )
                return ids
            except (OSError, ValueError) as e:
                logger.warning(
                    f"Failed to load cdi3.json at {path}: {e}; "
                    "parameterOverrides whitelist empty (all keys dropped)."
                )
                return set()
        logger.warning(
            f"cdi3.json not found for '{self.live2d_model_name}' (rel={rel}); "
            "parameterOverrides whitelist empty (all keys dropped)."
        )
        return set()


    def _load_file_content(self, file_path: str) -> str:
        """Load the content of a file with robust encoding handling."""
        # Try common encodings first
        encodings = ["utf-8", "utf-8-sig", "gbk", "gb2312", "ascii"]

        for encoding in encodings:
            try:
                with open(file_path, "r", encoding=encoding) as file:
                    return file.read()
            except UnicodeDecodeError:
                continue

        # If all common encodings fail, try to detect encoding
        try:
            with open(file_path, "rb") as file:
                raw_data = file.read()
            detected = chardet.detect(raw_data)
            detected_encoding = detected["encoding"]

            if detected_encoding:
                try:
                    return raw_data.decode(detected_encoding)
                except UnicodeDecodeError:
                    pass
        except Exception as e:
            logger.error(f"Error detecting encoding for {file_path}: {e}")

        raise UnicodeError(f"Failed to decode {file_path} with any encoding")

    def _lookup_model_info(self, model_name: str) -> dict:
        """
        Find the model information from the model dictionary and return the information about the matched model.

        Parameters:
            model_name (str): The name of the live2d model.

        Returns:
            dict: The dictionary with the information of the matched model.

        Raises:
            FileNotFoundError if the model dictionary file is not found.

            json.JSONDecodeError if the model dictionary file is not a valid JSON file.

            KeyError if the model name is not found in the model dictionary.

        """

        self.live2d_model_name = model_name

        try:
            file_content = self._load_file_content(self.model_dict_path)
            model_dict = json.loads(file_content)
        except FileNotFoundError as file_e:
            # model_dict.json 缺失 → 回退到 registry 构造最小 model_info（plan 边界表）
            if self.registry is not None:
                entry = self.registry.get_by_id(model_name)
                if entry is not None:
                    logger.warning(
                        f"Model dictionary file not found at "
                        f"{self.model_dict_path}; using model registry fallback "
                        f"for '{model_name}'."
                    )
                    return self._registry_model_info(entry)
            logger.critical(
                f"Model dictionary file not found at {self.model_dict_path}."
            )
            raise file_e
        except json.JSONDecodeError as json_e:
            logger.critical(
                f"Error decoding JSON from model dictionary file at {self.model_dict_path}."
            )
            raise json_e
        except UnicodeError as uni_e:
            logger.critical(
                f"Error reading model dictionary file at {self.model_dict_path}."
            )
            raise uni_e
        except Exception as e:
            logger.critical(
                f"Error occurred while reading model dictionary file at {self.model_dict_path}."
            )
            raise e

        # Find the model in the model_dict
        matched_model = next(
            (model for model in model_dict if model["name"] == model_name), None
        )

        if matched_model is None:
            # model_dict.json 未命中 → 回退到 registry 构造最小 model_info（plan §4.4）
            if self.registry is not None:
                entry = self.registry.get_by_id(model_name)
                if entry is not None:
                    logger.warning(
                        f"Model '{model_name}' not found in {self.model_dict_path}; "
                        "using model registry fallback."
                    )
                    return self._registry_model_info(entry)
            logger.critical(f"Unable to find {model_name} in {self.model_dict_path}.")
            raise KeyError(
                f"{model_name} not found in model dictionary {self.model_dict_path}."
            )

        # The feature: "translate model url to full url if it starts with '/' " is no longer implemented here

        logger.info("Model Information Loaded.")

        return matched_model

    @staticmethod
    def _registry_model_info(entry: ModelEntry) -> dict:
        """从注册表条目构造最小 model_info。

        说明：除 plan 契约中的 name/url 外，额外补 `emotionMap: {}`——
        `set_model()` 无条件访问 `model_info["emotionMap"]`，缺失会 KeyError。
        空 emotionMap 对应 plan 边界表「emo_map 为空 → 无表情切换，不影响渲染」
        的降级行为；前端对缺失字段也有默认值（emotionMap:{}）。
        """
        return {
            "name": entry.id,
            "url": f"/live2d-models/{entry.model3_json_path}",
            "emotionMap": {},
        }

    def extract_emotion(self, str_to_check: str) -> list:
        """
        Check the input string for any emotion keywords and return a list of values (the expression index) of the emotions found in the string.

        Parameters:
            str_to_check (str): The string to check for emotions.

        Returns:
            list: A list of values of the emotions found in the string. An empty list is returned if no emotions are found.
        """

        expression_list = []
        str_to_check = str_to_check.lower()

        i = 0
        while i < len(str_to_check):
            if str_to_check[i] != "[":
                i += 1
                continue
            for key in self.emo_map.keys():
                emo_tag = f"[{key}]"
                if str_to_check[i : i + len(emo_tag)] == emo_tag:
                    expression_list.append(self.emo_map[key])
                    i += len(emo_tag) - 1
                    break
            i += 1
        return expression_list

    def remove_emotion_keywords(self, target_str: str) -> str:
        """
        Remove the emotion keywords from the input string and return the cleaned string.

        Parameters:
            str_to_check (str): The string to check for emotions.

        Returns:
            str: The cleaned string with the emotion keywords removed.
        """

        lower_str = target_str.lower()

        for key in self.emo_map.keys():
            lower_key = f"[{key}]".lower()
            while lower_key in lower_str:
                start_index = lower_str.find(lower_key)
                end_index = start_index + len(lower_key)
                target_str = target_str[:start_index] + target_str[end_index:]
                lower_str = lower_str[:start_index] + lower_str[end_index:]
        return target_str

    @staticmethod
    def _extract_json_candidate(text: str) -> str | None:
        """提取 JSON 候选串：FENCE（```json/``` 包裹）优先 → NAKED（首个 { 到末个 }）兜底。

        与 Android parseStructured 提取顺序一致（M1 §2.1）；找不到返回 None。
        """
        fence = _JSON_FENCE_RE.search(text)
        if fence is not None:
            return fence.group(1)
        naked = _JSON_NAKED_RE.search(text)
        if naked is not None:
            return naked.group(0)
        return None

    def extract_expression_mix(self, text: str) -> dict | None:
        """从 LLM 回复中解析结构化表情 JSON（emotion-protocol.md B 段 schema）。

        提取顺序（与 Android parseStructured 一致）：
          1. re.DOTALL 匹配 ```json ... ``` / ``` ... ``` fence 内 JSON 对象
          2. 无 fence 时，取 text 中第一个 '{' 到最后一个 '}' 的跨行子串（裸 JSON）

        校验规则（与 Android 完全一致，双端同输入同输出）：
          - expressionMix.expressions[].name 须 ∈ PROTOCOL_EMOTION_KEYS（8 key，大小写不敏感）
          - weight 非法（<0 或 >1）→ 丢弃该项；全部丢弃 → expressionMix 为空（不降级）
          - 权重总和 > 1.0 → 归一化
          - parameterOverrides key 不在当前模型 cdi3.json 白名单 → 丢弃该 key
          - motionLabel 不在别名表 → 置 None
          - JSON 解析失败 / 无 JSON / 结构非法 → 返回 None（调用方走 extract_emotion）

        返回 dict 形如：
          {"expressionMix": [{"name": "joy", "weight": 0.7}, ...],
           "parameterOverrides": {"ParamEyeLOpen": 1.2},
           "motionLabel": "wave" | None}
        或 None（降级信号）。
        """
        raw = self._extract_json_candidate(text)
        if raw is None:
            return None
        try:
            data = json.loads(raw)
        except (json.JSONDecodeError, TypeError):
            return None
        if not isinstance(data, dict):
            return None

        has_valid_field = False
        mix: list[dict] = []
        overrides: dict = {}
        motion: str | None = None

        # expressionMix（C1/C2）
        em = data.get("expressionMix")
        if isinstance(em, dict):
            has_valid_field = True
            exprs = em.get("expressions")
            if not isinstance(exprs, list):
                exprs = []
            total = 0.0
            for item in exprs:
                if not isinstance(item, dict):
                    continue
                name = item.get("name")
                weight = item.get("weight")
                if not isinstance(name, str):
                    continue
                key = name.lower()
                if key not in PROTOCOL_EMOTION_KEYS:
                    continue
                if isinstance(weight, bool) or not isinstance(weight, (int, float)):
                    continue
                w = float(weight)
                if w < 0 or w > 1:
                    continue
                mix.append({"name": key, "weight": w})
                total += w
            if total > 1.0 and total > 0:
                for m in mix:
                    m["weight"] = m["weight"] / total

        # parameterOverrides（C3：cdi3.json 白名单，运行时动态校验）
        po = data.get("parameterOverrides")
        if isinstance(po, dict):
            has_valid_field = True
            for k, v in po.items():
                if not isinstance(k, str) or k not in self._cdi3_parameter_ids:
                    continue
                if isinstance(v, bool) or not isinstance(v, (int, float)):
                    continue
                overrides[k] = v

        # motionLabel（C4：别名表，未知 → None）
        ml = data.get("motionLabel")
        if isinstance(ml, str):
            has_valid_field = True
            if ml.lower() in _MOTION_LABEL_ALIASES:
                motion = ml

        if not has_valid_field:
            return None
        return {
            "expressionMix": mix,
            "parameterOverrides": overrides,
            "motionLabel": motion,
        }

    def strip_expression_json(self, text: str) -> str:
        """剥离 LLM 回复中的表情 JSON 块，供 display/TTS 使用（与 Android stripStructuredJson 同规则）。

        ① 若文本含 FENCE 块 → 剥离全部 ```json/``` 块（DOT_MATCHES_ALL）；
        ② 无 FENCE 且 NAKED 解析成功（extract_expression_mix 非 None）→ 剥离该 NAKED 子串（首个 { 到末个 }）；
        ③ 其余原样返回。
        """
        # ① FENCE 块整体剥离（含 ``` 标记）
        stripped = _JSON_FENCE_RE.sub("", text)
        if stripped != text:
            return stripped
        # ② 无 FENCE → NAKED：结构化解析成功才剥离（保证不误剥正文花括号）
        if self.extract_expression_mix(text) is not None:
            naked = _JSON_NAKED_RE.search(text)
            if naked is not None:
                return text[: naked.start()] + text[naked.end():]
        # ③ 原样返回
        return text

