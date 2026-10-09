# 0.2.3-rc.1 对齐、重编译、收尾

> 立档：2026-10-09。状态：**活**。配套提示词 `docs/plans/PROMPT-align-0.2.3-rc.1-2026-10-09.md`。
> 版本停在 `0.2.3-rc.1`。不打新 tag。不改 Rust / Dart 行为，不改 `scripts/ignite.sh`。

## 为什么还有一轮

`origin/main` 与 tag `v0.2.3-rc.1` 已是 `5af352c`。代码、`tts-is-core.md` §7、发布说明是对的。下面这些**现行句**还停在上一态。两份 README 的「快速开始」仍教 `cargo build --release` 后直接跑二进制，并打开 `http://localhost:18080/`。那不是入口。

18080 上的旧 Live2D 进程**已经停掉**。这一轮按下面的标准命令拉**新**进程。不要去找旧 pid，也不要为了「凑出厂缺省」改本机 `mods.json`、`.env`、`live2d-ai.toml`。不要停 `127.0.0.1:8080`，不要动 `/home/skystar/CosyVoice 3.0`。`ignite.sh` 里的 `pkill` 只匹配 `live2d-ai-desktop --web`。

## 现行句

| 位置 | 现在写的 | 改成 |
| --- | --- | --- |
| `AGENTS.md` 约 169–174 行 | 注册含 `local-tts`；缺省只启用 `external-input` | 注册 6 个：`external-input` / `persona` / `voice-input` / `memory` / `director` / `local-tts-melo`。缺省启用 `external-input` 和 `local-tts-melo`。`local-tts` 封存、未注册 |
| `AGENTS.md` 约 27–30 行 | `assets/models/` 不捆绑二进制；括号里现行版本 `0.2.2` | 出厂带白模型（条约见 `assets/models/bai/MODEL_LICENSE.md`，不是作者授权再分发）。括号里的现行版本改成 `0.2.3-rc.1` |
| `AGENTS.md` 约 247 行点火纪律 | 只分开写 `ignite.sh` 和 flutter 构建旗标 | 补上与下面「标准命令」相同的一条。写明：`ignite.sh --build` 在 `shell/flutter/build/web/index.html` 已存在时**不会**重编 Flutter |
| `README.md` / `README.zh-CN.md` 首屏与 Mod 节 | 不捆绑模型；5 个注册；缺省只启用 `external-input` | 与注册表那一行同一口径。英文首屏 “does not ship any model” 一起改 |
| 两份 README 的「快速开始」代码块，以及紧接着的「模型二进制不会进入 git」 | `cargo build --release` + 直接跑二进制 + `http://localhost:18080/` | 换成下面的标准命令。浏览器只写 `http://127.0.0.1:18080/app/`。模型那句改成：出厂带白模型，条约见 `MODEL_LICENSE.md` |
| 两份 README 的点火小节 | `--build` 的注释写成「先重建 Flutter Web」 | 注释改成与标准命令一致：`--build` 编的是调试版后端并启动；前端要靠标准命令的第一行 |
| `docs/local-inference-setup.md` 文首 2026-10-09 那段 | `local-tts` 缺省停用，出声例子仍是 8080 / `skystar` / 24000 | 只改这段横幅：出厂是 MeloTTS `8091` / `ZH` / `pcm` / `44100`；CosyVoice3 封存。下面的旧 curl 不动 |
| `docs/architecture/cosyvoice3-tts-integration.md` 文首 | 「接入能力已就绪」 | 加一行：引擎已封存，出厂出声不走这里。正文 curl 不动 |
| `tts-is-core.md` §7 | 没写 stderr 管道 | 加一行现行句：立刻退出要带上脚本原因，所以 stderr 改为管道并透传。§6 那句「禁止管道」是上一态，不改 |
| `assets/models/README.md` 文首「不入库」和「仓库内不捆绑」 | 叫人自行放置、并说 git 不跟踪模型 | 出厂已跟踪白模型的 `MODEL_LICENSE.md`、`runtime/bai.model3.json`、`bai.moc3`、`bai.cdi3.json`、`bai.physics3.json`、`runtime/bai.16384/texture_00_4096.png`。条约不是作者授权再分发。`texture_00.png`、`bai.vtube.json`、`items_pinned_to_model.json` 仍不入库。历史来源那节不改 |

不改：`AGENTS.md` 变更历史、`tts-is-core.md` §1–§6、发布说明里的过程记录、`docs/verification/**`、`docs/README.md` 里「含本仓 0.2.2」那种研究索引、`scripts/ignite.sh`。

## 标准构建 + 启动

产物不入库。在仓库根执行。这一条同时是入口文档要标注的命令，也是本轮要跑的命令。

```bash
(cd shell/flutter && flutter build web --release --base-href /app/ --no-web-resources-cdn)
./scripts/prune_web_artifacts.sh
./scripts/prune_web_artifacts.sh --check
./scripts/ignite.sh --build
```

`flutter` 在本机是 `/home/skystar/flutter/bin/flutter`。入口文档写 `flutter`，并写一句 SDK 在 `~/flutter`（与 `ignite.sh` 把 `$HOME/flutter/bin` 放进 `PATH` 相同）。提示词里的实跑命令用绝对路径。

`./scripts/ignite.sh --build` 会 `exec` 进服务，不会自己退出。18080 旧进程已停，直接拉新的。启动后另开终端跑 `./scripts/ignite.sh --check`。浏览器打开 `http://127.0.0.1:18080/app/`。

不要执行 `cp live2d-ai.toml.example live2d-ai.toml`。本机 toml 仍指向旧的 8080 垫片，重启后的服务读的是这份文件。`GET /api/v1/settings` 里看到 8080 是预期。

## 提交

只有文档。一次提交，说明是对齐 `v0.2.3-rc.1` 的现行句，并标上标准启动命令。快进推 `origin main`。不打 tag，不升版本。认证失败就停。不要 `git add -A`。根目录 `此项目的调研结果.md` 仍不提交。

验收贴最后约 30 行：`prune --check`、`ignite.sh --check`、8080 仍在听（若已不在听，只记一笔，不要去拉起 CosyVoice）、`git status`、push 结果。没跑的不要写成绿。
