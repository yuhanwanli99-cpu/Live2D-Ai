# 语音二选一、设置生效、调试归位、文档收口

> 立档：2026-10-09。状态：**活**。配套提示词 `docs/plans/PROMPT-tts-choice-and-closeout-2026-10-09.md`。
> 版本停在 `0.2.3-rc.1`。不打 tag。不重开已验收的设置信息架构（导航名、人设改名、三级介绍）。

## 不做

- 不把 `local-tts`（CosyVoice3）挂回注册表。不碰 `/home/skystar/CosyVoice 3.0`，不去拉 8080。
- Mod 仍然不许调用 `apply_settings` 去写 `[tts]`。不许重试、看门狗、云端兜底、静默成功。
- 不修 `docs/audit/2026-10-09-silent-errors/FINDINGS.md` 的 S-000…S-007。
- 不改 `.env`，不整份覆盖 `live2d-ai.toml` / `mods.json`。不提交 `此项目的调研结果.md`。
- 不删待机生命体征。不接回 `action_tx`。不升版本。

## 1. 文档里还矛盾的现行句

只改这些句子。历史段落、旧 curl、`scripts/ignite.sh` 不动。

| 位置 | 现在 | 改成 |
| --- | --- | --- |
| `README.md` 许可节、`README.zh-CN.md` 许可节 | 本仓库不再分发 `.moc3` 与贴图 | 出厂带白模型那几份已跟踪文件；条约见 `MODEL_LICENSE.md`，不是作者授权再分发；AGPL 不覆盖这些文件 |
| `LICENSE-SCOPE.md` §3 与 §5 里「模型原件不入库 / gitignore 除占位外」 | 与已跟踪的 `bai.moc3`、`texture_00_4096.png` 矛盾 | 代码许可不覆盖模型。已跟踪的是渲染必需的那几份，附带免责和 24 小时删除。`texture_00.png`、`bai.vtube.json`、`items_pinned_to_model.json` 仍不入库 |
| `README.zh-CN.md`「常用 CLI」 | 仍列 `--chat` / `--model-smoke` / `--benchmark` | 与英文 README 一样：这些旗标已删除，只留 `--web` 与 `--audio-smoke` |
| `README.md` Mod 节 wallpaper / pet-desktop | crate 还在 workspace | 与中文同节：2026-10-01 已物理删除，只在 tag `checkpoint/pre-d1-dormant` |
| `docs/local-inference-setup.md` 横幅末尾「另注」 | 「当前默认面向 CosyVoice 3」 | 删掉或改成：出厂本地出声是 MeloTTS。下面 TL;DR 的旧 curl 保持原样 |

## 2. 语音：云端或本地，本地只跑一个

出厂代码和 `live2d-ai.toml.example` 已经是 Melo：`http://127.0.0.1:8091/v1`、音色 `ZH`、`pcm`、44100、单声道、无密钥。本机 `live2d-ai.toml` 仍是 `http://127.0.0.1:8080/v1`、`skystar`、24000，而 8080 没有进程。所以现在合成失败。界面提示还写着 `http://127.0.0.1:8080/v1` 和 `alloy`。

在「模型服务 → 语音合成」加一个二选一，文案只用「云端」「本地」。

- **本地（缺省）**：地址固定 `127.0.0.1:8091`，普通层不给改端口。音色 `ZH`，格式 `pcm`，采样率 44100，声道 1。这些由设置保存写进 `[tts]`（核心链路），不是 Mod 去改。同时只启用 `local-tts-melo` 这一个本地引擎。已有子进程就不要再起一个。CosyVoice 保持封存。
- **云端**：停掉 `local-tts-melo`。地址、音色、模型名、密钥交给用户填，请求仍是 OpenAI 兼容 `/audio/speech`。不预填厂商，不带 Key。连通性自检仍只打 `/models`，不合成。
- 切换走同一次「保存并应用」。失败就让这一轮出声失败，错误码照旧，不要自动改回另一个。

`AudioSpec::DEFAULT_SAMPLE_RATE`（24000）是没配音频时的调度常量。本地出声以 `[tts].sample_rate` 44100 为准，WS 帧带这个数。状态里如果还把 24000 显示成「当前语音采样率」，改成读 `[tts]` 的值。不要把两个常量合成一个。

本机只改 `[tts]` 的出声字段到本地这组值，并在 `mods.json` 里启用 `local-tts-melo`（`with_app`）。未知 id `local-tts` 留在文件里，不要启用它，也不要因为保存把它删掉。LLM 段和 `.env` 不动。

## 3. 设置要保存、生效，并刷新前端

现在按保存只写盘，再按 `apply_status` 弹一条「已保存」或「需重启」。页面不刷新。用户要的是一步：保存、让后端用上新值、前端重新加载。

- 热重载已经能用的字段：保存成功后重新加载 `/app/`。
- 返回「需重启」的字段：重启的只是 18080 上的 `live2d-ai-desktop`，起来后再加载页面。不要停别的端口。
- 文案不要再停在「请自行重新点火」。失败仍显示原来的 `code` 和 `message`。

本地显示偏好（待机、档位、主题）不走这个 toml 保存。它们本来就该随手生效。

## 4. 启用 Mod 时界面卡一下

`enable` 在 HTTP 处理里同步跑 `start_one`。Melo 的 `start` 还要等立刻退出观察窗，第一次还可能装依赖。`tiny_http` 这条接受循环是单线程的，这段时间整个页面都不应答。

启用和保存的请求要先回到界面。子进程仍在原来的失败口径里：立刻退出就是 Failed，错误里带脚本输出。不要把「还在启动」写成已成功，也不要另做看门狗。

## 5. 待机小动作和渲染档位

待机开关在 `MotionSection`，渲染面 `idle_enabled == false` 时会跳过呼吸眨眼。先在 `http://127.0.0.1:18080/app/` 上实际拨一次。拨动后帧没进渲染面，或进了仍在呼吸，就修那条链路。不要删 `IdleState`。

渲染档位只在开发者模式里出现，而且现在是画布最长边的上限。普通窗口远小于 4096，4K / 8K / 16K 画出来一样，所以看起来失效。让三档在这台屏幕上也能看出差别：按档位改变实际绘制分辨率，来回切得到、刷新后还是所选的那一档。不要为了字面 16384 去分配一张 1GB 的纹理。改之前读 `docs/architecture/renderer-texture-tier-adr.md`。动了 wasm 就要重建渲染面产物。

## 6. 表情调试和动作调试

这两块还在核心「开发模式」页（`dev_tools_developer.dart` 的 `DebugPanels`）。导演可观测已经在导演卡片上。

把表情调试、动作调试，以及它们附带的临时幅度，挪到「扩展 → 导演」卡片里，仍只在开发者模式显示。核心开发模式页只留开发者开关和诊断。能力留下，不删。不接 `action_tx`。面板文件超过 500 行就按现有 `part` 拆，不要复制一份逻辑。

## 7. 做完之后

仓库根按标准命令重建并拉起（18080 上已有进程会被 `ignite.sh --build` 换掉，这是这一轮要的）：

```bash
(cd shell/flutter && flutter build web --release --base-href /app/ --no-web-resources-cdn)
./scripts/prune_web_artifacts.sh
./scripts/prune_web_artifacts.sh --check
./scripts/ignite.sh --build
```

另开终端 `./scripts/ignite.sh --check`。浏览器打开 `http://127.0.0.1:18080/app/`，亲手点：云端/本地切换、待机开关、开发者模式下的档位、导演卡片上的表情和动作调试。本地模式要确认 8091 在听、请求打到它；云端模式要确认 Melo 进程已停。

新界面文案用已有汉字。`flutter test` 里的字体子集门禁变红就改文案，不要重建 `assets/fonts/*.ranges.txt`。

门禁按动到的层跑：Rust 改动跑 fmt、clippy、相关测试和 `rust-ratio`；Flutter 改动跑 `flutter analyze` 与 `flutter test`。文档改动跑 `cargo run -p xtask -- code-stats --check --only docs`。

一次提交，快进推 `origin main`。不要 `git add -A`。不要打 tag。认证失败就停并贴远端原话。
