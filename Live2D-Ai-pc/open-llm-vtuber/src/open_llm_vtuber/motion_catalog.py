"""PC Preview 头/脸安全动作目录（单一来源）。

只允许「按 Bai 模型真实参数可诚实表达的六个动作」进入公开协议。
轴约定与渲染端 choreography.ts 一致：AngleX=左右转头(yaw)、
AngleY=上下点头(pitch)、AngleZ=侧倾(roll)；只写头/眼/眉/嘴参数，
绝不写任何 ParamBodyAngle*（身体/肩/胯/腿）通道：

- nod         → 头部 AngleY（pitch）上下点两拍后回中
- shake_no    → 头部 AngleX（yaw）左右摇两拍后回中
- tilt        → 头部 AngleZ（roll）向单侧倾斜保持后回中
- look_around → 眼球 EyeBallX 先看向一侧、头部 AngleX 随动，再转向另一侧后回中
- listen      → 头部 AngleZ/AngleY 轻微侧倾低首、眉毛 BrowLY/BrowRY 微抬，视线稳定向前
- surprise    → 眉毛 BrowLY/BrowRY 上挑、嘴 MouthOpenY 张开、头部 AngleY 轻微后仰

`idle` 仅作为开发者面板的 release 语义，绝不能作为逐句 motion 下发给前端。

不再包含任何「挥手/鼓掌/蹦跳/摇摆身体/鞠躬/凑近」等 Bai 模型无法用参数诚实
表达的动作名（模型无手臂/手/全身骨骼/位置平移参数）；无法诚实映射的词
一律不产出动作（None）。
"""
from __future__ import annotations

# 可触发的半身安全动作 id（渲染端 BodyActionPlayer / choreography 白名单子集）。
# 定版：仅以下六个；bounce_happy / wave_big / happy / excited / sway / bow /
# lean_in 一律不进入目录，任何来源产出这些 id 都会被丢弃为 None。
SAFE_MOTION_IDS: tuple[str, ...] = (
    "nod",
    "shake_no",
    "tilt",
    "look_around",
    "listen",
    "surprise",
)

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
#   精确描述真实参数效果，不得出现挥手/鼓掌/蹦跳/摇摆身体/鞠躬/凑近等语义；
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
    "idle": {
        "label": "释放 / 回中",
        "description": "回到中性待机（仅 release）",
        "halfbody": True,
        "suggested_duration_ms": 500,
        "release_only": True,
    },
}