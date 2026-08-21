# -*- coding: utf-8 -*-
"""Built-in native plugin manifests for the PC backend."""

from __future__ import annotations

from .manifest import PluginManifest

# (id, name, hook_points, capabilities)
_NATIVE_PLUGINS: list[tuple[str, str, list[str], list[str]]] = [
    ("input-port", "Input Port", ["input.before", "input.after"], ["external_chat"]),
    ("live-ingress", "Live Ingress", ["live_event.before"], ["live_event"]),
    ("memory", "Memory", ["llm.before", "llm.after"], ["memory"]),
    ("persona", "Persona", ["llm.before"], ["persona"]),
    ("context-compactor", "Context Compactor", ["llm.before"], ["context_compaction"]),
    ("body-director", "Body Director", ["sentence.after"], ["motion"]),
    ("mood-tts-director", "Mood TTS Director", ["sentence.after"], ["mood", "tts_style"]),
    ("asr", "ASR", ["input.audio"], ["asr"]),
    ("knowledge", "Knowledge", ["llm.before"], ["knowledge"]),
    ("web-search", "Web Search", ["tool"], ["web_search"]),
    ("voice-assets", "Voice Assets", ["config"], ["voice_assets"]),
    ("model-assets", "Model Assets", ["config"], ["model_assets"]),
    ("config-pack", "Config Pack", ["config"], ["config_pack"]),
]


# 已实现运行时的原生插件 entrypoint（其余为 gated/core 实现，无纯占位）
_NATIVE_ENTRYPOINTS = {
    "memory": "open_llm_vtuber.plugins.native_runtime:MemoryPlugin",
    "context-compactor": "open_llm_vtuber.plugins.native_runtime:ContextCompactorPlugin",
    "body-director": "open_llm_vtuber.plugins.native_runtime:BodyDirectorPlugin",
    "mood-tts-director": "open_llm_vtuber.plugins.native_runtime:MoodTtsDirectorPlugin",
    "knowledge": "open_llm_vtuber.plugins.native_runtime:KnowledgePlugin",
    "web-search": "open_llm_vtuber.plugins.native_runtime:WebSearchPlugin",
}

# 各原生插件的 implementation 分类：
#   runtime - 拥有可加载运行时实例（entrypoint 非空）
#   gated   - 能力由核心路由实现，仅用 is_enabled 门控
#   core    - 能力由核心链路实现，非插件门控
#   stub    - 纯占位，无运行时、无核心实现
_NATIVE_IMPLEMENTATION = {
    # runtime (6)
    "memory": "runtime",
    "context-compactor": "runtime",
    "body-director": "runtime",
    "mood-tts-director": "runtime",
    "knowledge": "runtime",
    "web-search": "runtime",
    # gated (5)
    "input-port": "gated",
    "live-ingress": "gated",
    "voice-assets": "gated",
    "model-assets": "gated",
    "config-pack": "gated",
    # core (2)
    "persona": "core",
    "asr": "core",
}


_NATIVE_ZH = {
    "input-port": ("输入端口", "暴露外部文本/对话输入端口（HTTP/WS/文件管道）"),
    "live-ingress": ("直播事件输入", "通用直播/弹幕事件输入接口，不接具体平台"),
    "memory": ("长期记忆", "短期历史、长期事实、偏好与约定"),
    "persona": ("长程人设", "人设锚定、设定书与角色卡导入导出"),
    "context-compactor": ("上下文压缩", "分层摘要、token 预算与压缩计划"),
    "body-director": ("身体/表情导演", "半身动作、表情混合与 motion 选择"),
    "mood-tts-director": ("心情/TTS 导演", "心情分层与 TTS 风格/instruction"),
    "asr": ("语音识别", "语音输入转写"),
    "knowledge": ("知识库", "本地知识库检索与来源引用"),
    "web-search": ("联网查询", "联网搜索工具"),
    "voice-assets": ("声音资产", "参考音频与克隆音色管理"),
    "model-assets": ("模型资产", "Live2D 模型导入/删除/默认模型"),
    "config-pack": ("配置包", "配置导入/导出/重置"),
}


def register_all(manager) -> None:
    """Register every native plugin manifest into a PluginManager."""
    for plugin_id, name, hook_points, capabilities in _NATIVE_PLUGINS:
        name_zh, description_zh = _NATIVE_ZH.get(plugin_id, ("", ""))
        manager.register(
            PluginManifest(
                id=plugin_id,
                name=name,
                name_zh=name_zh,
                version="1.0.0",
                description="Built-in {} plugin".format(name),
                description_zh=description_zh,
                origin="native",
                hook_points=list(hook_points),
                capabilities=list(capabilities),
                config_schema={},
                default_enabled=True,
                entrypoint=_NATIVE_ENTRYPOINTS.get(plugin_id),
                implementation=_NATIVE_IMPLEMENTATION.get(plugin_id, "stub"),
            )
        )
