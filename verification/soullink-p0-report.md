# P0 Spike 报告 —— soullink-emotion-sdk × bai 皮套

> 日期：2026-08-22　分支：refactor/elegance　结论：**GO**（P1 双跑开关可启动）

## 执行内容
1. renderer 引入依赖（锁精确版本）：`@soullink-emotion/engine@0.1.0-beta.1`(runtime)、`@soullink-emotion/profile-generator@0.1.0-beta.1`(dev)。
2. 确定性生成 bai ModelProfile → `shared/model-profiles/bai.profile.json`（heuristic provider，未用 LLM refine）。
3. 无头冒烟 `renderer/tools/soullink-p0-smoke.mjs`：SoullinkRuntime 加载 profile，20s@30fps 共 600 tick。

## 结果
| 检查 | 结果 |
| --- | --- |
| 输出参数合法性 | 每帧输出 16 参数，**全部 ⊆ bai.cdi3 128 参数集，未知参数 0** |
| 眨眼 | ParamEyeLOpen 变化幅度 **0.887**（真实眨眼曲线）✓ |
| 呼吸 | ParamBreath 波动 **0.900** ✓ |
| FACS 覆盖 | 24 语义键映射 20；usedCdi 16/128（其余为物理随动链/非演出参数） |
| 能力检测（SDK CapabilityDetector） | headControl/bodyControl/eyeBlink/gazeControl/mouthOpen/mouthSmile/browControl/breath = **true**；**eyeSmile/blush/tear/sweat = false** |

## 关键发现
1. SDK 能力检测与人工 cdi 盘点**完全互证**：bai 缺眉形(→eyeSmile false)/腮红(blush)/泪汗特效——anger/sadness/害羞的面部细腻度上限受此限制，需靠节奏与台词补偿。
2. 启发式 profile 只映射标准演出参数（16 个）；**非标彩蛋参数（肩 BodyAngleX3/X6、胯 X4、腿 X5、翅膀×6、耳×6、光环×3）未自动入档** → "皮套专属彩蛋通道"需要手工扩展 profile 或启用 LLM refinement（P2 后评估）。
3. 生成器要求 modelDir 平铺（bai/runtime 嵌套不兼容）→ 冒烟脚本用 /tmp staging 解决；P1 若需常驻生成，考虑在 shared/model-profiles 放置手工维护版并在脚本中固化 staging 步骤。
4. npm audit 报 4 个漏洞（transitive dev 链）；与本引擎 runtime 零依赖无关，记录待后续统一处理。

## 对 P1/P2 的输入
- EmotionIntent 映射表确定：8key(neutral/joy/anger/sadness/surprise/fear/smirk/disgust) → engine 词表(happy/sad/angry/anxious?/surprised/fearful/smirk/disgusted)，以 engine 实际导出为准在适配层做显式表驱动；
- anger/sadness 表演力度受眉形缺失限制 → 由 MotionStyle(intensity/gestureFrequency) 与台词治理补偿；
- P2 默认切换仍门控于人工冒烟（无人值守不做主观视觉判定）。
