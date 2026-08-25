"""Preset-free half-body semantic control for the PC director.

The director emits model-independent semantic channels.  This module filters them
against the current model's CDI parameter IDs and clamps intent values before the
renderer maps them through the model's real Cubism min/default/max profile.
Mouth channels are intentionally excluded: lip sync remains the sole owner.
"""

from __future__ import annotations

import math
from collections.abc import Iterable
from dataclasses import dataclass
from typing import Any


@dataclass(frozen=True)
class SemanticChannel:
    param_ids: tuple[str, ...]
    minimum: float = -1.0
    maximum: float = 1.0


SEMANTIC_CHANNELS: dict[str, SemanticChannel] = {
    "headX": SemanticChannel(("ParamAngleX",)),
    "headY": SemanticChannel(("ParamAngleY",)),
    "headZ": SemanticChannel(("ParamAngleZ",)),
    "gazeX": SemanticChannel(("ParamEyeBallX",)),
    "gazeY": SemanticChannel(("ParamEyeBallY",)),
    "browY": SemanticChannel(("ParamBrowLY", "ParamBrowRY")),
    "browAngle": SemanticChannel(("ParamBrowLAngle", "ParamBrowRAngle")),
    "browForm": SemanticChannel(("ParamBrowLForm", "ParamBrowRForm")),
    "breath": SemanticChannel(("ParamBreath",), 0.0, 1.0),
    "bodyX": SemanticChannel(("ParamBodyAngleX",)),
    "bodyY": SemanticChannel(("ParamBodyAngleY",)),
    "bodyZ": SemanticChannel(("ParamBodyAngleZ",)),
}


def detect_halfbody_capabilities(parameter_ids: Iterable[str] | None) -> dict[str, bool]:
    """Return semantic-channel availability from a model parameter ID set.

    Pair channels require at least one side, so asymmetric models still degrade
    usefully instead of disabling the whole brow channel.
    """
    ids = {str(item) for item in (parameter_ids or ()) if item}
    return {
        key: any(param_id in ids for param_id in channel.param_ids)
        for key, channel in SEMANTIC_CHANNELS.items()
    }


def sanitize_halfbody_intent(
    raw: Any,
    parameter_ids: Iterable[str] | None,
) -> dict[str, float]:
    """Validate, capability-filter, and clamp one semantic half-body intent."""
    if not isinstance(raw, dict):
        return {}
    capabilities = detect_halfbody_capabilities(parameter_ids)
    out: dict[str, float] = {}
    for key, value in raw.items():
        channel = SEMANTIC_CHANNELS.get(str(key))
        if channel is None or not capabilities.get(str(key), False):
            continue
        if isinstance(value, bool) or not isinstance(value, (int, float)):
            continue
        number = float(value)
        if math.isnan(number) or number in (float("inf"), float("-inf")):
            continue
        out[str(key)] = min(channel.maximum, max(channel.minimum, number))
    return out


def available_halfbody_channels(parameter_ids: Iterable[str] | None) -> tuple[str, ...]:
    capabilities = detect_halfbody_capabilities(parameter_ids)
    return tuple(key for key in SEMANTIC_CHANNELS if capabilities[key])
