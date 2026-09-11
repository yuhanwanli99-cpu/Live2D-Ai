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
