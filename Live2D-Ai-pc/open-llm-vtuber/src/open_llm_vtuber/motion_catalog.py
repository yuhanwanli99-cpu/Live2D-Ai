"""PC Preview 头/脸安全动作目录（单一来源）。

只允许「按模型真实参数可诚实表达的头/脸动作」进入公开协议。
轴约定与渲染端 choreography.ts 一致：AngleX=左右转头(yaw)、
AngleY=上下点头(pitch)、AngleZ=侧倾(roll)；只写头/眼/眉/嘴参数，
绝不写任何 ParamBodyAngle*（身体/肩/胯/腿）通道。

基础六动作（任何头/脸模型都可降级表达，无能力门槛）：
- nod         → 头部 AngleY（pitch）上下点两拍后回中
- shake_no    → 头部 AngleX（yaw）左右摇两拍后回中
- tilt        → 头部 AngleZ（roll）向单侧倾斜保持后回中
- look_around → 眼球 EyeBallX 先看向一侧、头部 AngleX 随动，再转向另一侧后回中
- listen      → 头部 AngleZ/AngleY 轻微侧倾低首、眉毛 BrowLY/BrowRY 微抬，视线稳定向前
- surprise    → 眉毛 BrowLY/BrowRY 上挑、嘴 MouthOpenY 张开、头部 AngleY 轻微后仰

能力门控扩展动作（MOTION_REQUIRED_PARAMS 列出的参数在当前模型上全部存在
才会进入运行时目录；缺失则目录不显示，触发一律拒绝）：
- look_up / look_down   → 眼球 EyeBallY 先行 + 头部 AngleY 随动（需两者）
- glance_left / glance_right → 仅眼球 EyeBallX 快速侧瞥（头不动）
- wink_left / wink_right → 单眼 EyeLOpen / EyeROpen 快速闭合再睁开
- smile  → 嘴形 MouthForm 正向 + 眉毛微抬 + 双眼轻微眯起
- frown  → 眉毛下压 + 嘴形 MouthForm 负向
- pout   → 嘟嘴专用通道 Param4（bai 类模型的「嘟嘴」参数；无此参数不提供）
- wry_mouth → 歪嘴专用通道 Param6（无此参数不提供）
- deep_breath → 呼吸 ParamBreath 深吸后缓慢回落

强度语义（与渲染端 BodyActionPlayer.intensityOf 对齐）：
1=轻微但可见、2=正常清晰（原「3」的视觉水平）、3=强烈但安全 clamp
（渲染端逐帧 delta 与归一化值双重钳制，绝不越出模型 min/max）。

`idle` 仅作为开发者面板的 release 语义，绝不能作为逐句 motion 下发给前端。

不再包含任何「挥手/鼓掌/蹦跳/摇摆身体/鞠躬/凑近」等模型无法用参数诚实
表达的动作名（模型无手臂/手/全身骨骼/位置平移参数）；无法诚实映射的词
一律不产出动作（None）。

旧 provider 兼容：任何来源下发本目录之外的未知/已删 id，sanitize_motion
一律收敛为 None —— 旧后端/旧插件不需要升级即可安全共存。
"""
from __future__ import annotations

from collections.abc import Iterable

# 可触发的安全动作 id（渲染端 BodyActionPlayer / choreography 白名单子集）。
# 基础六动作 + 能力门控扩展十一动作；bounce_happy / wave_big / happy / excited /
# sway / bow / lean_in 一律不进入目录，任何来源产出这些 id 都会被丢弃为 None。
BASE_MOTION_IDS: tuple[str, ...] = (
    "nod",
    "shake_no",
    "tilt",
    "look_around",
    "listen",
    "surprise",
)

CAPABILITY_GATED_MOTION_IDS: tuple[str, ...] = (
    "look_up",
    "look_down",
    "glance_left",
    "glance_right",
    "wink_left",
    "wink_right",
    "smile",
    "frown",
    "pout",
    "wry_mouth",
    "deep_breath",
)

SAFE_MOTION_IDS: tuple[str, ...] = BASE_MOTION_IDS + CAPABILITY_GATED_MOTION_IDS

# 动作能力门槛：动作可进入运行时目录所需的最小参数集（全部命中才可用）。
# 基础六动作无门槛（缺参时渲染端按 profile 跳过对应通道，仍诚实可见）；
# 扩展动作的语义由这些通道构成，缺任何一个都无法诚实表达 → 不进目录。
MOTION_REQUIRED_PARAMS: dict[str, frozenset[str]] = {
    "look_up": frozenset({"ParamAngleY", "ParamEyeBallY"}),
    "look_down": frozenset({"ParamAngleY", "ParamEyeBallY"}),
    "glance_left": frozenset({"ParamEyeBallX"}),
    "glance_right": frozenset({"ParamEyeBallX"}),
    "wink_left": frozenset({"ParamEyeLOpen"}),
    "wink_right": frozenset({"ParamEyeROpen"}),
    "smile": frozenset({"ParamMouthForm", "ParamBrowLY", "ParamBrowRY"}),
    "frown": frozenset({"ParamMouthForm", "ParamBrowLY", "ParamBrowRY"}),
    # bai 类模型的非标面部通道：嘟嘴(Param4)/歪嘴(Param6)。多数模型没有，
    # 必须按运行时 cdi3 参数集门控——不存在则目录不显示。
    "pout": frozenset({"Param4"}),
    "wry_mouth": frozenset({"Param6"}),
    "deep_breath": frozenset({"ParamBreath"}),
}

_NO_REQUIREMENT: frozenset[str] = frozenset()


def motions_for_parameter_ids(parameter_ids: Iterable[str] | None) -> tuple[str, ...]:
    """按当前模型真实参数集过滤动作目录（运行时能力的单一裁决入口）。

    - parameter_ids 为空/None（模型未加载或不可知）→ fail-closed：
      只返回无门槛的基础六动作，所有能力门控动作不提供。
    - 返回保持 SAFE_MOTION_IDS 的声明顺序。
    """
    ids = {str(item) for item in (parameter_ids or ()) if item}
    return tuple(
        mid
        for mid in SAFE_MOTION_IDS
        if MOTION_REQUIRED_PARAMS.get(mid, _NO_REQUIREMENT) <= ids
    )


def allowed_actions_for_parameter_ids(parameter_ids: Iterable[str] | None) -> frozenset[str]:
    """运行时合法 id 集合 = 能力过滤后的动作 + release 专用 idle。"""
    return frozenset(motions_for_parameter_ids(parameter_ids)) | RELEASE_ONLY_MOTIONS

# 仅作为 release（回到中性待机）的动作——绝不作为正向 trigger。
RELEASE_ONLY_MOTIONS: frozenset[str] = frozenset({"idle"})

# 触发端可接受的合法 id 集合（含 release 专用 id，供 400 校验）。
# 保持 `ALLOWED_ACTIONS` 名称以兼容既有 dev_routes 测试导入。
ALLOWED_ACTIONS: frozenset[str] = frozenset(SAFE_MOTION_IDS) | RELEASE_ONLY_MOTIONS


def sanitize_motion(value: str | None) -> str | None:
    """动作边界收敛：任何来源的 motion 必须先过本目录白名单。

    返回 value 本身（小写且属于 SAFE_MOTION_IDS）或 None。这是后端所有
    payload 的最终防线：导演/括注/插件/用户命令产出已删动作 id 一律清空。
    """
    if not isinstance(value, str):
        return None
    key = value.strip().lower()
    return key if key in SAFE_MOTION_IDS else None


# 动作元数据：开发者目录 / payload 建议时长的单一来源。
# - label / description：中文显示，供开发者面板与日志阅读；description 必须
#   精确描述真实参数效果（含能力门控动作的门槛说明），不得出现挥手/鼓掌/
#   蹦跳/摇摆身体/鞠躬/凑近等模型无法诚实表达的语义；
# - halfbody：该动作是否为半身安全动作（本目录内均为 True）；
# - suggested_duration_ms：渲染端执行该动作的推荐时长（前端可覆盖）；
# - release_only：仅作为 release 使用，不可作为正向 trigger。
MOTION_METADATA: dict[str, dict[str, object]] = {
    "nod": {
        "label": "点头",
        "description": "头部 AngleY（pitch）上下点两拍后回中",
        "halfbody": True,
        "suggested_duration_ms": 900,
        "release_only": False,
    },
    "shake_no": {
        "label": "摇头",
        "description": "头部 AngleX（yaw）左右摇两拍后回中",
        "halfbody": True,
        "suggested_duration_ms": 1100,
        "release_only": False,
    },
    "tilt": {
        "label": "歪头",
        "description": "头部 AngleZ（roll）向单侧倾斜保持后回中",
        "halfbody": True,
        "suggested_duration_ms": 900,
        "release_only": False,
    },
    "look_around": {
        "label": "左右张望",
        "description": (
            "眼球 EyeBallX 先看向一侧、头部 AngleX 随动，"
            "再转向另一侧后回中"
        ),
        "halfbody": True,
        "suggested_duration_ms": 1300,
        "release_only": False,
    },
    "listen": {
        "label": "倾听",
        "description": (
            "头部 AngleZ/AngleY 轻微侧倾低首、眉毛 BrowLY/BrowRY 微抬，"
            "视线稳定向前"
        ),
        "halfbody": True,
        "suggested_duration_ms": 1100,
        "release_only": False,
    },
    "surprise": {
        "label": "惊讶",
        "description": (
            "眉毛 BrowLY/BrowRY 上挑、嘴 MouthOpenY 张开、"
            "头部 AngleY 轻微后仰"
        ),
        "halfbody": True,
        "suggested_duration_ms": 1000,
        "release_only": False,
    },
    # ---- 能力门控扩展动作（模型缺参数时目录不显示）----
    "look_up": {
        "label": "向上看",
        "description": (
            "眼球 EyeBallY 先行上移、头部 AngleY 随之轻抬，"
            "保持后回中（需模型同时具备 AngleY 与 EyeBallY）"
        ),
        "halfbody": True,
        "suggested_duration_ms": 1100,
        "release_only": False,
    },
    "look_down": {
        "label": "向下看",
        "description": (
            "眼球 EyeBallY 先行下移、头部 AngleY 随之低俯，"
            "保持后回中（需模型同时具备 AngleY 与 EyeBallY）"
        ),
        "halfbody": True,
        "suggested_duration_ms": 1100,
        "release_only": False,
    },
    "glance_left": {
        "label": "瞟一眼（一侧）",
        "description": (
            "仅眼球 EyeBallX 向负方向快速一瞥，头部不动，随即回中"
            "（屏幕左右取决于模型绑定，目录按参数方向诚实描述）"
        ),
        "halfbody": True,
        "suggested_duration_ms": 700,
        "release_only": False,
    },
    "glance_right": {
        "label": "瞟一眼（另一侧）",
        "description": (
            "仅眼球 EyeBallX 向正方向快速一瞥，头部不动，随即回中"
            "（屏幕左右取决于模型绑定，目录按参数方向诚实描述）"
        ),
        "halfbody": True,
        "suggested_duration_ms": 700,
        "release_only": False,
    },
    "wink_left": {
        "label": "眨左眼",
        "description": (
            "仅左眼 ParamEyeLOpen 快速闭合再睁开，右眼与视线保持默认"
        ),
        "halfbody": True,
        "suggested_duration_ms": 600,
        "release_only": False,
    },
    "wink_right": {
        "label": "眨右眼",
        "description": (
            "仅右眼 ParamEyeROpen 快速闭合再睁开，左眼与视线保持默认"
        ),
        "halfbody": True,
        "suggested_duration_ms": 600,
        "release_only": False,
    },
    "smile": {
        "label": "微笑",
        "description": (
            "嘴角上扬（ParamMouthForm 正向）、眉毛 BrowLY/BrowRY 微抬、"
            "双眼轻微眯起后回中"
        ),
        "halfbody": True,
        "suggested_duration_ms": 1600,
        "release_only": False,
    },
    "frown": {
        "label": "皱眉",
        "description": (
            "眉毛 BrowLY/BrowRY 下压、嘴角下压（ParamMouthForm 负向），"
            "保持后回中"
        ),
        "halfbody": True,
        "suggested_duration_ms": 1600,
        "release_only": False,
    },
    "pout": {
        "label": "嘟嘴",
        "description": (
            "嘟嘴通道 Param4 抬起保持后回中；当前模型无 Param4 时本动作"
            "不出现在目录、触发一律拒绝"
        ),
        "halfbody": True,
        "suggested_duration_ms": 1700,
        "release_only": False,
    },
    "wry_mouth": {
        "label": "歪嘴",
        "description": (
            "歪嘴通道 Param6 单侧拉动保持后回中；当前模型无 Param6 时"
            "本动作不出现在目录、触发一律拒绝"
        ),
        "halfbody": True,
        "suggested_duration_ms": 1400,
        "release_only": False,
    },
    "deep_breath": {
        "label": "深呼吸",
        "description": (
            "呼吸 ParamBreath 深吸抬升、屏息后缓慢呼气回默认"
            "（经仲裁器临时覆盖呼吸基线）"
        ),
        "halfbody": True,
        "suggested_duration_ms": 2200,
        "release_only": False,
    },
    "idle": {
        "label": "释放 / 回中",
        "description": "回到中性待机（仅 release）",
        "halfbody": True,
        "suggested_duration_ms": 500,
        "release_only": True,
    },
}