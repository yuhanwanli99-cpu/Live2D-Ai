# 0.3.13 文件夹在「进入」时启动

> 状态：**已落地**（2026-10-21，本地 tag `v0.3.13-rc.1` = `747a1aa`（子进程）、`v0.3.13-rc.2` = `225fd81`（启动器），rc.3 只清理（tag `v0.3.13-rc.3` 在收口提交），未 push）。不并进已落地的 0.3.12，也不并进未开始的 `0.4.0` / `0.4.1`。
> `Cargo.toml` 保持 `0.3.1`。写法 `0.3.13-rc.z`：执行 agent 按代码拆 `z`；每个 rc 一次本地提交 + 一次本地附注 tag，**不 push**。最后一个 `rc.(z+1)` 只清理，活文档放在这一笔。
> 不改 `.env`、仓库根 `mods.json`、`live2d-ai.toml`。不提交 `official_live2d_samples.7z`。不点火 18080。测试只用临时目录。

用户把作者打好的文件夹放进 `mods/<目录名>/`。点「进入」且该行勾选时，启动清单里的那个程序。扫描和「刷新」仍不执行。这不是把那 7 个 Rust 库载进主程序。

## 行为

- `kind = process` 且勾选：启动已校验的 `entry`。
- `kind = launcher` 且 `launch.mode = with_app` 且勾选：启动 `launch.program`。
- `on_demand`：只记勾选，不启动。
- 未勾选：若**本进程**上次为这个 id 起过子进程，结束它。不要结束用户另外启动的同名程序。
- 「预启动」不参与文件夹。它仍只影响工厂里的 `local-tts-melo` / `local-engine`。
- 已发现 / 不可用、保留 id、工厂 0、`GET /api/v1/mods` 只列工厂：都保持 0.3.12。
- 启动失败不挡住 `/app/`。勾选已经写盘成功时 POST 仍是 200，body 里带失败原因。

## 实现

新文件 `crates/live2d-ai-desktop/src/folder_spawn.rs`（连测试 ≤500 行）。`main.rs` 增加 `mod folder_spawn`。

- 工作目录 = 该 Mod 目录。`Command::new(规范化后的文件绝对路径)`。不要 shell，不要 `sh -c`，不要额外参数。清单不新增键。
- 不 `chmod`。文件不能执行就记失败。
- `env_clear` 之后只保留父进程已有的 `PATH`、`HOME`、`LANG`、`LC_ALL`。不传 `.env`，不传名字里带 `KEY` / `TOKEN` / `SECRET` 的变量。
- Linux 上对子进程设置 `PDEATHSIG=SIGTERM`。子进程单独进程组；结束时杀整组，只杀我们记下的 pid。
- `spawn()` 不等待退出。同一 id 的子进程还活着就不要再起一个；已经退出则下次进入可以再起。
- 子进程表是 `Arc<Mutex<…>>`，放在 `ServerContext` 的新字段。`web_api/mod.rs` 现 704 行，已是超 500 的 22 个文件之一：只加字段和空表，不把启动逻辑写进去，不拆文件。每个 `ServerContext {` 字面量补上该字段。
- `GET /api/v1/mods/discovered` 和「刷新」不启动。`ignite` 不预启动文件夹。
- `POST /api/v1/mods/discovered`：先 `persist_folder_selection`（失败则 409，且不启动）。成功后再启动或结束。响应在现有 `ok` / `changed` / `selectable` 上增加 `started`、`stopped`、`failed`。
- `mods_scan.rs` 现 459 行，必须保持 ≤500。`scan` 仍然不执行。可选行的 JSON 增加 `launch_mode`：`process` 写 `with_app`，`launcher` 写清单里的 mode，不可用行写 `null`。加字段若会超过 500，把 `discovered_json` 挪到 ≤500 的新文件，不要为此拆 `web_api/mod.rs`。
- `launcher.rs` 现 429 行，必须保持 ≤500。页面在 `assets/launcher.html`。`buildRows` 保留清单的 `kind` 与 `launch_mode`。**本次拉起** = 预启动开着时勾选的工厂拉起类 + 勾选的 `process` + 勾选的 `with_app`。`on_demand` 不计。
- 删掉页面上「文件夹只记下勾选 / 不执行」的句子，改成与上面的行为一致。新中文必须通过 `launcher.rs` 的 `every_launcher_html_char_is_covered_by_the_font_subset`。缺字就改措辞，不换字体，不引用 `gstatic`。
- 对话回合不等待这个进程。不做 `secret.get`、`cue`、通用 `settings.write`、JSON-RPC、SDK、`.so`、wasmtime。
- 不改 `mod-product-chain.md` §8 和 `mod-community-license.md` §2。本版是另起进程，不是把代码载入主进程。
- 不把七个 Mod 编回 `AVAILABLE_MOD_FACTORIES`。`mod_count_is_zero` 保持。不复制那七个仓库，不在仓库根创建 `mods/demo-mod`。
- 不放宽棘轮：`src>500` 仍 22，desktop deps 仍 18，Dart `lib>800` 仍 0，docs 预算仍 45000。不改 `AGENTS.md` 历史段、`CHANGELOG.md`、旧发布说明。

## 测试

`mods_scan_tests` 里「扫描绝不执行 entry / launch.program」保持绿色。

新增（临时目录，测完杀掉自己起的子进程）：勾选后 POST，标记文件出现；不勾选、GET、刷新都不出现；`on_demand` 不出现；子进程还活着时再进入不产生第二个；取消勾选再进入后该进程消失；父进程里的 `LIVE2D_AI_SENTINEL_SECRET` 不出现在子进程环境；绝对路径和 `..` 仍拒绝。

不碰仓库根的三份配置，不占用 18080。

提交前这三件必须绿：`cargo test -p live2d-ai-desktop --all-targets`、`cargo fmt --all -- --check`、`cargo clippy -p live2d-ai-desktop --all-targets -- -D warnings`。全 workspace 的 `cargo test` 不是本单阻断；没跑就在发布说明里写明。

## 文档（只在最后一个清理 rc）

落地时把本单和索引从「已派、未执行」改成「已落地」，写上 tag 与提交。`0.3.12` 的句子保持「只预检、不执行」，不要改成会启动。

改这些：`CURRENT.md` §2、`docs/mods-user.md` 的第三步、`README.zh-CN.md`、`README.md`、`two-repo-mods.md` §4、`DOC-MAP.md`、`docs/README.md`、`PROMPT-3.0-motion-first-2026-10-17.md`。发布说明 `docs/releases/v0.3.13-rc.N.md`。QQ 不写进 `README.zh-CN.md`。

`cargo run -q -p xtask -- code-stats --check --only docs` 必须通过，预算仍是 45000。派发后余量大约 15 行，清理 rc 以改现有句子为主，不要新增长文。
