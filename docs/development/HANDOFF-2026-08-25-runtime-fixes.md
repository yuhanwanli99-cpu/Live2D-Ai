# 交接：PC 运行体验修复与开发者模式（2026-08-25）

> 状态：**本轮修改已落盘，工作暂停交接**。以下为当前工作区真实状态、已完成项、未完成项与下一轮入口。
> 注意：绝大多数改动仍是**未提交**工作区修改，接手者先看 git status。

## 一、本轮目标（用户定版）

1. 导演半身动作不明显、提示动作（点头/摇头等）不执行。
2. 口型与 TTS 强度/时序不同步。
3. 停止按钮无法清空旧播放队列缓存，旧 TTS 音频继续播。
4. 回复应像微信/QQ 一样多条短消息。
5. 长回复按音频块数量与真实播放时长逐步展示。
6. 新增默认关闭的本机开发者模式：可与导演对话/分析、列出并触发模型动作、预留彩蛋动作注册接口。

## 二、已落盘且验证通过

### A. Melo 句子误计修复（已完成，Kimi 原子任务）
- 文件：`Live2D-Ai-pc/open-llm-vtuber/src/open_llm_vtuber/tts/melo_worker.py`
- 修改：`_SENTENCE_END_RE` 改为 `[.!?。！？…\n]+`，连续省略号/标点只计 1 句。
- 新增：`Live2D-Ai-pc/open-llm-vtuber/tests/test_melo_worker.py`
- 验证：`pytest tests/test_melo_worker.py -v` → **18 passed**
- 修复现象：日志中 `文本包含约 6/7 个句子，超过上限 4` 不再误报（……/...... 之前每个字符计 1 句）。

### B. 开发者模式后端骨架（已落盘，验证部分通过）
- `src/open_llm_vtuber/api/dev_routes.py`：本地 only + 默认关闭（config `developer_mode` 或 `LIVE2DAI_DEV_MODE`）。
  - `GET /api/dev/actions/catalog`：动作白名单 + halfbody 能力。
  - `POST /api/dev/actions/trigger`：触发/释放动作（强度 1/2/3，未知动作 400）。
  - `POST /api/dev/director/analyze`：导演分析任意文本，返回脱敏 DirectorResult。
- `src/open_llm_vtuber/config_manager/system.py`：新增 `developer_mode: bool = False`。
- `src/open_llm_vtuber/server.py` / `routes.py`：已接线 `init_dev_routes`。
- `src/open_llm_vtuber/director.py`：新增 `sanitize_director_result`（脱敏）。
- `renderer/src/settings/api-client.ts`：新增 getDevCatalog / triggerDevAction / analyzeDevDirector。
- `renderer/src/ws-bridge.ts`：新增 `dev-action` 消息、`onDevAction`、`currentBodyGesture`、halfBody/motionStrength 透传。
- `renderer/src/dev-console/action-registry.ts`：彩蛋动作注册表（前端镜像 + 扩展点）。
- 验证：`pytest tests/test_halfbody.py tests/test_settings_api.py tests/test_route_architecture.py` → **36 passed**；
  renderer `choreography + bootstrap` → **14 passed**；`tsc --noEmit` 通过。

### C. 动作强度贯通（已落盘）
- `actions.motionStrength` 由导演 strength 填充（single_conversation）。
- renderer `bodyMotion.play(action, strength)` 已接入（ws-bridge）。
- `BodyActionPlayer.play(name, strength)` / choreography intensity 缩放已实现。

## 三、未完成（下一轮入口，按原子任务拆分）

1. **开发者模式前端控制台 UI**：`renderer/src/dev-console/dev-console.ts` 尚未创建/接入 `main.ts`；默认隐藏，localStorage 开关。
2. **设置页开发者模式开关**：SystemConfig 字段已有，UI 未接。
3. **dev 后端专项测试**：`tests/test_dev_routes.py` 未建（403 默认关闭 / localhost / 白名单 / 脱敏）。
4. **dev 前端专项测试**：`renderer/tests/dev-console.test.ts` 未建。
5. **取消后真正 `await tts_manager.interrupt()`**：single_conversation 取消路径仍待修改；当前 finally 只 clear()，旧 TTS 任务会继续跑并发送迟到 payload。
6. **前端音频 epoch/生命周期令牌**：`stopAudio`/新对话后丢弃迟到旧 audio 帧；audio.play() 成功后同步 `lipSync.play`（口型与真实播放时序一致）。
7. **微信式短消息展示**：每个 TTS chunk 对应独立 assistant 消息；长回复按音频真实播放节奏上屏，不再合成一条长气泡。
8. **显式动作命令触发**：用户输入 `点点头/摇摇头/歪头/张望/鞠躬/靠近` 直接触发对应动作（本地意图层）。
9. **Melo 连续省略号修改后全量回归**：`pytest tests/` 全量验证。

## 四、当前已知基线

- PC 后端 focused：36 passed（含本轮回归）。
- renderer focused：14 passed；tsc 通过。
- 全量 pytest 与全量 vitest **未**在本轮最后状态重跑（改动多，建议下一轮开头先全量）。

## 五、工作区纪律提醒

- 大量改动未提交；先 `git status`、`git diff` 审阅再继续。
- 机械改动继续按“单文件原子任务 + focused 测试”委派 Kimi（workflow 硬 600s 墙钟，任务必须 2-4 分钟完成）。
- 涉及导演/口型/停止/短消息的关键决策由主代理做；Kimi 只做机械实现与测试。
- 许可证纪律：SoulLink_Live2D 无 LICENSE，仅参考；soullink-emotion-sdk 为 MIT。
