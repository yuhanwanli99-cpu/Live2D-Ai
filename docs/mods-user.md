# Mod 怎么放（用户说明）

> 2026-10-21。对应已落地的 **0.3.12**。这页只讲使用。行为以
> [`CURRENT.md`](architecture/CURRENT.md) 和源码为准。缺省程序里**没有**编进任何一个 Mod。

对话本身不需要 Mod。填好 `[llm]` 和 `[tts]` 就能用。下面只在你想让启动器看见一个文件夹时才做。

## 固定位置

服务的当前目录必须是仓库根（`./scripts/ignite.sh` 会自己切过去）。只认这一层：

```text
<仓库根>/mods/<目录名>/mod.toml
```

文件夹名是 `mods`（有 s）。没有这个目录也正常：启动器「已发现」为 0，照样能进 `/app/`。程序不会替你创建它。

Windows 里的 `\\wsl.localhost\Ubuntu\home\skystar\Live2D-Ai` 和 WSL 的 `/home/skystar/Live2D-Ai` 是同一份文件。`mod.toml` 存成 **UTF-8、无 BOM**。换行可以是 CRLF。

## 打开启动器之后

浏览器打开 `http://127.0.0.1:18080/`。

| 页面上的字 | 含义 |
| --- | --- |
| 已发现 | 清单合格、可以勾的行数 |
| 不可用 | 有清单但不合格。复选框禁用，旁边写原因，不计入已发现 |
| 本次确认 | 你勾上的行数 |
| 本次拉起 | 本版对文件夹永远是 0 |

- **刷新**：再看一遍磁盘，不运行文件，不写盘。
- **进入**：合格文件夹只把勾选写进 `mods.json`（`"enabled": true` 或 `false`）。`mod.toml` 里的 `entry` 不会被执行。
- 设置 → 扩展仍只显示**已经编进这次程序**的 Mod。缺省是空的。文件夹不会变成那里的卡片。

`mods.json` 里本来就有、但这份清单里没有的 id，原样保留，不删除，也不当成可勾的行。

## 一份最小例子

```bash
mkdir -p mods/demo-mod
cat > mods/demo-mod/mod.toml <<'EOF'
schema = 1
id = "demo-mod"
name = "演示"
version = "0.1.0"
api_version = 1
kind = "process"
entry = "run.sh"
EOF
printf '#!/bin/sh\n' > mods/demo-mod/run.sh
```

刷新后「已发现」为 1，这一行可以勾。进入后 `mods.json` 里有 `demo-mod`。`run.sh` 不会跑。不要求它有可执行位。

`id` 必须和目录名相同，并且是小写字母开头、只含小写字母、数字、连字符，最长 64。`api_version` 必须是 `1`。`kind` 只能是 `process` 或 `launcher`。`process` 要有相对路径 `entry`，指向本目录里一个普通文件。`launcher` 要有 `[launch]`：`program` 同样是本目录内的相对路径，`mode` 是 `with_app` 或 `on_demand`。路径不能是绝对路径，也不能含 `..`。多出来的键（包括 `[permissions]`）会让整份变成不可用。

这五个 id 写对了也不可选：`director`、`local-llm`、`wallpaper`、`pet-desktop`、`local-tts`。原因是「保留，不能当文件夹装入」。

## 放了却看不见

| 放进去的东西 | 结果 |
| --- | --- |
| 根下的 `mod/`（少了 s） | 不扫描 |
| 没有 `mod.toml` 的目录，包括那 7 个公开仓库、`.rlib`、单独一个二进制 | 整夹跳过，连「不可用」都不出现 |
| `mod.toml` 或目录是符号链接 | 不可用 |
| 两份清单用了同一个 `id` | 目录名排序的第一份可勾，其余不可用 |

`mods/` 已被 git 忽略。`git status` 看不到它。`git clean -xfd` 会删掉你放进去的夹。

## 那 7 个公开仓库

它们是 Rust 库（`src/lib.rs`），仓库里没有 `mod.toml`，也没有可以单独运行的程序。编译产物是给链接器用的库文件。拖进 `mods/` 不会出现在启动器或设置里。

要让它们进正在跑的程序，由维护者按 [`two-repo-mods.md`](architecture/two-repo-mods.md) §4 重新编译主程序。这不是往文件夹里丢文件。

不要给这些仓库补一份 `id = "external-input"` 这类假清单。以后真编进程序时，文件夹会显示「与已编进二进制的 Mod 冲突」。

## 和「丢一个文件就加载」的差别

MC 里放下的是作者打好的整包。包里的 json 只是标签，用户不用自己写。放下之后，下次启动会加载包里的代码。

这里对用户的目标也是三步。第三步已派为 **0.3.13**（[`PLAN-0.3.13-folder-spawn-2026-10-21.md`](plans/PLAN-0.3.13-folder-spawn-2026-10-21.md)），**还没落地**。落地之前，进入仍只记住勾选，不启动程序：

1. 作者给你一个打好的文件夹（里面已经有 `mod.toml` 和程序）。
2. 你整夹放进 `mods/`，不改内容。
3. 启动器里看见名字，勾上，进入后那个程序才启动。

今天只做到第 2 步之后的「看见并记住勾选」。单独一份 json 或 toml 没有行为，装不上那 7 个库。
