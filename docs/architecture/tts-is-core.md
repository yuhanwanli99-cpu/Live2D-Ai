# 语音合成（TTS）是核心链路，不是 Mod

日期：2026-09-11
状态：**已实施**（用户裁决，v0.5.0）
适用对象：任何想把 TTS 做成 Mod / 插件 / 可选扩展的人

## 1. 裁决

> 「tts 本地从 mod 删除。明确本地 tts 唯一权威核心链路。这个升级。」

语音合成**不再是 Mod**。它和 LLM 一样，是核心链路的一环：

```
用户输入 → LLM（OpenAI 兼容 /chat/completions）
        → TTS（OpenAI 兼容 /audio/speech）
        → PCM → WS audio 帧 → 浏览器播放 + 口型
```

`crates/live2d-ai-mod-local-tts` 已删除；`AVAILABLE_MOD_FACTORIES` 从 5 个变为 4 个
（external-input / director / pet-desktop / local-llm）。
**2026-09-12（rc.2）更新**：`live2d-ai-mod-director` 也已删除（动作层裁决），
现为 **3 个**（external-input / pet-desktop / local-llm）。

> **2026-09-14（封存波）更新**：`wallpaper` / `pet-desktop` 已 **ARCHIVED**（移出
> `AVAILABLE_MOD_FACTORIES`、不再编译进 binary；crate 暂留 workspace，**禁止挂回**）。
> 当前注册数为 **5 个**：`external-input` / `persona` / `voice-input` / `memory` / `director`。
> 理由与恢复条件见 [ARCHIVED-mods.md](ARCHIVED-mods.md)。上文的 5/4/3 等数字是当时的
> 历史记录，保留不动。

## 2. 为什么（三条，按重要性排序）

1. **它在这条链路的中间，不是在旁边。**
   Mod 的语义是「扩展能力，可停用、可缺省关闭」。而 TTS 一旦缺失，
   链路就断了（只剩文字）。把中间环节做成「可插拔扩展」会让
   「默认配置下到底会不会出声」变成一个依赖 manifest 的问题——
   用户看到的是「TTS 坏了」。
2. **端点的唯一权威来源本来就只有一个。**
   `live2d-ai.toml` 的 `[tts]` 段：
   `base_url` / `voice` / `response_format` / `sample_rate` / `channels`。
   `live2d-ai-runtime` 直接读它、直接调用。Mod 做的事情是**探活后改写这个段**
   ——也就是说，它是在给一个已经权威的地方写值，而不是提供能力本身。
   多一层的结果是**两个地方能决定 TTS 端点**（配置文件 vs manifest），
   出问题时得同时看两处。
3. **它没有提供任何核心做不到的事。**
   `local-tts` 的能力只有：TCP 端口探活、HTTP `/v1/models` 探活、
   探到之后写 `base_url` + `voice` 并 `supervisor.reload()`。
   在 `externally_managed=true`（默认值）下**它连子进程都不 spawn**——
   等于「一个只在启动时跑一次的配置写入器」。这类东西不该占用 Mod 边界。

## 3. 具体改了什么

| 位置 | 改动 |
| --- | --- |
| `crates/live2d-ai-mod-local-tts/` | **删除整个 crate** |
| 根 `Cargo.toml` | workspace members 去掉该 crate |
| `crates/live2d-ai-desktop/Cargo.toml` | 去掉该依赖 |
| `main.rs::AVAILABLE_MOD_FACTORIES` | 5 → 4；两个 count/id 断言同步收紧 |
| `web_api/cli_entry.rs::default_mods_manifest` | 缺省只启用 `local-llm`；并加断言**守住它别再加回来** |
| `mods.json` | 去掉 `local-tts` 段（`local-llm` 保持 `enabled: false`） |
| `web_api/index.html`（遗留 JS 前端） | Mod 分区说明改为「TTS 不在 Mod 里」 |

**没有做的事**（刻意的）：没有把探活逻辑搬进核心。
配置已经写在 `live2d-ai.toml` 里了，再做一个「启动时探测并覆写」的核心逻辑
只是在核心内部重建同一个问题。权威来源就是那一个文件。

## 4. 后果（诚实记录）

- **TTS 端点不再自动发现。** 想换 TTS 服务就改 `live2d-ai.toml` 的 `[tts]`
  （或在 Flutter 的「语音合成」分区里改，它走 `PATCH /api/v1/settings`，
  写回的是同一个段）。**这是刻意的**：显式配置胜过启动时猜。
- **`[tts]` 缺失时链路照旧降级**：引擎走「空句」路径，只出文本、不报错。
  这条行为没变（`runtime/src/conversation/worker.rs`）。
- **旧的 `mods.json` 若仍写着 `local-tts`**，会被当作未知 id 忽略
  （manifest 只是启用开关表，引用了不存在的工厂不会让服务起不来）。
  已在 v0.5.0 里清掉了仓库根的那份。

## 5. 与 LLM 的不对称（为什么 `local-llm` 还在）

`local-llm` 暂时保留为 Mod。它和 `local-tts` 现在做的是同一类事
（探活 + 写 base_url），理论上也该一起收编。**本轮只动 TTS**——
用户只裁了这一条。这处不对称是**已知的**，不是遗漏；
若要统一，应该一次把「端点自动发现」整体从 Mod 里拿掉，而不是再拆一半。
## 6. 2026-10-09 追加：「本地拉起」可以回到 Mod 里，但**不许碰端点**

> **本节是追加**：第 1–5 节写的是 2026-09-11 的裁决与当时的实现，**一字不改**
> （那时被删的 `local-tts` 会「探活 → 改写 `base_url`」，删它是对的）。本轮新增的
> crate 复用了同一个 id / 显示名，职责收窄到一条：**拉起进程**。

维护者口径（2026-10-09）：允许一个**不改端点**的拉起 Mod。逐条：

| 项 | 本轮口径 |
| --- | --- |
| 出声地址 | 仍然只有 `live2d-ai.toml` 的 `[tts]`；「语音合成」设置页是它**唯一**的写入者 |
| 新 Mod | `crates/live2d-ai-mod-local-tts`（**同一个 crate 两个工厂**）：id `local-tts`（界面名 `本地tts_CosyVoice3-0.5B`）与 id `local-tts-melo`（`本地tts_MeloTTS`）。两者都**缺省停用**，都不进 `cli_entry::default_mods_manifest`；工厂数 6 → **7**（`main.rs::mod_count_is_seven`） |
| 它做什么 | 只按 `argv`（`program` = argv0 + `args_json` = JSON 字符串数组）`spawn` 一个外部进程。2026-10-09 起 `program` **留空 = 仓库内引擎脚本**（`engine/start.sh` / `melo/start.sh`，编译期锚定），不再等于「没有可执行文件」 |
| 它**不做**什么 | 不写 `[tts]` 的任何一个键（含 `ModServices.apply_settings`，调用次数**恒为 0**）；不探活；不改请求地址；停用它之后链路继续请求已经配好的地址 |
| 拉起时机 | `launch_mode`：`with_app`（**缺省**，键缺失也用它；Mod 已启用时 Boot / Enable / Apply / 命令 `launch` 都拉起）/ `on_apply`（只在「保存并应用」与命令 `launch` 时拉起）；**非法值在 `create` 返回 `Err`**，不折成缺省 |
| spawn 前预检 | 脚本不在 / 权重不在 / 解释器不在 → `start` 返回 `Err`（宿主标 `Failed`）。权重由 `engine/download.sh`（CosyVoice3，落 `weights/`，进 `.gitignore`）或 Git LFS（MeloTTS 的 `checkpoint.pth`）提供；**不重试、不回落仓库外的 `3-start.sh`、不改出声地址** |
| 进程形态 | `stdin=null`、`stdout` / `stderr` = inherit（**禁止管道**——没人读会把子进程堵死）；**禁止 `sh -c`**，逐参数传；`workdir` 非空才设 `current_dir`，不创建目录 |
| 挂掉时（维护者原话：「不管」） | spawn 失败 → `start` 返回 `Err` → 宿主现有路径把它标成 `ModStatus::Failed`；不重试、不换地址、不写看门狗、退出后不自动拉起、不把 `Err` 记完日志再返回 `Ok` |
| 状态面 | `state_json` 只用**非阻塞** `try_wait` 报 `pid` / `child_running` / `exit_code`；`try_wait` 自身出错 → 错误字符串进 `wait_error`，且**不**在快照里再 spawn、不改 `[tts]`、不把失败改写成「在跑」 |
| 停止失败 | `shutdown` 把子进程句柄**放回**运行时结构再返回该 `Err`；宿主 `disable` 因此把 runtime 放回槽位、不写 `enabled=false`、不写 `mods.json`；`restart` 也就不再 `start`（避免孤儿进程与双份进程） |
| 老 crate 那种做法 | 「探活再改写 `base_url`」**不恢复**——第 1–5 节的理由一条都没变 |

用户把地址指向本机、对面没有进程时，这一次合成请求按**现有错误帧**失败。
本轮不加新的失败通路，也不自动改走云端。

实现与单测在 `crates/live2d-ai-mod-local-tts/`；启动原因
（`StartCause { Boot, Enable, Apply }`）在 `live2d-ai-mod-system`，宿主接线在
`crates/live2d-ai-desktop/src/mod_registry/registry.rs`；前端把配置按钮文案改成
「保存并应用」（成功文案「已保存并生效」/「已保存，未启用」，且成功的配置保存
不再挂「需重新点火」常驻条）。

## 7. 2026-10-09（0.2.3-rc.1）现行口径：出厂出声是 MeloTTS；CosyVoice3 封存、未注册

> 本节是**现行句**。上面 §6 写的是同一轮稍早的状态（两个引擎都缺省停用、工厂数 7），
> 已被本轮取代，**保留不动**（历史段落不改）。

| 项 | 现行（0.2.3-rc.1） |
| --- | --- |
| 出厂出声 | `local-tts-melo`（界面名 `本地tts_MeloTTS`）：`cli_entry::default_mods_manifest` 收录，`enabled: true` + `launch_mode: with_app` —— 开机拉起仓库内 `melo/start.sh`（缺省监听 127.0.0.1:8091） |
| 出厂 `[tts]` | `base_url = http://127.0.0.1:8091/v1`、`voice = ZH`、`response_format = pcm`、`sample_rate = 44100`、`channels = 1`、**不设密钥**（代码缺省与 `live2d-ai.toml.example` 同步） |
| CosyVoice3 | `id local-tts` **已封存**：移出 `AVAILABLE_MOD_FACTORIES`，缺省 manifest 不收录，开机不拉起。crate 与 `engine/` **保留在树上**（可编译可测），**还要适配，未适配前不要挂回** |
| 工厂数 | 6（`external-input` / `persona` / `voice-input` / `memory` / `director` / `local-tts-melo`），`main.rs::mod_count_is_six` 守住 |
| 子进程立刻非 0 退出 | `start` 返回 `Err`、Mod 状态 `Failed`，**错误里带脚本印出的原因**（子进程 stderr 的尾巴）——不许显示「已启用」而其实没在听端口；退出码 0 同样算失败 |
| 不许碰的 | 起落 Mod **仍然不写 `[tts]` 的任何键**：`[tts]` 的唯一写入者是「语音合成」设置页（`PATCH /api/v1/settings`）与用户手改 `live2d-ai.toml` |
| BERT | 中文推理要的 `bert-base-multilingual-uncased` **不入库**；缺它就是失败（错误里逐字写缺的是它），不静默下载完还声称开箱 |
