# 语音二选一与收口 · 交给下一个 agent

> 立档：2026-10-09。状态：**活**。范围真源是 `docs/plans/PLAN-tts-choice-and-closeout-2026-10-09.md`。
> 不升版本，不打 tag。

先读计划书，再按下面做。本机 18080 上已有 `live2d-ai-desktop`。8080 没有人听。语音失败是因为本机 `[tts]` 还指向 `http://127.0.0.1:8080/v1`。

## 提示词（整段复制给下一个 agent）

```text
你在 /home/skystar/Live2D-Ai，分支 main。先读 docs/plans/PLAN-tts-choice-and-closeout-2026-10-09.md，按它做。不升版本，不打 tag。不重开设置信息架构。不挂回 local-tts。不碰 /home/skystar/CosyVoice 3.0，不去拉 8080。不改 .env。不整份覆盖 live2d-ai.toml 或 mods.json。Mod 不许 apply_settings 写 [tts]。不做重试、看门狗、云端兜底。不修 S-000…S-007。不提交 此项目的调研结果.md。

1. 只改计划书第 1 节那几句现行文档。历史段落和旧 curl 不动。

2. 「模型服务 → 语音合成」加「云端 / 本地」二选一，缺省本地。
   本地：端口固定 127.0.0.1:8091，普通层不能改端口。设置保存把 [tts] 写成 voice ZH、pcm、44100、声道 1、无密钥。只启用 local-tts-melo，已有子进程不要再起一个。
   云端：停掉 local-tts-melo。地址、音色、模型名、密钥由用户填。不预填厂商，不带 Key。自检仍只打 /models。
   切换失败不要自动改回另一个。界面上那句 8080 / alloy 提示换成这套口径。
   AudioSpec 的 24000 常量留着。本地出声用 [tts] 的 44100。状态若把 24000 显示成当前语音采样率，改成读 [tts]。

3. 本机只把 [tts] 出声字段改成本地这组值，并在 mods.json 启用 local-tts-melo（with_app）。未知 id local-tts 留着且不要启用。LLM 段不动。

4. 服务端设置的保存做成一步：写盘、能热重载就热重载、必须重启就只重启 18080 的 live2d-ai-desktop，然后刷新 /app/。不要再只弹「请自行重新点火」。失败仍带原来的 code 和 message。待机和档位不走这条 toml 保存。

5. 启用 Mod 不要堵死 tiny_http 的接受循环。立刻退出仍是 Failed，错误里带脚本输出。不要把「还在启动」写成成功。

6. 在 http://127.0.0.1:18080/app/ 上拨「待机小动作」。帧没到渲染面，或到了仍在呼吸眨眼，就修链路。不要删 IdleState。
   渲染档位现在只是画布上限，普通窗口三档看起来一样。让三档在这台屏幕上能看出差别，来回切得到，刷新后还在。不要分配 16384 的整张纹理。先读 docs/architecture/renderer-texture-tier-adr.md。改了 wasm 就重建渲染面。

7. 把表情调试、动作调试和它们的临时幅度从核心开发模式页挪到扩展里的导演卡片，仍只在开发者模式显示。核心开发模式页只留开关和诊断。不接 action_tx。

做完用标准命令重建并拉起（仓库根；flutter 用 /home/skystar/flutter/bin/flutter）：

(cd shell/flutter && /home/skystar/flutter/bin/flutter build web --release --base-href /app/ --no-web-resources-cdn)
./scripts/prune_web_artifacts.sh
./scripts/prune_web_artifacts.sh --check
./scripts/ignite.sh --build

另开 ./scripts/ignite.sh --check。浏览器亲手点云端/本地、待机、档位、导演卡片上的表情和动作调试。本地要看到 8091 在听。云端要看到 Melo 进程已停。

新文案用已有汉字，字体子集门禁变红就改字，不要重建 ranges.txt。
动到的 Rust 跑 fmt、clippy、相关测试和 rust-ratio。Flutter 跑 analyze 和 test。文档跑 code-stats --check --only docs。

一次提交，快进推 origin main。不要 git add -A。认证失败就停并贴原话。贴出检查、浏览器点过的结果、git status 和 push 末段。没跑的不要写成绿。
```
