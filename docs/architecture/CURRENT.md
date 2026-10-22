# 现行契约（一页，2026-10-13）

> 状态：**活** · 立档：2026-10-13 · 这一页是**现状**，不是沿革。
> 新人先读这一页；长文与逐条记录在 `AGENTS.md` 与 `docs/architecture/` 各专文，历史段落一行不改。

## 1. 产品是什么

**本地优先的皮套接入服务**。产品侧的**核心**只指最小能跑起来的一套：Live2D 表演 + AI 对话 + 动作。有且只有这三块：

1. **一路表演 LLM**：当前这一路 `[llm]`，纯对话。
2. **二路导演**：按句清洗，把这一句送上屏、送 TTS、并据原文出动作。0.2.0 之后新进入核心的只有这一路。
3. **一个前端**：渲染、动作、设置、对话。这是会话系统本身。

分句只由引擎给，音频播放时钟是唯一时间基准。TTS 出声是第 2 块把句子送出去的那一跳，端点在 `[tts]`，可以是云端 API。`local-tts-melo` 只在端点是本机 Melo 时拉起那个进程，本身是 Mod（缺省不挂，见 §7/§8），不是这跳出声。

记忆、角色卡、外部事件、语音输入、本地引擎、A2A 都是扩展，归 Mod，各自一个本机仓库（§8），主仓不依赖它们。工程目录里的 crate 名叫 `core` / `runtime` / `desktop`，只说明代码放哪，**不**用来判断一个功能是不是核心。桌宠不是这套最小实现：`pet-desktop` 已删除，不恢复。

## 2. 版本与工作区

- 版本号以 `Cargo.toml` 为准：**`0.3.1`**；发布说明 `docs/releases/v0.3.1.md`。`v0.2.7` 在 `5618b55`，`v0.2.6-rc.1` 在 `767a286`。
- tag `v0.2.5-rc.1` 仍在原处：导演开启时按句清洗并按原文出动作；出厂不设人设。
- **`v0.2.7` 是 2.0 稳定版，原 `v0.3.1` 内容是 `0.3.1-rc.1`（外部事件忙时合并）。** 其后各版与验收边界在 [`PROMPT-3.0-motion-first-2026-10-17.md`](../plans/PROMPT-3.0-motion-first-2026-10-17.md)：`0.3.1-rc.2` 基线拆分，`0.3.2-rc.1` 手势到期放回，`0.3.2-rc.2` 导演常驻 + 第二路并入核心，`0.3.3` 分仓库，`0.3.4` 多 TTS/LLM 厂商选择 + spawn-only `local-engine`，`0.3.5` 多 Live2D。其后 `0.4.0` 只做 Windows 支持，`0.4.1` 只做桌面透传的复杂 Mod。两版都还没点火。
- **版本号写法 `0.x.y-rc.z`（2026-10-18）**：`0.x.y` 通常是一块具体功能，下一块用下一个号。`z` 不事先钉死，由做这块的执行 agent 按代码拆成小任务；`0.x.y-rc.1` … `0.x.y-rc.z` 都在实现这块功能，每步换一队 agent，每步一次本地提交加一个本地附注 tag，不 push。最后一个才是 `0.x.y-rc.(z+1)`，只清理这一块的拆分、测试、文档和棘轮，不加功能，也不另开一个 `0.x.y`。`Cargo.toml` 仍是 `0.3.1`，本地 rc 不改版本串。2026-09-10 的本地 tag `v0.4.0`–`v0.4.5` 不在当前 `main` 上，远端也没有，已经删除，号留给上面的 `0.4.0` / `0.4.1`。`CHANGELOG.md` 里的同名旧条目是历史正文，不改。
- **0.3.6 已收口（2026-10-18，本地 tag `v0.3.6-rc.1`…`v0.3.6-rc.4`，未 push）**：热换原子化（panic 未复现，结构性删除二次 load）、设置纪律（草稿不生效，只有「保存并重载」）、cue / field map 重绑（`model_epoch`）。发布说明 `docs/releases/v0.3.6-rc.4.md`。
- **0.3.7 已收口（2026-10-18，本地 tag `v0.3.7-rc.1`…`v0.3.7-rc.3`，未 push）**：LLM `[llm].mode = cloud | local`；动作 12 → 21（D1=A 手不做）。发布说明 `docs/releases/v0.3.7-rc.3.md`。
- **0.3.8 已落地（2026-10-18，本地 tag `v0.3.8-rc.1`…`v0.3.8-rc.3`，未 push）**：当前这一路 `[llm]` 对外暴露 A2A（agent card + JSON-RPC `message/send` / `tasks/get`）；皮套回合仍纯对话、不调工具，皮套链路未接 A2A。产品侧 A2A 是扩展，不是核心；A2A 已在 0.3.9 进第二仓 Mod `a2a`（缺省停用），宿主只留门 `web_api/a2a_gate.rs`，与 §1 的归属一致。不并进 `0.4.0` / `0.4.1`。发布说明 `docs/releases/v0.3.8-rc.3.md`。真机补验（Pi 从 GitHub 克隆、浏览器看隐藏）见 [`PLAN-0.3.8-verify-pi-browser-2026-10-18.md`](../plans/PLAN-0.3.8-verify-pi-browser-2026-10-18.md)。真 Ollama 本轮不拉模型，记为 3.0 遗留，不写入 `0.4.0`。
- **0.3.9 已落地（2026-10-19，本地 tag `v0.3.9-rc.1` / `v0.3.9-rc.2` 同在提交 `6d841e4`（主仓非 standalone，拆两次会让 rc.1 不能单独构建），`v0.3.9-rc.3`（主仓 `a36421a`，第二仓同 tag `519a3e0`）逐个落地，rc.4 只清理（tag `v0.3.9-rc.4` 在 `41096fa`），未 push）**：A2A（含已写进 `web_api` 的路由）拆出核心，进第二仓新 Mod `a2a`（缺省停用，工厂 6 → 7）；TTS/LLM 的多厂商选择器留在核心，但收成「云端 / 本地 + 二级设置」，厂商目录与 `vendors.*` 快照已删；旧 toml 仍能加载、写回不再写出 `vendor` / `vendors`。发布说明 [`docs/releases/v0.3.9-rc.4.md`](../releases/v0.3.9-rc.4.md)。不并进 `0.4.0` / `0.4.1`。范围 [`PLAN-0.3.9-core-boundary-2026-10-19.md`](../plans/PLAN-0.3.9-core-boundary-2026-10-19.md)。
- **0.3.10 已收口（2026-10-19，本地 tag `v0.3.10-rc.1` = `3a7f6c0`、`v0.3.10-rc.2` = `cab3996`，rc.3 只清理（tag `v0.3.10-rc.3` 在 `6a0a1ef`），未 push）**：标红修复（`live2d_stage.dart` 844 → 785 + 新 `director_cue_plan.dart`；pytest Melo 路径指第二仓）、项目声明收成单一 AGPL-3.0（商业付费 = 单独书面协议，见 §11）、皮套入库与仓库分发分开、活文档对齐；`0.4.0` 未开始。发布说明 [`docs/releases/v0.3.10-rc.3.md`](../releases/v0.3.10-rc.3.md)。范围 [`PLAN-0.3.10-red-docs-statement-2026-10-19.md`](../plans/PLAN-0.3.10-red-docs-statement-2026-10-19.md)。
- **仓库（2026-10-19，已落地）**：主仓只留核心链路，克隆并填好 `[llm]` / `[tts]` 就能编译并跑，不需要任何 Mod 实现。`AVAILABLE_MOD_FACTORIES` 为空（工厂 **0**），`default_mods_manifest` 的 `mods` 为空对象——缺省不再启用 `external-input` 与 `local-tts-melo`。七个实现各自一个本机仓库，公开地址见 §8（分支 `main` 已推）：`/home/skystar/live2d-ai-mod-external-input`、`-persona`、`-voice-input`、`-memory`、`-local-tts`（id `local-tts-melo`，工厂 `MELO_FACTORY`）、`-local-engine`、`-a2a`。旧目录 `/home/skystar/Live2D-Ai-mods` 保留作历史来源，已不被主仓引用。挂载 = 维护者手动把 crate 的 path 依赖加回根 `[workspace.dependencies]` 与 desktop 依赖、加回 `AVAILABLE_MOD_FACTORIES`、恢复该 Mod 的宿主路由/集成代码；没有动态加载器，缺省提交里没有这些。执行单 [`PLAN-main-core-mod-repos-2026-10-19.md`](../plans/PLAN-main-core-mod-repos-2026-10-19.md)。项目经理 2026-10-19 抽查验收：主仓 `15cdfa077946eb1540c47bdc9eaf04c5524f3174`（父提交即 `v0.3.10-rc.3`），**不占版本号、未打 tag**。分支 `main` 已快进推到 `origin`（`7178e6f..15cdfa0`）；附注 tag 与已删除的远端 `v0.3.1` 都不在这次推送里。七个 Mod 公开仓库已建并推了 `main`，见 §8。附注 tag 仍只在本地。全门禁按执行报告，本会话未重跑。接受的偏差：persona 取消对 memory 的 dev-dep 与 3 条共存测试；external / voice / a2a 的真实 handler 从父提交取回；`turn_in_flight` 保留并 `allow(dead_code)`；`live2d-ai-runtime` 四处历史注释未改；`AGENTS.md` 历史段未改；Flutter Web 未重编。S2 仍开：`external-input` 与 `memory` 仍调用 `secrets::lookup`。`0.4.0` 仍未开始。
- **0.3.11 已落地（2026-10-20，本地 tag `v0.3.11-rc.1` = `a728d4a`（点火）、`v0.3.11-rc.2` = `c7e5913`（启动器），rc.3 只清理（tag `v0.3.11-rc.3` 在 `85dc9e2`），未 push）**：点火不再为 local TTS 空等 600 秒、不再清理 Melo 子进程；`GET /` 与 `GET /index.html` 是启动器页（200；已注册数量只读；本次确认与本次拉起只在已编进二进制的 Mod 上改；预启动开关缺省关），加载动画结束后进入现有 `/app/`。只探测当前这一份 `[llm].base_url` 与 `[tts].base_url`，报告可达性，不改档、不写 toml；没有第二份地址，不恢复 `vendors.*`。工厂仍是 0。不并进 `0.4.0` / `0.4.1`。执行单 [`PLAN-0.3.11-launcher-2026-10-20.md`](../plans/PLAN-0.3.11-launcher-2026-10-20.md)，发布说明 [`docs/releases/v0.3.11-rc.3.md`](../releases/v0.3.11-rc.3.md)。
- **0.3.12 已落地（2026-10-20，本地 tag `v0.3.12-rc.1` = `acc9ea4`（清单预检）、`v0.3.12-rc.2` = `28bfa69`（启动器「已发现」），rc.3 只清理（tag `v0.3.12-rc.3` 在 `8ca0201`），未 push）**：启动器只读扫描工作目录 `mods/<id>/mod.toml`（`GET /api/v1/mods/discovered`），显示已发现 / 不可用（带原因）；不执行 `entry` / `launch.program`、不 spawn、不把文件夹写进 `AVAILABLE_MOD_FACTORIES`（工厂仍 0），保留 id（`director`、`local-llm`、`wallpaper`、`pet-desktop`、`local-tts`）不可选。点「进入」：工厂仍走 0.3.11 的启停，可选文件夹**只记勾选**（写 `mods.json`，不启停）。不改 `mod-product-chain` §8 与 `mod-community-license` §2（仍是静态链接、不做动态 `.so`）。不并进 `0.3.11` / `0.4.0` / `0.4.1`。执行单 [`PLAN-0.3.12-mod-folder-scan-2026-10-20.md`](../plans/PLAN-0.3.12-mod-folder-scan-2026-10-20.md)，发布说明 [`docs/releases/v0.3.12-rc.3.md`](../releases/v0.3.12-rc.3.md)。
- **0.3.13 已落地（2026-10-21，本地 tag `v0.3.13-rc.1` = `747a1aa`（子进程）、`v0.3.13-rc.2` = `225fd81`（启动器），rc.3 只清理（tag `v0.3.13-rc.3` 在 `3528c65`），未 push）**：勾选的文件夹在点「进入」时启动清单里的程序（`process` 与 `launch.mode = with_app`）；`on_demand` 只记勾选；扫描与刷新仍不执行。Linux 子进程带 `PDEATHSIG=SIGTERM` + 独立进程组，工作目录 = Mod 目录，不经过 shell、不传 `.env` 与密钥环境变量。启动失败不影响进入。不编回工厂（仍 0），不并进 `0.3.12` / `0.4.0` / `0.4.1`。执行单 [`PLAN-0.3.13-folder-spawn-2026-10-21.md`](../plans/PLAN-0.3.13-folder-spawn-2026-10-21.md)，发布说明 [`v0.3.13-rc.3.md`](../releases/v0.3.13-rc.3.md)。
- **0.3.14 已落地（2026-10-22，本地 tag `v0.3.14-rc.1` = `d7e83f5`（宿主闸门 + FIFO）、`v0.3.14-rc.2` = `9cb8f9e`（按句 cue + 播放上报），rc.3 只清理，未 push）**：下一条外部注入要「本轮生成已结束」且「浏览器里上一句已播完」才逐条说，多出来的按 FIFO 排队，不再用换行拼进同一次（待发合计 2000 字符只丢当条，超时不倒队列）；同一轮的动作按 `sentence_seq` 留下，后一帧只覆盖自己那一条，清空只在六处（停止 / `new_epoch` / baseline / 换模 / 被接受的新 `user_line` / 播放忙→闲且不在 streaming）。聊天框自己的下一句不等待，停止与 `new_epoch` 仍立刻打断。缺省二进制的外部入口仍是 404 `mod_disabled`（`web_api/external_queue.rs` 等挂回 `external-input` 时由宿主 `push` / `pump`）。不并进 `0.3.13` / `0.4.0` / `0.4.1`。执行单 [`PLAN-0.3.14-playback-queue-2026-10-22.md`](../plans/PLAN-0.3.14-playback-queue-2026-10-22.md)，发布说明 [`v0.3.14-rc.3.md`](../releases/v0.3.14-rc.3.md)。
- **多 Live2D（0.3.5）**：设置 → 模型库可加载本机 `model3`。唯一真源仍是
  `model_registry.json.active_id`；`GET /api/v1/app/status` 同时派生 `active_model_url`
  （空 = 无模型），不加 DisplayPrefs/localStorage 字段、不加端点。白模 `bai` 出厂
  seed、可从列表切回、不可删除；active 指向坏 entry / 缺 `model3` 时启动按无 active
  回 bai（id/URL 同步回退，不写盘）。未授权模型不入库（`assets/models` 白名单只有
  `README.md` / `.gitkeep` / `bai`），用户须自持授权。参数名不匹配的动作继续丢弃并
  沿用 ack 原因；不新增 remap 表。**未验**：第二张合法 `model3`、浏览器肉眼、真实服务
  HTTP 换模、公开远端 / CI；换模 / 回滚 / 字段映射仅 fixture 单测。known minor：
  `ModelsSection` 仍硬编码 `bai`（后端 builtin id 真源），`FieldMap::for_model` 无直接单测。

## 3. 出厂不设人设

`system_prompt` 为空。角色卡仍然是 `live2d-ai-mod-persona`：用户**自己启用**后导入
（粘贴 JSON / 选 `.json` / 带 `chara` 的 `.png`），停用即还原。出厂不带作者人设。

## 4. 导演与动作

- **导演常驻开启（无开关）**：第二路**按句清洗**；这一句的**原文切片**既上屏、也送口型、也据它出动作。
  依据是**这一句原文**，不是按用户输入打五类情绪。
- 开 `[performance].enabled` 只接管 `segments` / `speak`（TTS / 上屏真源）；
  **动作仍按每段原文走核心规则表**（不是无动作），表演层自己的 `cues` 不投递。
- 动作链路：`action_cue` → 前端 → 渲染面 `preset`（表情槽 / 手势槽）。
  **不走 core reducer**；`ModServices.action_tx` 仍是休眠 sender。
- **动作包（0.3.7）**：12 → **21 条**（12 表情 + 9 手势，不含 `none`）；维护者裁决
  **D1=A 手不做**——不扩 13 参数白名单，wave / clap / point / cover_mouth / shrug 不做，
  不用头身冒充。新增 8 表情（angry/sad/blush/wink/confused/sleepy/excited/calm）+ `bow`；
  未知 id 仍 Ignore，`none` / handoff / release 语义不变；默认 emotion→preset 映射未改。
  参数名不匹配的模型（如 Epsilon_free 的旧式大写 `PARAM_*`）继续丢弃并沿用 ack 原因，不加 remap 表。
- **换模（0.3.5 / 0.3.6）**：iframe 热换、不重载 iframe；只认**匹配 URL 的 `loaded`**，
  旧/错帧忽略。渲染面 `model_epoch` 首装 1、只有成功换模 +1；`loaded` 带 `model_id` /
  `model_epoch` / `field_map{source,degraded,reason}`，失败 `error` 带当前旧模型身份且**不换 epoch**。
  失败保留旧模型（同 GpuContext 新核心装载成功才原子替换，**不再回滚二次 load**）并给「重试换皮」；
  成功后按新 model id 重载 field map、清旧贡献 / pending、保留 idle；Flutter 按新 epoch 清
  `DirectorCuePlan` / `PresetStatus` 并重设动作幅度上下文，旧 epoch 的 preset 帧渲染面 Ignore。
- 同槽换动作从屏幕上的姿势接上。表情进出走曲线。撤销从屏幕值收回；手势到期也走同一条放回（`ttl × (1-HOLD_END)`），表情到期仍立即清槽。
- 第二路失败（请求失败 / 超时 / 坏 JSON）：回退**按句规则表**。
- 设置一级「**导演**」卡：常驻状态 + 动作幅度滑条；`dev_mode` 下才显示可观测四栏与调试面板。
  「扩展」区不再有导演卡，也**没有启用开关**。

## 5. 表演倾向（三档）

| 档 | 行为 |
|---|---|
| `restrained` 克制 | 提示词照旧；选不出就给空数组，**不**回退规则表 |
| `normal` 平常（**缺省**） | 提示词更愿意选一个；仍**不**回退规则表 |
| `eager` 主动 | 1~2 个；只有它对该句原文命中的规则表动作回退 |

三档由宿主从 `mods.json` 的既有 `mods.director.config.motion_bias` 读取（界面不显示，不新增配置键）。
动作幅度滑条走界面底部「**保存并重载**」：后端热重载，**进程不重启**。

## 5.1 设置与保存纪律（0.3.6）

- 设置**表单字段**只进草稿：改滑条 / 开关 / 输入框不写盘、不重建引擎、不向舞台或音频下发。
  **切换分区、关闭设置、点取消都是零写盘零下发**——舞台与音频继续用已保存那份。
- 只有底部「**保存并重载**」在 Mod 表单草稿 + 服务端草稿 + 本机偏好草稿**全部成功**后才生效，
  并且**只刷新一次**；任一步失败不刷新、草稿保留（写盘成功但热重载失败也不刷新，让人看见提示就地重试）。
- 动作幅度（全局三键与每模型覆盖）也走这条：舞台**只吃已保存值**，保存成功后才下发一帧；
  拖动 / 放弃 / 保存失败都不下发。
- 显式动作按钮保留各自立即语义：模型 activate、保存 `.env` 密钥、Mod 启停、Mod 卡片「保存并应用」。
- 写明例外只有两个：模型 activate（独立立即动作）；`dev_mode` 调试临时幅度覆盖（显式开发工具，不落盘）。
- 旧计划书（如 `PLAN-settings-reload-2026-10-10.md`）与本页冲突处，以本页为准。

## 6. 长数字与本地 TTS

- 连续 **17 位及以上**数字：送 TTS 的那份换成「一长串数字」，**屏幕上仍是原句**。
- 本地 TTS 回「最长支持 16 位」时：该句**静音**，整轮继续（不再把整轮判失败）。

## 7. TTS 与点火

- 出厂 TTS 是**本机 Melo**：`http://127.0.0.1:8091/v1`（`live2d-ai.toml` 的 `[tts]`）。
- `./scripts/ignite.sh`（0.3.11）：等 `GET /` = 200（**启动器页**；仍是 302 的旧构建回落
  等 `GET /app/` = 200）后打印 `http://127.0.0.1:18080/` 并打开浏览器；**不再等本机 TTS**——
  旧行为（`[tts].mode=local` 时等 `GET {tts}/models` 最长 600 秒、超时退出 1）已删。
  `mode=cloud` 本来就不等 8091；现在 local 也不等。点火**不拉起任何 Mod**，也**不清理**
  用户自己拉起的 Melo 进程（只替换本服务的 `pkill live2d-ai-desktop --web`）。
- **0.3.12 已落地**：启动器增加「已发现」文件夹清单——只读工作目录
  `mods/<id>/mod.toml`（`GET /api/v1/mods/discovered`），只校验、**不执行**
  `entry` / `launch.program`、不 spawn，也不把文件夹写进 `AVAILABLE_MOD_FACTORIES`
  （缺省工厂仍 0）。保留 id（`director`、`local-llm`、`wallpaper`、`pet-desktop`、
  `local-tts`）不可选；扫描失败 / 单份清单无效不阻止启动，也不阻止进入 `/app/`。
  执行单 [`PLAN-0.3.12-mod-folder-scan-2026-10-20.md`](../plans/PLAN-0.3.12-mod-folder-scan-2026-10-20.md)。
- **`/` 是启动器页（0.3.11 / 0.3.12）**：`GET /` 与 `GET /index.html` 返回一页（200、`no-store`），
  `/app/` 与 `/app/*` 仍是现有 Flutter、直接打开不跳回。页面只做三件事：
  - 三个数字（**没有数字输入框**）：**已发现**（可选行条数 = 校验通过的文件夹 + 已编进
    二进制的工厂；无效文件夹不计入，但列在下面并带原因）、**本次确认**（勾选条数）、
    **本次拉起**（预启动开着时，勾选里的 `local-tts-melo` / `local-engine` **工厂**条数；
    文件夹里的 `process` / `launcher` 本版不计入）。工厂 0 且没有文件夹时三个数字
    都是 0，仍可进入。
  - 文件夹行来自只读 `GET /api/v1/mods/discovered`；`mods.json` 里 `enabled = true`
    的可选行**初始勾选**。「刷新」只重扫，不执行、不写盘。
  - **预启动**开关缺省关；拨动只改页面数字，不发请求、不写 `live2d-ai.toml` / `mods.json`。
  - 点「进入」后播放加载动画：工厂 id 按勾选调用**现有**启停接口（预启动关时拉起类
    id 一律 disable；开且勾选才 enable）；可选文件夹 id **只把勾选写进 `mods.json`**
    （`POST /api/v1/mods/discovered`，不 enable / disable、不 spawn，其余 id / 键原样
    保留）。然后对**当前这一份** `[llm].base_url` 与 `[tts].base_url` 各探测一次
    （`GET {base_url}/models`，404 算可达、401/403 算鉴权失败、空地址不发请求），
    再进入 `/app/`。探测**只报告、不改档**：不改 `mode` / `base_url`、不改本次进程
    出站端点、不写 toml、不改 `.env`；没有「另一档」可探（`vendors.*` 不恢复）。
    `0.3.9` 那条「**local 连不上不会偷偷切 cloud**」继续有效。
- **缺省构建不挂 `local-tts-melo` Mod**，Melo 不会由主链自动拉起：本机出声要么按 §8 挂回该 Mod，要么自己准备 Melo 进程。点火不再等它，也不再替用户清理它。
- **云端 / 本地两档（0.3.9 收束，原 0.3.4 多厂商）**：LLM 与 TTS 各一个模式开关，
  二级设置直接挂在当前这一档下面；**不再有**厂商目录、`vendors.*` 快照、
  `vendor_modified` 或「一次 PATCH 把快照复制到当前段」。
  - **LLM `[llm].mode`**（0.3.7 起）：`cloud | local`，缺省 **cloud**，省略不写
    （旧 toml 无此键**逐字 round-trip**）。transport 不变（同一条 OpenAI-compatible
    `/chat/completions`）；local 只表示「端点在用户本机 / 局域网、通常不需要云 key」，
    **不嵌模型、不下载权重、不加依赖、不预填厂商或 Key**。二级字段：`base_url` 地址、
    `model` 模型名、`api_key_env` 密钥变量名（值仍只住 `.env`，GET 只给
    `has_api_key`）、`show_reasoning` 思考开关（只决定要不要上屏；思考**永不进**
    句子装配器与 TTS）。
  - **TTS `[tts].mode`**（0.3.9 起）：`cloud | local`，缺省 **local**（本机 Melo）。
    二级字段：`base_url` 地址、`voice` 音色、`model` 模型名（可空）、
    `response_format` 格式（缺省 `pcm`）、`sample_rate` 采样率（缺省 44100）、
    `channels` 声道（缺省单声道）、`api_key_env` 云端密钥变量名。界面：本地档只读
    固定一组（`127.0.0.1:8091/v1` / `ZH` / `pcm · 44100 Hz · 单声道` / 无密钥），
    云端档可填地址 / 音色 / 模型名并在保存后绑密钥；采样率与声道在开发者层；
    `response_format` 只住 `[tts]`，设置面板不出这个控件。`mode=cloud` 时核心只打
    `[tts].base_url`，**不要求** `local-tts-melo` 在跑（点火也不等 8091）。
  - 切档只是标签，仍走同一次「保存并重载」：**不预填**厂商 / Key / 地址，
    也**不自动回退**另一个模式（local 连不上不会偷偷切 cloud）。云端 TTS 要求同一次
    保存里地址与音色非空，否则整体 400、本次不生效。
  - **旧 toml**：0.3.4–0.3.8 的 `[llm].vendor` / `[llm.vendors.*]` / `[tts].vendor` /
    `[tts.vendors.*]` 读入时**忽略**（不让 `deny_unknown_fields` 拦下启动），
    下一次保存**不再写回**这两键，手写注释保留。旧机制只作历史留证，见
    [`docs/releases/v0.3.9-rc.4.md`](../releases/v0.3.9-rc.4.md)。
  - 连通性自检沿用 TTS 纪律：`GET {base_url}/models`，404 算可达 + note，401/403
    `auth_failed`，其余 `protocol_error`，**不真跑对话 / 不合成**；本地没配置 / 连不上回
    `llm_local_unreachable`（不显示 ready）。

## 8. Mod（缺省 0；七个实现各一本地仓库）

`AVAILABLE_MOD_FACTORIES` 为空，工厂 **0**；`default_mods_manifest` 的 `mods` 为空对象，缺省**不启用**任何 Mod（含 `external-input` / `local-tts-melo`）。七个实现各自一个**本机仓库**，主仓不依赖、缺省不编译：

| 本机仓库 | 公开仓库 | id |
|---|---|---|
| `/home/skystar/live2d-ai-mod-external-input` | [live2d-ai-mod-external-input](https://github.com/yuhanwanli99-cpu/live2d-ai-mod-external-input) | `external-input` |
| `/home/skystar/live2d-ai-mod-persona` | [live2d-ai-mod-persona](https://github.com/yuhanwanli99-cpu/live2d-ai-mod-persona) | `persona` |
| `/home/skystar/live2d-ai-mod-voice-input` | [live2d-ai-mod-voice-input](https://github.com/yuhanwanli99-cpu/live2d-ai-mod-voice-input) | `voice-input` |
| `/home/skystar/live2d-ai-mod-memory` | [live2d-ai-mod-memory](https://github.com/yuhanwanli99-cpu/live2d-ai-mod-memory) | `memory` |
| `/home/skystar/live2d-ai-mod-local-tts` | [live2d-ai-mod-local-tts](https://github.com/yuhanwanli99-cpu/live2d-ai-mod-local-tts) | `local-tts-melo`（工厂 `MELO_FACTORY`） |
| `/home/skystar/live2d-ai-mod-local-engine` | [live2d-ai-mod-local-engine](https://github.com/yuhanwanli99-cpu/live2d-ai-mod-local-engine) | `local-engine` |
| `/home/skystar/live2d-ai-mod-a2a` | [live2d-ai-mod-a2a](https://github.com/yuhanwanli99-cpu/live2d-ai-mod-a2a) | `a2a` |

- 七个公开仓库的分支 `main` 已推；旧目录 `/home/skystar/Live2D-Ai-mods` 保留作历史来源，已不被主仓引用。`live2d-ai-mod-system` 与出厂协议仍在主仓。主仓缺省构建仍不依赖它们。
- **挂载**由维护者手动完成：把该 crate 的 `path` 依赖加回根 `[workspace.dependencies]` 与 `live2d-ai-desktop` 依赖、加回 `AVAILABLE_MOD_FACTORIES`（并同步计数 / id 断言）、恢复该 Mod 的宿主路由 / 集成代码。**没有动态加载器**；缺省提交里没有这些。
- **未挂载时**：`POST /api/v1/external/chat`、`POST /api/v1/voice/transcript`、`GET /.well-known/agent-card.json`、`POST /api/v1/a2a` 一律回 **404**，body `error.code = "mod_disabled"`，不打 LLM、不 `say`。
- `a2a`（0.3.9）：协议核在本机仓库 `live2d-ai-mod-a2a`（agent card、JSON-RPC dispatch、`message/send` / `tasks/get`、进程内任务表上限 32、错误脱敏）；宿主只留门 `web_api/a2a_gate.rs`——loopback + Origin + Content-Type 校验与「Mod 未挂载则不调用」。挂回后线格式与 0.3.8 逐字一致（取消仍拒绝）；密钥仍只在宿主按 `[llm].api_key_env` 查，Mod 与响应 / 日志 / 卡片都拿不到值。
- `local-engine`（0.3.4）：spawn-only，4 项 schema（program / args_json / workdir / launch_mode），无 `base_url`；不写 `[tts]`/`[llm]`、不探活、不换地址、不重启；`apply_settings` 恒 0 次。立即退出（1s 窗口）→ Failed 带 stderr 尾；之后退出只进 `state_json`。**2026-10-18 裁决：不删除、不注销。** 设置 → 扩展不再显示这张卡；缺省未挂载（见上表）。出声仍是 `local-tts-melo`（挂回后），对话云 / 本地仍是 `[llm].mode`。
- `memory`：本地 JSONL + 词重叠检索（**零向量云依赖**），缺省**关**。
- `director` 的 crate 仍在主仓 workspace（规则表已迁入核心），但**退出 `AVAILABLE_MOD_FACTORIES`、不再投递，禁止挂回**；
  旧 8 个配置键（3 个词表映射 + 5 个 `staging_*`）不再出现在界面，`mods.json` 残留键被忽略。
- `wallpaper`、`pet-desktop`、`local-llm` **已删除，禁止挂回**（台账 `docs/architecture/ARCHIVED-mods.md`）。

## 9. 开发工具

`dev_mode=true` 才有开发工具。

- 「**导演可观测**」四栏随设置一级「导演」卡，在 `dev_mode=true` 时显示；
  A 栏（决策参数）是静态「核心常驻（无决策账本）」——导演已不是 Mod，不读 `/api/v1/mods/director/state`。

- 「**B 事件流**」列出的是**前端已收到的帧**，不是第二路模型的原文。
- 该栏默认**不列出**「口型时钟」与「段结束」。

## 10. 读法

这一页和源码冲突时以源码为准；和更旧的架构文冲突时以这一页为准；
小改动不新写交接书、台账或发布说明。

## 11. 声明

- 项目自有代码的**公开许可证只有一份：AGPL-3.0-only**（`LICENSE`）；没有第二份公开许可证。
- AGPL **允许**商业使用：照 AGPL 履约即可，不需要另买许可。
- 想**避开 AGPL 的源码义务**（闭源 / 托管服务等），是**商业付费 + 单独书面协议**，不是第二份公开许可证；联系 QQ `338438383`，见 `COMMERCIAL-LICENSE.md`。
- 第三方不在本声明内：字体（OFL-1.1）与渲染栈 / 上游依赖（如 Ayagami，MIT OR Apache-2.0）保持原条款；`crates/l2d` 的 **MIT OR Apache-2.0** 属上游兼容的第三方部分，不在单一 A3 声明内改写；模型资源不计入，白模条约仍以 `assets/models/bai/MODEL_LICENSE.md` 为准。
- 本页与 `README.md`、`COMMERCIAL-LICENSE.md`、`LICENSE-SCOPE.md` 同一口径；声明里的 QQ 只写在这四份文件里（`README.zh-CN.md` 只指向 `COMMERCIAL-LICENSE.md`）。
