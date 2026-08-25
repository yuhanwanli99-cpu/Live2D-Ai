/**
 * renderer 共享类型定义
 *
 * WS 消息类型对齐 contract-pc-pipeline.md §P6 消息协议 + 后端实际产出
 * （src/open_llm_vtuber/utils/stream_audio.py prepare_audio_payload
 *   + src/open_llm_vtuber/live2d_model.py model_info）
 */

/** 表情参数覆盖：key = Cubism 参数 ID，value ∈ [0,1] */
export type ParameterOverrides = Record<string, number>;

/** 模型信息（WS "set-model-and-conf" 消息的 model_info 字段） */
export interface ModelInfo {
  /** model3.json 路径，如 "/live2d-models/白-免费版/runtime/白-免费版.model3.json" */
  url: string;
  /** 缩放倍率（半身构图，推荐默认 1.8，兼容旧前端 kScale 字段语义） */
  kScale?: number;
  /** 垂直偏移（px，正 = 下移；兼容旧前端 initialYshift） */
  initialYshift?: number;
  /** 水平偏移（px；兼容旧前端 kXOffset） */
  kXOffset?: number;
}

/** 字幕文本（WS "audio" 消息的 display_text 字段） */
export interface DisplayText {
  text: string;
  name?: string;
  avatar?: string | null;
}

/**
 * 动作时间轴帧（my-neuro 式"播放位置触发"）。
 * at ∈ [0,1)：该动作在所在音频块内的归一化播放进度（后端按文本位置线性近似）。
 *
 * 帧级调度元数据（Preview 协议可选，旧后端不下发 → 全部缺省）：
 *  - strength：该帧动作强度 1..3（主 LLM 工具动作逐帧携带）；
 *  - source：触发来源（'llm_tool' / 'stage_direction' / ...），透传给单 active
 *    调度器用于面板展示与日志；
 *  - priority：后端协议调度优先级（数值越大越优先：用户命令100 > llm_tool
 *    90 > 括注60 > 导演50），渲染端映射为 ActionPriority 档位；
 *  - reason：一句话触发原因（仅调试/面板展示，不参与调度）。
 */
export interface MotionCue {
  at: number;
  motion: string;
  strength?: number;
  source?: string;
  priority?: number;
  reason?: string;
}

/** 动作（WS "audio" 消息的 actions 字段，后端 Actions.to_dict() 产出） */
export interface Actions {
  expressions?: Array<string | number> | null;
  pictures?: string[] | null;
  sounds?: string[] | null;
  /** motionLabel 别名（整句一次性触发；存在 motionTimeline 时被忽略） */
  motion?: string | null;
  /** 导演动作强度 1/2/3；渲染端映射为确定性幅度 */
  motionStrength?: number | null;
  /** 表情参数覆盖：key = Cubism 参数 ID，value = 目标值 0..1 */
  parameterOverrides?: ParameterOverrides | null;
  /** 无预设半身语义通道（-1..1；breath=0..1），渲染端按模型真实范围映射 */
  halfBody?: Record<string, number> | null;
  /** 括注动作帧时间轴：存在时前端在音频播放到对应进度才触发，优先于 motion */
  motionTimeline?: MotionCue[] | null;
}

/** 元音嘴型帧（pymouth viseme，to_dict() 产出） */
export interface VisemeFrame {
  /** 嘴开合 0..1 */
  mouth_open_y?: number;
  /** 嘴型 0..1 */
  mouth_form?: number;
}

/** WS 消息：set-model-and-conf（模型切换/初始化） */
export interface SetModelAndConfMessage {
  type: 'set-model-and-conf';
  model_info: ModelInfo;
}

/** WS 消息：audio 帧（口型 volumes + 元音 visemes + 表情 actions + 字幕） */
export interface AudioMessage {
  type: 'audio';
  /** base64 wav（无声帧为 null） */
  audio?: string | null;
  /** 归一化 RMS 能量 [0..1]，每片一个值，index 对应时间片 */
  volumes: number[];
  /** 每片时长 ms（默认 20，正数） */
  slice_length: number;
  /** 元音嘴型帧，与 volumes 同长度（或 ≤ volumes） */
  visemes: VisemeFrame[];
  display_text?: DisplayText | null;
  actions?: Actions | null;
  /** 句首块标记：前端在该块开始播放时才上屏该句字幕（字幕-声音同步） */
  sentence_start?: boolean;
  forwarded?: boolean;
}

/** WS 消息：emotion 帧（P6 新增，独立表情帧） */
export interface EmotionMessage {
  type: 'emotion';
  /** 8 key 情绪枚举（如 joy/sad/angry/...） */
  emotion: string;
  /** 强度 1-3 */
  strength: number;
  /** 表情参数覆盖：key = Cubism 参数 ID，value ∈ [0,1] */
  parameterOverrides: ParameterOverrides;
  /** 过渡时长 ms（默认 400） */
  fadeMs: number;
}

/** WS 消息联合类型 */
export type WsMessage = SetModelAndConfMessage | AudioMessage | EmotionMessage;
