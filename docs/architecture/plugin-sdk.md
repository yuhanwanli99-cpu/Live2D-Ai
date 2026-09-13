# Live2D-Ai 扩展接口 / 插件 SDK（历史，已被取代）

> **本页描述的是 Python 时代（`Live2D-Ai-pc/`）的插件系统，已归档；文中「动态加载插件」
> 「动作解析 → 半身动作」等能力在当前主线中不存在。现行 Mod 契约见
> [`mod-product-chain.md`](mod-product-chain.md)（冲突时以它为准）。保留本页仅为历史参考。

## 1. 核心链路（只打磨这些）

```
用户输入 ──► ASR/文本 ──► LLM(流式) ──┬──► TTS 合成 ──► 音频帧+口型能量 ──► Live2D 口型
                                     ├──► 情绪导演   ──► emotion 帧 ──────► 表情参数
                                     └──► 动作解析   ──► 括注位置时间轴 ──► 半身动作(播放位置触发)
```

| 环节 | 实现 | 说明 |
| --- | --- | --- |
| 口型 | `lip-sync.ts` + RMS 能量 | 行业通用做法：TTS 无音素时间轴，音量驱动 ParamMouthOpenY |
| 表情 | `emotion 帧` → EmotionConsumer → Arbiter | 8 key × 强度，fade 过渡 |
| 待机活人感 | 眨眼/呼吸/微表情/idle 小动作（idle/*） | 全部常驻、分层仲裁 |
| 动作 | motionTimeline 位置触发 → BodyActionPlayer | 编排库(choreography)/单姿态手势自动路由，仅用全皮套标准参数 |

## 2. 插件接口（PC，已落地）

```python
class MyPlugin:
    id = "my-plugin"
    def handle_event(self, name: str, payload: dict) -> dict: ...
```

- 注册：`plugins/native.py` 加 manifest（id/hooks/capabilities/default_enabled/entrypoint），或外部 entrypoint 动态加载。
- 宿主：`PluginHost.emit_event(name, payload)` 把事件派给所有启用插件；异常被捕获记录，永不炸主链路。
- 实例访问：`manager.host.get_instance(plugin_id)`（管理类路由用）。

### 2.1 钩子点全表（payload → 返回值语义）

| 钩子 | payload 关键字段 | 返回值作用 |
| --- | --- | --- |
| `input.before` / `input.after` | text | `context` 注入输入前缀 |
| `llm.before` | text, history | `context` 注入提示前缀（记忆/知识库/压缩都用它） |
| `llm.after` | text(AI 回复) | 记忆沉淀等副作用（返回值仅观测） |
| `sentence.after` | text, emotion, strength, mood_tier, motion, parameterOverrides | `motion` 覆盖该句动作；`parameterOverrides` 覆盖表情参数 |
| `live_event.before` | 弹幕事件 | 过滤/改写直播事件 |
| `tool` | 工具调用 | 工具执行结果 |
| `config` | 配置变更 | 配置联动 |

### 2.2 已内置的原生插件（13 个）

| 分类 | 插件 |
| --- | --- |
| runtime（有独立逻辑） | memory（正则即时层）、context-compactor、body-director、mood-tts-director、knowledge（本地文档检索注入）、web-search（DuckDuckGo/Tavily） |
| gated（核心路由实现，插件仅开关） | input-port、live-ingress、voice-assets、model-assets、config-pack |
| core | persona、asr |

管理端点（设置中心「插件」「记忆」页消费）：`/api/plugins*`、`/api/memory/facts*`、`/api/knowledge/documents*`、`/api/tools/web-search`。

## 3. 协议扩展面（前后端契约，改这里不改代码）

- `shared/persona.yaml`：人设/模型参数（双端单源）。
- `shared/emotion-protocol.md`：8 key 表情协议 + §J motionTimeline 括注动作帧（at∈[0,1) 播放进度）。
- `shared/model-adapter/*.adapter.json`：模型表情索引/参数帧/别名。
`shared/mcp_tools.json`：MCP 工具定义。
- WS 下行：`audio`（含 sentence_start / actions.motionTimeline）/ `emotion` / `subtitle` / `text-delta` / `control` / `live-event` / `error`。

## 4. 边界（核心不做）

- 手臂/手指级骨骼控制、3D 级动作（需专用绑定，不跨皮套通用）。
- 视线注视/看向谁等场景化注意力系统。
- 任何把插件逻辑写进核心模块的行为：新能力先问"能否用上面某个钩子表达"，能则做成插件。

## 5. Android Kotlin 骨架（草案，未接入构建）

```kotlin
interface Live2DAiPlugin {
    val id: String
    val version: String
    fun onLoad(host: PluginHost)
    fun onUnload()
}

interface ToolDefinition {
    val name: String
    val description: String
    val requiresConfirmation: Boolean
    suspend fun execute(argsJson: String): String
}
```

落地顺序与禁项沿用旧草案：不做热加载/AIDL/插件市场；不把插件写进 LoopCoordinator/VoiceIoController。