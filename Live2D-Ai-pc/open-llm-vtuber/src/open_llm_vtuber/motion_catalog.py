"""PC Preview 半身安全动作目录（单一来源）。

只允许「半身安全」动作进入公开协议：头部 / 上身 / 视线类；不使用全身
跳跃等容易被渲染端误判为冲突的动作。`idle` 仅作为开发者面板的 release
语义，绝不能作为逐句 motion 下发给前端（避免重复触发回中冲突）。

该模块同时服务：
- `api/dev_routes.py`（开发者动作目录 / trigger 校验 / release 语义）；
- `director.py`（导演/规则/括注的 motion 边界收敛）；
- `single_conversation.py`（下游 payload 补充 source/priority/duration 元数据）。
"""

from __future__ import annotations

# 可触发的半身安全动作 id（渲染端 BodyActionPlayer / choreography 白名单子集）。
# 注意：
# - `bounce_happy`（蹦跳/全身）不在其中——需要整身位移，与半身参数驱动冲突，
#   已由导演映射到安全的 excited（上身雀跃）替代；
# - `wave_big`（挥手）也不在其中——bai 皮套无手臂参数，不能叫「挥手」，
#   相关语义由导演映射到 happy（上身开心摆动）的上身近似替代。
SAFE_MOTION_IDS: tuple[str, ...] = (
    "nod",
    "tilt",
    "sway",
    "happy",
    "surprise",
    "listen",
    "excited",
    "bow",
    "shake_no",
    "lean_in",
    "look_around",
)

# 仅作为 release（回到中性待机）的动作——绝不作为正向 trigger。
RELEASE_ONLY_MOTIONS: frozenset[str] = frozenset({"idle"})

# 触发端可接受的合法 id 集合（含 release 专用 id，供 400 校验）。
# 保持 `ALLOWED_ACTIONS` 名称以兼容既有 dev_routes 测试导入。
ALLOWED_ACTIONS: frozenset[str] = frozenset(SAFE_MOTION_IDS) | RELEASE_ONLY_MOTIONS

# 动作元数据：开发者目录 / payload 建议时长的单一来源。
# - label / description：中文显示，供开发者面板与日志阅读；
# - halfbody：该动作是否为半身安全动作（本目录内均为 True）；
# - suggested_duration_ms：渲染端执行该动作的推荐时长（前端可覆盖）；
# - release_only：仅作为 release 使用，不可作为正向 trigger。
MOTION_METADATA: dict[str, dict[str, object]] = {
    "nod": {
        "label": "点头",
        "description": "轻轻点头示意",
        "halfbody": True,
        "suggested_duration_ms": 900,
        "release_only": False,
    },
    "tilt": {
        "label": "歪头",
        "description": "歪头卖萌或疑惑",
        "halfbody": True,
        "suggested_duration_ms": 950,
        "release_only": False,
    },
    "sway": {
        "label": "摇摆",
        "description": "上身左右轻摆（模型无手臂/全身骨骼，使用上身近似）",
        "halfbody": True,
        "suggested_duration_ms": 1100,
        "release_only": False,
    },
    "happy": {
        "label": "开心",
        "description": "上身开心地左右摆动（模型无手臂，使用上身近似）",
        "halfbody": True,
        "suggested_duration_ms": 1150,
        "release_only": False,
    },
    "surprise": {
        "label": "惊讶",
        "description": "睁大眼睛、身体微微后仰",
        "halfbody": True,
        "suggested_duration_ms": 1000,
        "release_only": False,
    },
    "listen": {
        "label": "倾听",
        "description": "专注上身前倾倾听与思考",
        "halfbody": True,
        "suggested_duration_ms": 1200,
        "release_only": False,
    },
    "excited": {
        "label": "兴奋",
        "description": "上身雀跃地前后兴奋摆动（模型无手臂，无鼓掌）",
        "halfbody": True,
        "suggested_duration_ms": 1050,
        "release_only": False,
    },
    "bow": {
        "label": "鞠躬",
        "description": "上身前倾、低头弯腰行礼",
        "halfbody": True,
        "suggested_duration_ms": 1250,
        "release_only": False,
    },
    "shake_no": {
        "label": "摇头",
        "description": "头部左右摆动否认",
        "halfbody": True,
        "suggested_duration_ms": 1150,
        "release_only": False,
    },
    "lean_in": {
        "label": "靠近",
        "description": "上身前倾靠近",
        "halfbody": True,
        "suggested_duration_ms": 1150,
        "release_only": False,
    },
    "look_around": {
        "label": "左右张望",
        "description": "视线+头部环顾四周打量",
        "halfbody": True,
        "suggested_duration_ms": 1300,
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