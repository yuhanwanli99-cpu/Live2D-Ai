# PC 无预设半身联动实现

> 范围：仅 PC（Python 导演 + TypeScript renderer）。Android 暂不接入。

## 数据流

`ExpressionDirector` 输出与模型无关的 `halfBody` 语义通道，随现有 `Actions` 和 audio WebSocket 帧下发：

`AI 句子 → director.halfBody → Actions.halfBody → ws-bridge → HalfBodyController → ParamArbiter → Cubism coreModel`

不增加新的 WebSocket 消息类型。

## 语义通道

- 头部：`headX/headY/headZ`，范围 -1..1
- 视线：`gazeX/gazeY`，范围 -1..1
- 眉毛：`browY/browAngle/browForm`，范围 -1..1；左右参数成对展开，单侧存在时自动单侧降级
- 呼吸：`breath`，范围 0..1
- 身体：`bodyX/bodyY/bodyZ`，范围 -1..1

嘴部参数不属于 halfBody，仍由 LipSync 单一来源控制。Arm/Hair 不直接驱动；发丝继续由 physics 响应头身参数。

## 无预设与能力降级

后端从当前模型 `cdi3.json` 的参数 ID 集合生成可用通道列表，导演结果在下发前会：

1. 丢弃未知语义键；
2. 丢弃当前模型不存在的通道；
3. 钳制语义范围；
4. 排除 mouth 等非半身白名单参数。

renderer 在模型加载后从 Cubism Core 读取每个参数的真实 `minimum/default/maximum`，建立运行时 Profile。语义值以 default 为中性点映射到实际范围，因此 body ±10、±12 或其他模型自定义范围均无需写死。

缺通道时只关闭该通道，其余头部/视线/眉/呼吸/身体继续工作。

## 兼容性

- 原 `parameterOverrides` 绝对值直通保留；`halfBody` 是新增的推荐无预设通道。
- 原 `motion` / `motionTimeline` / choreography 保留，可与语义参数渐进迁移。
- 半身写入使用 `ParamSource.Emotion`；口型的 `LipSync` 优先级更高，不会被抢占。

## 验证

- Python：`tests/test_halfbody.py` 覆盖能力检测、过滤、范围钳制和导演接线。
- Renderer：`renderer/tests/halfbody-profile.test.ts` 覆盖真实范围映射、能力降级、左右眉单侧降级和仲裁写入。
