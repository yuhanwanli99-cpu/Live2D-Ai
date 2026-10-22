# 0.3.14 弹幕等播完，动作按句留下

> 状态：**已落地**（2026-10-22，本地 tag `v0.3.14-rc.1` = `d7e83f5`（宿主闸门 + FIFO）、
> `v0.3.14-rc.2` = `9cb8f9e`（按句 cue + 播放上报），rc.3 只清理，**未 push**）。
> `Cargo.toml` 仍 `0.3.1`，`pubspec` 仍 `0.3.1+22`。发布说明 [`v0.3.14-rc.3.md`](../releases/v0.3.14-rc.3.md)。

## 落地内容

- 下一条**外部**注入要同时满足「本轮生成已结束」与「浏览器里上一句已播完」才 `say` 一条；
  多出来的按到达顺序 **FIFO**，**禁止**用换行拼成一条。待发合计 2000 字符只丢当条；超时**不**倒队列。
- 同一轮后一句的 `action_cue` 只覆盖自己的 `sentence_seq`，还没开播的其它句子留在计划里；
  同 seq 留最后一条；空 `cues` 不再等于清空。`clear` 只在六处：停止 / `new_epoch` / baseline /
  换模 / 被接受的新 `user_line`（重复帧不清）/ 播放忙→闲且不在 `streaming`。
- 字幕仍到就上屏；聊天框自己的下一句不等待；`Stop` 与 `new_epoch` 仍立刻打断。
- 缺省二进制 `POST /api/v1/external/chat` 仍是 **404 `mod_disabled`**；逐条等待做成宿主模块
  `web_api/external_queue.rs` + `web_api/playback_gate.rs`，挂回 `external-input` 时由宿主 `push` / `pump`。
- 不改 `.env` / 仓库根 `mods.json` / `live2d-ai.toml`；不提交 `official_live2d_samples.7z`；不点火、不占 18080。
  工厂仍 **0**，desktop 依赖仍 **18**。七 Mod 挂载实验（本地未提交）未进入任何一笔提交。
- 门禁：`cargo test -p live2d-ai-desktop --all-targets` 544 passed、`fmt`、`clippy`；
  `flutter analyze` / `flutter test` 1734 passed；docs 预算通过。**浏览器肉眼未看**（本单不点火）。

> 实施正文（§工作区 / §原因 / §行为 / §实现 / §测试 / §文档）随 rc.1 原样入库，原文取回：
> `git show d7e83f5:docs/plans/PLAN-0.3.14-playback-queue-2026-10-22.md`
