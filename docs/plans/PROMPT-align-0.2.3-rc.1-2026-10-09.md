# 对齐 0.2.3-rc.1 并重新编译 · 交给下一个 agent

> 立档：2026-10-09。状态：**活**。范围真源是 `docs/plans/PLAN-align-0.2.3-rc.1-2026-10-09.md`。
> 不升版本，不打 tag，不改 Rust / Dart 行为，不改 `scripts/ignite.sh`。
> 不改本机 `mods.json`、`.env`、`live2d-ai.toml`。不要停 `127.0.0.1:8080`。不要动 `/home/skystar/CosyVoice 3.0`。
> 不要执行 `cp live2d-ai.toml.example live2d-ai.toml`。

18080 上的旧 Live2D 进程已经停掉。按计划书「标准构建 + 启动」拉新进程。`ignite.sh --build` 在前端产物已存在时不会重编 Flutter，所以第一行必须先跑。它会 `exec` 进服务、不退出；后台启动，另开终端跑 `--check`。

入口文档要标上同一条命令：`README.md`、`README.zh-CN.md` 的快速开始与点火小节，以及 `AGENTS.md` 约 247 行。文档里写 `flutter`（SDK 在 `~/flutter`），不要写这台机器的绝对路径。快速开始里的 `cargo build --release` + 直接跑二进制 + `http://localhost:18080/` 换成这条。浏览器只写 `http://127.0.0.1:18080/app/`。

本机 toml 仍指向 8080。`GET /api/v1/settings` 看到 8080 是预期。

文档一次提交，快进推 `origin main`。不要 `git add -A`。认证失败就停并贴远端原话。

## 本机实跑

```bash
cd /home/skystar/Live2D-Ai
(cd shell/flutter && /home/skystar/flutter/bin/flutter build web --release --base-href /app/ --no-web-resources-cdn)
./scripts/prune_web_artifacts.sh
./scripts/prune_web_artifacts.sh --check
./scripts/ignite.sh --build
```

最后一行不退出。起来之后：

```bash
./scripts/ignite.sh --check
```

## 提示词（整段复制给下一个 agent）

```text
你在 /home/skystar/Live2D-Ai，分支 main。先读 docs/plans/PLAN-align-0.2.3-rc.1-2026-10-09.md 和 docs/plans/PROMPT-align-0.2.3-rc.1-2026-10-09.md，按它们做。不升版本，不打 tag，不改 Rust/Dart 行为，不改 scripts/ignite.sh。不改本机 mods.json、.env、live2d-ai.toml。不要执行 cp live2d-ai.toml.example。不要停 127.0.0.1:8080，不要动 /home/skystar/CosyVoice 3.0。

只改计划书表格里的现行句。历史段落和旧 curl 不改。

README.md、README.zh-CN.md 的快速开始和点火小节，以及 AGENTS.md 约 247 行，标上这一条标准命令（文档里写 flutter，SDK 在 ~/flutter）：

(cd shell/flutter && flutter build web --release --base-href /app/ --no-web-resources-cdn)
./scripts/prune_web_artifacts.sh
./scripts/prune_web_artifacts.sh --check
./scripts/ignite.sh --build

浏览器只写 http://127.0.0.1:18080/app/ 。写明 ignite.sh --build 在 shell/flutter/build/web/index.html 已存在时不会重编 Flutter。删掉快速开始里 cargo build --release 后直接跑二进制、以及打开 http://localhost:18080/ 的那段。

18080 上的旧进程已经停掉。用下面的绝对路径实跑，最后一行后台启动（它会 exec 进服务、不退出），然后另开 ./scripts/ignite.sh --check：

cd /home/skystar/Live2D-Ai
(cd shell/flutter && /home/skystar/flutter/bin/flutter build web --release --base-href /app/ --no-web-resources-cdn)
./scripts/prune_web_artifacts.sh
./scripts/prune_web_artifacts.sh --check
./scripts/ignite.sh --build

本机设置仍显示 8080 是预期。8080 若已不在听，只记一笔，不要去拉起 CosyVoice。

文档一次提交，快进推 origin main。不要 git add -A。认证失败就停并贴原话。贴出 prune --check、ignite --check、8080 是否仍在听、git status 和 push 的末段。
```
