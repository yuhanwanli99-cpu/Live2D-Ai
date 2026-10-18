# 现行契约（一页，2026-10-13）

> 状态：**活** · 立档：2026-10-13 · 这一页是**现状**，不是沿革。
> 新人先读这一页；长文与逐条记录在 `AGENTS.md` 与 `docs/architecture/` 各专文，历史段落一行不改。

## 1. 产品是什么

**本地优先的皮套接入服务**：对话 → 按句 TTS → 口型跟音频时钟 → Live2D 渲染。
分句只由引擎给，音频播放时钟是唯一时间基准。桌宠**不是**底座：`pet-desktop` 已删除，不恢复。

## 2. 版本与工作区

- 版本号以 `Cargo.toml` 为准：**`0.3.1`**；发布说明 `docs/releases/v0.3.1.md`。`v0.2.7` 在 `5618b55`，`v0.2.6-rc.1` 在 `767a286`。
- tag `v0.2.5-rc.1` 仍在原处：导演开启时按句清洗并按原文出动作；出厂不设人设。
- **`v0.2.7` 是 2.0 稳定版，原 `v0.3.1` 内容是 `0.3.1-rc.1`（外部事件忙时合并）。** 其后各版与验收边界在 [`PROMPT-3.0-motion-first-2026-10-17.md`](../plans/PROMPT-3.0-motion-first-2026-10-17.md)：`0.3.1-rc.2` 基线拆分，`0.3.2-rc.1` 手势到期放回，`0.3.2-rc.2` 导演常驻 + 第二路并入核心，`0.3.3` 分仓库，`0.3.4` 多 TTS/LLM 厂商选择 + spawn-only `local-engine`，`0.3.5` 多 Live2D。其后 `0.4.0` 只做 Windows 支持，`0.4.1` 只做桌面透传的复杂 Mod。两版都还没点火。
- **版本号写法 `0.x.y-rc.z`（2026-10-18）**：`0.x.y` 通常是一块具体功能，下一块用下一个号。`z` 不事先钉死，由做这块的执行 agent 按代码拆成小任务；`0.x.y-rc.1` … `0.x.y-rc.z` 都在实现这块功能，每步换一队 agent，每步一次本地提交加一个本地附注 tag，不 push。最后一个才是 `0.x.y-rc.(z+1)`，只清理这一块的拆分、测试、文档和棘轮，不加功能，也不另开一个 `0.x.y`。`Cargo.toml` 仍是 `0.3.1`，本地 rc 不改版本串。2026-09-10 的本地 tag `v0.4.0`–`v0.4.5` 不在当前 `main` 上，远端也没有，已经删除，号留给上面的 `0.4.0` / `0.4.1`。`CHANGELOG.md` 里的同名旧条目是历史正文，不改。
- **两仓（0.3.3 起）**：核心在主仓 `/home/skystar/Live2D-Ai`，六个 Mod 实现住 sibling 仓 `/home/skystar/Live2D-Ai-mods`，主仓经 `path` 依赖引用。主仓**不再 standalone**；公开前换 git 依赖或 CI 双 checkout，见 [`two-repo-mods.md`](two-repo-mods.md)。
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
- **换模（0.3.5）**：iframe 热换、不重载 iframe；只认**匹配 URL 的 `loaded`**，
  旧/错帧忽略。失败保留旧模型并给「重试换皮」；成功后按新 model id 重载
  `/actions/field_map.json`，清旧贡献 / pending、保留 idle。
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
- `./scripts/ignite.sh`：等本机 `GET {tts}/models` = 200，才打印
  `http://127.0.0.1:18080/app/` 并打开浏览器；在此之前不报地址。
- **多厂商（0.3.4）**：`[llm].vendor` / `[tts].vendor` 只是标签，当前段始终是唯一权威；
  `[llm.vendors.*]` / `[tts.vendors.*]` 是文件里的快照。一次 PATCH 原子「快照→当前」复制；
  手改当前后 GET 只给 `vendor_modified`，不回写快照。内置虚拟 `melo` 快照不进文件
  （选中 = `apply_local_tts_defaults()` + `mode=local`），其它厂商一律 `mode=cloud`。
  每厂商只存 `api_key_env`（变量名），值仍只住 `.env`；GET 只给 `has_api_key`。
  本版只选不建厂商（TOML 手配）；旧 toml 无 vendor 字段时逐字 round-trip。
  **旧二进制读新 toml 可能因 `deny_unknown_fields` 失败**——rc 线接受。

## 8. Mod（在册六个）

`external-input`、`persona`、`voice-input`、`memory`、`local-tts-melo`、`local-engine`。

- **实现住第二仓** `../Live2D-Ai-mods/crates/`（`live2d-ai-mod-system` 与出厂协议仍在主仓）；
  工厂数、id、缺省集合不因拆仓改变。
- **缺省启用**：`external-input` 与 `local-tts-melo`；`local-engine` 只注册、**缺省停用**。
- `local-engine`（0.3.4）：spawn-only，4 项 schema（program / args_json / workdir / launch_mode），
  无 `base_url`；不写 `[tts]`/`[llm]`、不探活、不换地址、不重启；`apply_settings` 恒 0 次。
  立即退出（1s 窗口）→ Failed 带 stderr 尾；之后退出只进 `state_json`。
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
