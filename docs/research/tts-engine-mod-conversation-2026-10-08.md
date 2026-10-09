# 本地 TTS 显存与「本地 TTS Mod」对话记录（2026-10-08）

> 性质：**对话记录 + 维护者口径澄清**，**不是实施计划**。本轮**没有改任何代码 / 配置 / 模型**。
> 配套实测数据见 [tts-vram-selection-2026-10-08.md](tts-vram-selection-2026-10-08.md)（显存三态、项目自身占用、CPU 分担、候选矩阵）。
> 触发：维护者「本项目 + 本地的 CosyVoice3 显存占用过多，分析一下 TTS 选型」。

---

## 1. 本轮问答流水

| # | 维护者问 | 结论（证据在配套文档） |
| --- | --- | --- |
| Q1 | 本项目 + 本地 CosyVoice3 显存占用过多，分析 TTS 选型 | CosyVoice3 常驻 4.72 GB / **合成峰值 5.37 GB**；Windows 桌面另占 2.1–2.7 GB；全卡峰值 7.56 / 8.15 GB（余 ~590 MB）。换引擎成本极低：`[tts]` 是唯一权威 + OpenAI 兼容 shim，项目主链零改动 |
| Q2 | 项目自身资源占用多少 | Rust 后端 **RSS 23.7 MB、0 显存、0.25 ms/请求**；前端静态 30 MB + wasm 渲染面 5.4 MB + 模型 24 MB；**舞台渲染在浏览器里**，量级百 MB（不可按标签页拆分） |
| Q3 | TTS 合成峰值显存多少 / 能否 CPU 分担 | 峰值 **5 366 MB**（三态见配套文档 §2.1.1）。CPU 能分担**冷路径**（tokenizer 0.29–1.38 s，省 ~0.9 GB）；**不能**分担热路径（全链 CPU RTF 12.6–15.2×、RSS 6.5 GB） |
| Q4 | CosyVoice2-0.5B 能适配吗？想做成内置 Mod | 技术上适配成本极低（shim 的 `load_model` 见到 `cosyvoice2.yaml` 自动用 `CosyVoice2` 类；音色 API 继承自基类；4.52 GB / 19 文件；Apache-2.0）。但「做成 Mod」与 2026-09-11 裁决有接缝 → 见 §3、§4 |
| Q5 | （澄清）Mod 只是注册、拉起外挂 TTS、暴露 API、TTS 选择里多一个「本地 TTS」选项；TTS API 本身也可以走云端（最早 Python 时代用千问，太贵） | 记入 §2「维护者口径」，并据此改写「Mod 边界」的合法形状（§4） |

---

## 2. 维护者口径（2026-10-08，本轮澄清）

1. **Mod 的职责 = 注册 + 拉起外挂 TTS 进程 + 暴露 API + 在 TTS 选择里多一个「本地 TTS」选项。**
   它不是「实现 TTS」，也不是「决定端点」——引擎本体（CosyVoice2 等）住在仓库外的独立进程里，经 OpenAI 兼容 shim 提供能力。
2. **TTS 的 API 可以走云端。** 端点是不是本地，与是不是核心链路无关；本地 TTS 只是**其中一个可选引擎**。
3. **成本是硬约束**：Python 时代用过千问（Qwen）云端 TTS，**因为太贵被换掉**。云端档若要复活，先摆费用模型，不能只比延迟与音质。

> 这三条**没有推翻** 2026-09-11「TTS 是核心链路」的主干（链路中间环节不可缺省关闭、`[tts]` 是唯一权威）；
> 它们澄清的是「**引擎从哪来、怎么被拉起、怎么被选择**」——这一层可以由 Mod 承担。

---

## 3. 现行裁决与本次澄清的接缝（要变真源需一次修订）

| 现行文本 | 落在哪 | 与本次澄清的关系 |
| --- | --- | --- |
| 「语音合成不再是 Mod……`crates/live2d-ai-mod-local-tts` 已删除」 | `docs/architecture/tts-is-core.md` §1 / §3 | 当年删的是**「探活 + 写 `[tts].base_url`」的配置写入器**（理由：等于给已权威的地方再写一次值）。本次澄清的 Mod **不写 `[tts]`**，只拉起进程 + 提供选项 → 没有被直接否掉，但文本上没给它留出口 |
| Mod 红线：禁止写 `[tts].base_url` / `api_key_env` / `voice` / `model` / `speed` / `sample_rate` / `channels` / `response_format` | `docs/architecture/director-mod-v0.md` §红线 | **保持不变即可**：写 `[tts]` 仍归 core 的「语音合成」分区（`PATCH /api/v1/settings`），Mod 只「报出自己有一个可选引擎」 |
| 「端点权威永远只有 `[tts]` / `[llm]`」 | `director-rfc.md` §7.2 等多处 | 与本次澄清**一致**：选项列表只是 UI，选中后仍由 core 写 `[tts]` |
| `mod_count_is_five` 等注册数断言、`default_mods_manifest` | `crates/live2d-ai-desktop/src/main.rs`、`web_api/cli_entry.rs` | 新增一个 Mod 要动断言与文档（含 `mod-product-chain.md` 勾选表） |

**要让「本地 TTS Mod」成为真源里的合法形状，需要维护者新裁决 + 一次小修订**（本文只记录，不代改）：

1. 在 `tts-is-core.md` 增加一节：「引擎启动器 / 选项提供者可以由 Mod 承担；写 `[tts]` 的动作仍在 core」；
2. 明确 Mod 能暴露什么（进程状态 / 启停命令 / 可选引擎声明），并重申它**不得**写 `[tts]`；
3. 更新 `mod-product-chain.md` 的注册表与勾选表、`mod_count_*` 断言；
4. 若你要的是「能勾选停用」的语义，还要写明**停用 Mod ≠ TTS 失效**（`[tts]` 指向的端点仍被使用，只是没被本 Mod 拉起）。

---

## 4. 记录下来的实施形状（本轮不实施，供以后照做）

~~~text
┌─ Mod: local-tts（新增，缺省停用）────────────────┐
│ · 拉起 / 停止仓库外的 CosyVoice2 shim 进程        │
│ · GET /api/v1/mods/local-tts/state → 运行中/未起  │
│ · 声明一个可选引擎：                              │
│     { id: "local-cosyvoice2",                    │
│       label: "本地 TTS",                          │
│       base_url: "http://127.0.0.1:8080/v1",      │
│       sample_rate: 24000, channels: 1 }          │
│ · 不写 [tts]，不读密钥，不碰口型 / 舞台           │
└──────────────────────────────────────────────────┘
             │ 只「报出选项」
             ▼
┌─ Core: 设置 → 语音合成 ──────────────────────────┐
│ 引擎选择：[本地 TTS] / [云端 OpenAI 兼容] / 自定义 │
│ 选中后由 core 写 live2d-ai.toml 的 [tts] 段       │
│ （仍走 PATCH /api/v1/settings，唯一权威不变）      │
└──────────────────────────────────────────────────┘
~~~

- **本地档**：CosyVoice2-0.5B（4.52 GB，Apache-2.0；预估峰值 2.5–3 GB，tokenizer 挪 CPU 后约 2–2.5 GB，**待实测**）；shim 已支持 `CosyVoice2` 类，音色可用现有 `voices.yaml` + 同一段参考音频。
- **云端档**：任何 OpenAI 兼容 `/audio/speech` 端点（千问当年太贵 → 复活前先算账）。
- 两档的**契约完全相同**（`pcm` s16le / 24 kHz / 单声道 / 一句一单元），所以「多一个选项」在 Rust / Flutter 主链上是**零改动**的。

---

## 5. 待裁决（本轮未决）

1. 是否落 §3 的修订，让「本地 TTS Mod」成为真源里的合法形状？
2. Mod 的边界到哪：只「拉起 + 报选项」，还是允许它自带一个启停按钮（UI 归属）？
3. 权重来源：首次启动自动下 4.52 GB / 用户手动下 / 先只做 v3 的 tokenizer→CPU 止血（不降质量，峰值 5.37 → 约 4.5 GB）？
4. 是否接受 CosyVoice2 相对 CosyVoice3 的质量降级（上游自述 v3 在内容一致性、音色相似度、韵律上更好）——必须先做同一参考音频的 A/B 盲听。

---

## 6. 本轮动作声明

- **未实施**：没有下载权重、没有改 shim / 脚本 / `live2d-ai.toml` / `mods.json` / `.env` / 任何 Rust 或 Dart 代码，没有新增 Mod。
- **为取数重启过两个服务**：`127.0.0.1:8080`（`CosyVoice 3.0/scripts/3-start.sh`）与 `127.0.0.1:18080`（`scripts/ignite.sh`）；要停：`pkill -f shim/server.py`、`pkill -f live2d-ai-desktop`。
- **落盘**：本文 + 配套实测文档 [tts-vram-selection-2026-10-08.md](tts-vram-selection-2026-10-08.md)。

---

## 7. 变更历史

- 2026-10-08：首版。记录本轮 TTS 显存 / 选型问答与维护者关于「本地 TTS Mod」的口径澄清；只记录，不实施。
