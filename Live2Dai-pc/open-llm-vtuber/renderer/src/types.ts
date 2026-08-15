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

/** 动作（WS "audio" 消息的 actions 字段，后端 Actions.to_dict() 产出） */
export interface Actions {
  expressions?: Array<string | number> | null;
  pictures?: string[] | null;
  sounds?: string[] | null;
  /** motionLabel 别名 */
  motion?: string | null;
  /** 表情参数覆盖：key = Cubism 参数 ID，value = 目标值 0..1 */
  parameterOverrides?: ParameterOverrides | null;
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
