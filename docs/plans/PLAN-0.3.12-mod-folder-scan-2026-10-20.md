# 执行单：0.3.12 文件夹清单预检（2026-10-20）

> 交给执行 agent。这一块就是 **`0.3.12`**。不要并进 `0.3.11`、`0.4.0`、`0.4.1`。
> 写法 `0.3.12-rc.z`：`z` 按代码自拆；每步一次本地提交加一个本地附注 tag，不 push。
> 最后一个 `rc.(z+1)` 只清理测试、文档和棘轮，不加功能。`Cargo.toml` 仍是 `0.3.1`。
> 不改 `.env`、仓库根 `mods.json`、`live2d-ai.toml`。不提交 `official_live2d_samples.7z`。
> 不把 `/home/skystar/live2d-bili-sidecar/` 加进任何仓库。
> 不改 [`mod-product-chain.md`](../architecture/mod-product-chain.md) §8，也不改
> [`mod-community-license.md`](../architecture/mod-community-license.md) §2。
> 这两处仍写「静态链接、不做动态 `.so`」。本版不加载代码，所以不去改写它们。

## 0. 做完是什么样

工作目录下若有 `mods/<目录名>/mod.toml`，启动器在现有这一页把它列出来。清单只读、只校验。**不执行** `entry`，**不 spawn** `program`，不把文件夹注册进 `AVAILABLE_MOD_FACTORIES`。

缺省工厂仍是 0。没有 `mods/` 目录时，页面和现在一样能进 `/app/`。七个公开仓库仍是源码，本版不下载、不编译、不把它们拷进主仓。

## 1. 现在的事实

`GET /api/v1/mods` 只列已经编进二进制的工厂。缺省是 `{"mods":[]}`。`mods.json` 只覆写在册 id，文件里不认识的 id 原样保留（`mod_registry.rs` 的 `raw_manifest`）。`MOD_API_VERSION` 是 1。

0.3.11 的启动器三个数字是：已注册、本次确认、本次拉起。预启动缺省关。点「进入」只对已注册 id 调用现有 enable / disable，然后探测当前这一份 `[llm].base_url` 与 `[tts].base_url`，不改档。这些行为对**工厂 id** 保持不变。

导演在核心里。`local-llm`、`wallpaper`、`pet-desktop` 已删除。CosyVoice 的 id 是 `local-tts`，不要挂回。允许的本机语音 id 仍是 `local-tts-melo`。

## 2. 禁止

- 不把七个 Mod 编回缺省二进制。`mod_count_is_zero` 保持。不建 `.so`、WASM 运行时、下载市场、JSON-RPC、`live2d-ai-mod-sdk`。
- 不执行清单里的路径，不用它做 `+x` 探测以外的任何启动。本版连可执行位都不要求。
- 不新增 `secret.get`、`cue`、`settings.write`、同轮等待 Mod 应答。`action_tx` 仍休眠。
- 扫描失败、单份清单无效，都不阻止进程启动，也不阻止进入 `/app/`。
- 不跟随符号链接。`entry` / `program` 必须是该 Mod 目录内的相对路径，拒绝绝对路径和 `..`。
- 不读、不写 `[permissions]`。出现未知键就判这份清单不可用。
- 不提高文档预算（45000），不放宽 `src >500` 的文件数（22）和 desktop deps（18）。新 Rust 文件各自 ≤500，不能变成第 23 个超 500 的文件。扫描逻辑放在新文件里。`launcher.rs` 现在 396 行，改完仍 ≤500。`web_api/mod.rs` 已在那 22 个里（704 行），本版只改路由登记，不要把扫描搬进去，也不要为了压行数去拆它。
- 不改 `AGENTS.md` 历史段、`CHANGELOG.md`、已发布的 `docs/releases/**`。
- 单测只用临时目录和临时 `mods.json`。不碰仓库根那三份本机配置，不复制 Melo 权重。

## 3. 扫描

新模块放在 `crates/live2d-ai-desktop/src/mods_scan.rs`（测试可另文件）。扫描根是进程当前目录下的 `mods/`。目录不存在 = 已发现 0，不创建它。只看下一层 `mods/<目录名>/mod.toml`。更深的目录不扫。以 `.` 开头的目录跳过。`mod.toml` 所在路径或它本身是符号链接 = 不可用，原因写明。

`.gitignore` 增加一行 `/mods/`，让用户丢进来的文件夹不被提交。不要动已有的 `/mods.json` 规则。仓库里不要提交任何 `mods/<id>/` 样例；测试夹具放在测试临时目录。

`mod.toml` 用 `schema = 1`，未知键即不可用。字段：

| 键 | 要求 |
|---|---|
| `schema` | 整数 `1` |
| `id` | `^[a-z][a-z0-9-]{0,63}$`，且等于目录名 |
| `name` | 非空字符串 |
| `version` | 非空字符串 |
| `api_version` | 必须等于 `MOD_API_VERSION`（现为 1） |
| `kind` | 只允许 `process` 或 `launcher` |
| `description` | 可省略 |
| `entry` | `kind = process` 时必填 |
| `launch.program` | `kind = launcher` 时必填 |
| `launch.mode` | `with_app` 或 `on_demand` |

`entry` 与 `launch.program` 只做词法检查和「规范化之后仍在该 Mod 目录内的普通文件」。文件可以还不能执行。不要打开它当程序跑。

下列 id 即使清单别的字段都对，也不可用：`director`、`local-llm`、`wallpaper`、`pet-desktop`、`local-tts`。原因写「保留，不能当文件夹装入」。与 `AVAILABLE_MOD_FACTORIES` 里已有 id 冲突的文件夹同样不可用。缺省工厂为空，这条靠夹具测。

同一 id 出现两次：路径排序后的第一份可进入已发现，其余不可用，原因「重复」。

## 4. 启动器上怎么显示

仍是现有 `GET /` 这一页，不新做 Flutter 页，不进设置里的扩展卡片。

新增只读 `GET /api/v1/mods/discovered`。不要把文件夹行塞进现有 `GET /api/v1/mods`（那个列表仍只代表工厂，Flutter 设置继续只看见已注册的运行时）。响应形如：

```json
{"discovered":[{"id":"","name":"","version":"","api_version":1,"kind":"process","selectable":true,"reason":null}]}
```

`selectable = false` 时 `reason` 是一句短原因。不可用的行要出现在这个列表里。

页面三个数字改成：

- **已发现**：可选行的条数。可选行 = 校验通过的文件夹，加上已编进二进制的工厂。无效文件夹不计入，但列在下面。只读。没有数字输入框。
- **本次确认**：勾选的可选行条数。无效行不能勾。
- **本次拉起**：只统计已编进二进制、且 id 为 `local-tts-melo` 或 `local-engine`、预启动开着、并且被勾选的条数。文件夹里的 `process` / `launcher` **本版不计入**。缺省二进制下这个数字是 0。

初始勾选：该 id 在 `mods.json` 里 `enabled = true`，并且这一行可选。没有这条 id = 不勾。文件里那些不在可选列表中的旧 id 不显示、不改、不删。

「刷新」重新扫描。不执行，不写盘。消失的 id 丢掉页面临时勾选。

预启动开关仍在，缺省关。拨动它不发请求。文件夹行不会因为拨到开而让「本次拉起」增加。

点「进入」：

- 工厂 id 仍走 0.3.11 的 enable / disable。
- 可选文件夹 id 只把 `enabled` 写进 `mods.json`（沿用「只覆写这些 id、其余键原样保留」）。不调用 enable / disable，不 spawn。
- 然后仍是原来的加载动画和当前地址探测，再进 `/app/`。探测规则不变：不改档、不写 toml。

字体继续用现有子集校验。新汉字必须落在 `shell/flutter/assets/fonts/*.ranges.txt`。页面不得引用 gstatic，不得出现地址、密钥、厂商输入框。

## 5. 文档（行为落地的同一提交）

- `CURRENT.md` §2 的「0.3.12 已派、未执行」改成已落地，并写上最后一个 rc 的提交哈希。未做完不要改成已落地。
- `CURRENT.md` §7：三个数字按第 4 节。写明文件夹不执行。缺省工厂 0、不挂 Melo、探测不改档，这三句保留。
- `two-repo-mods.md` §4 补一句：扫到文件夹不等于挂载，挂载仍是下面五步重新编译。
- `README.zh-CN.md` 与 `README.md` 各补一句：本版只预检清单。QQ 仍不要写进 `README.zh-CN.md`。
- 本文件文首标已落地并写上提交哈希。未做完不要标。

## 6. 验收

- 工厂仍是 0。没有 `mods/` 时，已发现 0、本次确认 0、本次拉起 0，进入 `/app/` 仍成功。
- 临时目录里放一份合法 `process` 清单和一个普通文件当 `entry`：行可选，进程表里没有把它拉起。`launcher` 的 `program` 同样不拉起。
- `..`、绝对路径、符号链接、`api_version = 99`、未知键 `[permissions]`、保留 id、目录名与 id 不符：不可选，且带原因。
- 「进入」之后，临时 `mods.json` 里该 id 的 `enabled` 与勾选一致，原先不认识的 id 还在。仓库根 `mods.json` 的内容与扫描前一致。
- `bash -n scripts/ignite.sh` 不必改点火等待。`mod_count_is_zero` 仍过。未改 `.env`、`live2d-ai.toml`。未把权重或 `mods/` 样例加进 git。

## 7. 交付时写明

每个 rc 的提交哈希与本地 tag。是否有一次把 `entry` 或 `program` 当成进程启动。`GET /api/v1/mods` 是否仍为空列表。工厂数仍为 0。`0.4.0` 仍未开始。
