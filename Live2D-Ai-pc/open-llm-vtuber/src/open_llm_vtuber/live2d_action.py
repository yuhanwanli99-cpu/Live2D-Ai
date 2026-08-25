"""主对话 LLM 可调用的 Live2D 动作原生工具（非 MCP）。

设计边界（任务定版）：
- 严格白名单：唯一暴露 ``live2d.perform_action`` 一个函数；``action`` 参数只能
  取 motion_catalog.SAFE_MOTION_IDS 六个头/脸安全动作（nod/shake_no/tilt/
  look_around/listen/surprise）。越界动作（含 release-only 的 idle、历史已删 id）
  一律拒绝并打可见日志，绝不静默映射成近似动作。
- 函数命名双轨：API schema 用 ``live2d_perform_action``（DeepSeek/OpenAI 要求
  function.name 匹配 ^[a-zA-Z0-9_-]+$，点号名会被 HTTP 400 拒绝）；展示名
  ``live2d.perform_action`` 仅用于插件面板/日志。工具调用解析双名兼容，
  协议回显一律归一为 API 名。
- 结果不朗读、不入记忆正文：工具结果只回灌 API 对话列表（协议需要），由
  BasicMemoryAgent 的原生工具循环保证不进入 TTS 文本与 self._memory。
- 动作绑定：合法调用产出结构化结果，由对话层绑定到对应下一句/当前句的
  Actions payload（随该句字幕/TTS 音频一起下发）。
- 可观测：全部调用计数（accepted/rejected）、最近动作/错误在 stats() 里，
  经 live2d-action 插件与 /api/plugins/health 暴露。
- 独立 ExpressionDirector 仍是兜底；主 LLM 显式工具调用优先级更高
  （见 single_conversation._apply_director 的优先级表）。

provider 支持：最小 OpenAI 兼容工具循环只挂在 OpenAICompatibleAsyncLLM 上；
Claude/Ollama 等 provider 走原简单路径（无工具），行为安全降级。provider 报告
不支持 tools（__API_NOT_SUPPORT_TOOLS__ 哨兵）时自动降级为纯文本对话，
并在本进程内不再提供工具（可见 warning 日志一次）。

环境开关：LIVE2DAI_NATIVE_TOOLS=off 可整体关闭（默认开启）。
"""

from __future__ import annotations

import os
import re
import time
from collections.abc import Iterable
from typing import Any

from loguru import logger

from .motion_catalog import (
    MOTION_METADATA,
    SAFE_MOTION_IDS,
    motions_for_parameter_ids,
    sanitize_motion,
)

# OpenAI/DeepSeek function-calling 协议对 function.name 的硬性约束：
# ^[a-zA-Z0-9_-]+$。点号不合法 → provider 直接 HTTP 400 拒绝整次请求
# （真机日志中「原生 tool 崩溃 / Error calling chat endpoint」的根因）。
OPENAI_TOOL_NAME_PATTERN = re.compile(r"^[a-zA-Z0-9_-]+$")

# 展示名（插件面板 / 日志 / 文档 / 内部事件元数据）：保留点号风格，
# 仅在进程内部使用，绝不进入 API schema 或协议回显。
LIVE2D_ACTION_TOOL_NAME = "live2d.perform_action"
# API schema 函数名：合法字符集，发给 provider 的唯一名字。
LIVE2D_ACTION_TOOL_API_NAME = "live2d_perform_action"


def is_valid_openai_tool_name(name: str | None) -> bool:
    """是否符合 OpenAI/DeepSeek function-calling 函数名字符集约束。"""
    if not name or not isinstance(name, str):
        return False
    return OPENAI_TOOL_NAME_PATTERN.fullmatch(name) is not None


def is_live2d_action_tool_name(name: str | None) -> bool:
    """工具调用解析双兼容：API 名（新）/ 展示名（旧，仅内部兼容，不再发送）。"""
    return name in (LIVE2D_ACTION_TOOL_API_NAME, LIVE2D_ACTION_TOOL_NAME)


def canonical_tool_api_name(name: str | None) -> str:
    """把已知别名归一为 API 合法名（协议回显防 400）；未知名原样返回。"""
    if name == LIVE2D_ACTION_TOOL_NAME:
        return LIVE2D_ACTION_TOOL_API_NAME
    return name or ""


# 模块加载即校验：API schema 函数名必须永远满足 provider 硬约束（防回归）。
if not is_valid_openai_tool_name(LIVE2D_ACTION_TOOL_API_NAME):  # pragma: no cover
    raise AssertionError(
        f"API 工具函数名 {LIVE2D_ACTION_TOOL_API_NAME!r} 必须匹配 "
        f"{OPENAI_TOOL_NAME_PATTERN.pattern}（DeepSeek/OpenAI 硬约束，"
        "违反会被 provider 以 HTTP 400 拒绝）"
    )

# 动作强度档位（与导演/前端约定一致：1 轻 / 2 中 / 3 强）
MIN_STRENGTH = 1
MAX_STRENGTH = 3
DEFAULT_STRENGTH = 2

# reason 字段上限：只用于日志/面板展示，防止超长输入
_MAX_REASON_CHARS = 200

# 启用工具时注入给主 LLM 的短系统指导（任务定版）：
# - 普通对话允许「克制地」用与语义匹配的动作增强表现，但每句最多 1 个、
#   不要每句必调 —— 矫正此前工具描述里「普通陈述句不要调用」导致的
#   普通聊天几乎从不调用；
# - 用户一条消息要求多个/多步骤动作时按顺序多次调用（每次一个动作）；
# - 禁止只在正文里用文字描述动作却不调用工具（裸文本动作描写治根）。
# 通过 build_guided_system_prompt 注入，绝不写入持久 system prompt / 记忆。
LIVE2D_ACTION_TOOL_GUIDANCE_MARKER = "[Live2D 动作工具使用规范]"
LIVE2D_ACTION_TOOL_GUIDANCE = (
    f"{LIVE2D_ACTION_TOOL_GUIDANCE_MARKER}\n"
    "- 普通对话中，若某句话的语义与某个动作自然契合"
    "（同意→点头、否认→摇头、惊讶→surprise、安慰→listen 等），"
    "你可以克制地调用一次动作来增强表现力；每句话最多调用 1 个动作，"
    "不要每一句话都调用，没有合适的动作就不要调用。\n"
    "- 用户在一条消息里要求多个或有先后顺序的动作"
    "（如「向下看然后向上看」「摇头然后点头」）时：按顺序逐个调用本工具，"
    "每个动作调用一次，不要合并、不要遗漏。\n"
    "- 只能通过调用工具执行动作：禁止只在回复正文里用文字描述动作"
    "（如「（点头）」「我点点头给你看」）却不调用工具。\n"
    "- 用户已明确要求的动作由系统保证执行，你无需为同一句重复调用。"
)


def build_guided_system_prompt(base_system: str | None) -> str:
    """基础人设 prompt + 工具使用指导（仅在原生工具循环内使用）。

    base 为空时只返回指导本身；拼接结果每轮现算、不落盘、不入记忆。
    """
    base = (base_system or "").rstrip()
    if not base:
        return LIVE2D_ACTION_TOOL_GUIDANCE
    return f"{base}\n\n{LIVE2D_ACTION_TOOL_GUIDANCE}"


def native_tools_enabled() -> bool:
    """全局开关：LIVE2DAI_NATIVE_TOOLS=off 时关闭原生工具特性（默认开启）。"""
    return os.environ.get("LIVE2DAI_NATIVE_TOOLS", "").strip().lower() != "off"


def build_live2d_action_tool_schema(
    allowed_actions: Iterable[str] | None = None,
) -> dict[str, Any]:
    """构造 OpenAI function-calling 格式的工具定义（严格白名单 enum）。

    allowed_actions 缺省用完整静态目录；传入能力过滤子集时 enum 同步收窄，
    保证「提示词里的目录」与「执行时的白名单」永不漂移。

    函数名使用 ``LIVE2D_ACTION_TOOL_API_NAME``：必须匹配
    ``^[a-zA-Z0-9_-]+$``（DeepSeek/OpenAI 硬约束，点号会被 HTTP 400 拒绝）。
    """
    actions = tuple(allowed_actions) if allowed_actions else tuple(SAFE_MOTION_IDS)
    return {
        "type": "function",
        "function": {
            "name": LIVE2D_ACTION_TOOL_API_NAME,
            "description": (
                "让 Live2D 形象在说话的同时做一个头部/面部小动作。"
                "语义自然契合时克制调用（例如同意时点头、否认时摇头、惊讶时睁大眼），"
                "每句话最多 1 个动作，没有合适的动作就不要调用；"
                "用户要求多个有先后顺序的动作（如「向下看然后向上看」「摇头然后点头」）时，"
                "按顺序逐个调用本工具。动作会自动绑定到你接下来要说的那句话上，"
                "绝不要在正文里用文字描述动作来代替调用本工具。"
            ),
            "parameters": {
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": list(actions),
                        "description": (
                            "要执行的动作 id（只能从枚举中选择）："
                            + "; ".join(
                                "{}={}".format(
                                    mid,
                                    (MOTION_METADATA.get(mid) or {}).get("label", mid),
                                )
                                for mid in actions
                            )
                        ),
                    },
                    "strength": {
                        "type": "integer",
                        "minimum": MIN_STRENGTH,
                        "maximum": MAX_STRENGTH,
                        "description": (
                            f"动作幅度 1~3（1 轻 / 2 中 / 3 强），缺省 {DEFAULT_STRENGTH}"
                        ),
                    },
                    "reason": {
                        "type": "string",
                        "description": "一句话说明为什么做这个动作（仅用于调试日志，不会被朗读）",
                    },
                },
                "required": ["action"],
            },
        },
    }


class Live2dActionTool:
    """``live2d.perform_action`` 的校验器 + 执行器 + 统计器。

    本类不做任何 LLM/网络操作；execute() 只做白名单校验并把合法动作记录为
    「待绑定」状态，真正的 Actions payload 组装由对话层完成。
    进程内共享一个默认实例（get_default_tool），插件与主循环共用同一份统计。
    """

    def __init__(self) -> None:
        self._enabled = True
        # 当前模型可诚实表达的动作子集（None = 未按模型能力过滤，用全目录）。
        # 由 service_context 在 Live2D 模型加载后经 configure_allowed_actions 更新。
        self._allowed_actions: tuple[str, ...] | None = None
        self._calls = 0
        self._accepted = 0
        self._rejected = 0
        self._last_action: str | None = None
        self._last_strength: int | None = None
        self._last_reason: str | None = None
        self._last_error: str | None = None
        self._last_call_at: float | None = None

    # ---- 能力描述 ----

    @property
    def name(self) -> str:
        """展示名（插件面板/日志用）；API schema 名见 api_name。"""
        return LIVE2D_ACTION_TOOL_NAME

    @property
    def api_name(self) -> str:
        """API schema 函数名（发给 provider 的唯一名字，合法字符集）。"""
        return LIVE2D_ACTION_TOOL_API_NAME

    @property
    def allowed_actions(self) -> tuple[str, ...]:
        """当前生效的动作白名单（默认完整静态目录；可按模型能力收窄）。"""
        return self._allowed_actions or tuple(SAFE_MOTION_IDS)

    @property
    def openai_schema(self) -> dict[str, Any]:
        return build_live2d_action_tool_schema(self.allowed_actions)

    def configure_allowed_actions(self, parameter_ids: Iterable[str] | None) -> None:
        """按当前模型真实参数集收窄白名单（与导演能力过滤同源同边界）。

        parameter_ids 为空/None → fail-closed：只保留无门槛的基础六动作。
        """
        filtered = motions_for_parameter_ids(parameter_ids)
        self._allowed_actions = tuple(filtered)
        logger.info(
            "[live2d.perform_action] 动作目录已按模型能力过滤: {} 个动作 {}",
            len(filtered),
            ", ".join(filtered),
        )

    def available(self) -> bool:
        """是否可向主 LLM 提供本工具（全局开关 + 插件启用状态）。"""
        return self._enabled

    def set_enabled(self, value: bool) -> None:
        """由 live2d-action 插件的 on_enable/on_disable 调用。"""
        self._enabled = bool(value)

    # ---- 执行 ----

    async def execute(self, arguments: Any) -> dict[str, Any]:
        """校验并接受/拒绝一次动作调用，返回回灌给 LLM 的工具结果。

        返回值同时是协议结果（JSON 序列化后作为 tool 消息内容），因此保持
        精简、不含任何会被误朗读的长文本。
        """
        self._calls += 1
        self._last_call_at = time.time()
        if not isinstance(arguments, dict):
            arguments = {}

        action_raw = arguments.get("action")
        action = sanitize_motion(action_raw if isinstance(action_raw, str) else None)
        reason = str(arguments.get("reason") or "").strip()[:_MAX_REASON_CHARS]

        # 双重白名单：先过静态目录（sanitize_motion），再过当前模型能力子集
        # （能力门控动作在模型缺参数时同样拒绝，绝不近似替换）。
        if action and action not in self.allowed_actions:
            action = None

        if not action:
            # 非法动作：拒绝 + 可见日志（绝不静默丢弃或近似替换）
            allowed = self.allowed_actions
            detail = f"invalid action {action_raw!r} (allowed: {', '.join(allowed)})"
            self._rejected += 1
            self._last_error = detail
            logger.warning(
                "[live2d.perform_action] 拒绝非法动作调用: {} (reason={!r})",
                detail,
                reason or None,
            )
            return {
                "ok": False,
                "error": detail,
                "allowed_actions": list(allowed),
            }

        strength = self._clamp_strength(arguments.get("strength"))
        self._accepted += 1
        self._last_action = action
        self._last_strength = strength
        self._last_reason = reason or None
        self._last_error = None
        logger.info(
            "[live2d.perform_action] 接受动作: action={} strength={} reason={!r} → 绑定下一句",
            action,
            strength,
            reason or None,
        )
        meta = MOTION_METADATA.get(action) or {}
        return {
            "ok": True,
            "action": action,
            "strength": strength,
            "duration_ms": int(meta.get("suggested_duration_ms", 1000)),
            "bound": "next_sentence",
        }

    @staticmethod
    def _clamp_strength(value: Any) -> int:
        try:
            strength = int(value)
        except (TypeError, ValueError):
            return DEFAULT_STRENGTH
        return max(MIN_STRENGTH, min(MAX_STRENGTH, strength))

    # ---- 可观测 ----

    def stats(self) -> dict[str, Any]:
        """健康/效果统计：设置页与 /api/plugins/health 直接消费。"""
        return {
            "tool": LIVE2D_ACTION_TOOL_NAME,
            "api_name": LIVE2D_ACTION_TOOL_API_NAME,
            "registered": True,
            "available": self.available(),
            "calls": self._calls,
            "accepted": self._accepted,
            "rejected": self._rejected,
            "last_action": self._last_action,
            "last_strength": self._last_strength,
            "last_reason": self._last_reason,
            "last_error": self._last_error,
            "last_call_at": self._last_call_at,
        }

    def describe_registration(self) -> dict[str, Any]:
        """注册信息快照（启动报告 / health API 用）。"""
        return {
            "tool": LIVE2D_ACTION_TOOL_NAME,
            "api_name": LIVE2D_ACTION_TOOL_API_NAME,
            "plugin": "live2d-action",
            "registered": True,
            "available": self.available(),
            "whitelist": list(self.allowed_actions),
        }


_default_tool: Live2dActionTool | None = None


def get_default_tool() -> Live2dActionTool:
    """进程内单例：agent 循环与 live2d-action 插件共享同一实例与统计。"""
    global _default_tool
    if _default_tool is None:
        _default_tool = Live2dActionTool()
    return _default_tool


def reset_default_tool() -> None:
    """测试专用：清空单例，避免用例间统计串扰。"""
    global _default_tool
    _default_tool = None
