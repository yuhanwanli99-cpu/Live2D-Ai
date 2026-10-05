# BATCH-0110 · **修正后的全域扫描**（按 API 族枚举）+ `voice-input` 的进程执行能力

Phase 1 · 域覆盖 · Mod 根第 7 批

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-mod-voice-input/src/commands.rs` — （定点 262-306：脚本解析 + spawn）
2. 8 个 Mod crate 的**全域扫描**（文件读 4 族 / 文件写 4 族 / env 4 族 / 进程网络）

## 跑过的命令（全部只读，均**穷举无 head**）
```
grep -rn "fs::read|File::open|read_to_string|read_to_end" crates/live2d-ai-mod-*/src/*.rs   # 读 4 族
grep -rn "fs::write|File::create|OpenOptions|write_all"          crates/live2d-ai-mod-*/src/*.rs   # 写 4 族
grep -rn "env::var|env::set_var|env::remove_var|env::vars"      crates/live2d-ai-mod-*/src/*.rs   # env 4 族
grep -rn "Command::new|std::net|TcpStream|UdpSocket"            crates/live2d-ai-mod-*/src/*.rs   # 进程/网络
sed -n '262,306p' mod-voice-input/src/commands.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## ★ 本批产出：**0 条新发现**；但**修正后的扫描改变了两处结论的可靠性**
### ① 环境变量：**零命中**（唯一一条是注释）
`external-input/src/lib.rs:632`：「用注入的 lookup 构造，**不**用 `std::env::set_var`（并发下不可靠）」
⇒ **没有任何 Mod 读进程环境** ⇒ 密钥只能经 `secrets::lookup`（B0104 结论**这次有穷举证据**）。

### ② 进程 / 网络：5 个在册 Mod 中**只有 `voice-input` 有进程执行能力**
| 命中 | 归属 | 判定 |
|---|---|---|
| `Command::new(&argv[0])` | **`voice-input/commands.rs:292`** | **在册 Mod 唯一的进程执行能力** |
| `Command::new` / `TcpStream` | `local-llm/lib.rs` | **已废止未注册**（B0040）⇒ 不在产品路径 |
| `TcpListener` | `director/staging_http.rs:206`、`memory/summary_http.rs:164` | 测试用 mock server（`use` 语句位置） |

`voice-input` 的 spawn 实现**安全选择正确**：
- `:291` 「spawn：**逐参数**，绝不拼 shell 字符串」⇒ `Command::new(&argv[0])` + `cmd.args(&argv[1..])`
  ⇒ **无 shell 注入面**（token / 音频路径含用户内容，这是最容易出事的地方）；
- `:266` 先 `is_file()` 校验 + 可读错误；
- `:294` `stdout(null)` / `:295` `stderr(piped)` ⇒ 子进程输出**灌不进宿主 stdout**，stderr 留作诊断；
- `:300-306` wait 放**后台线程**，注释「**立刻返回，HTTP 线程绝不等 sidecar**」
  ⇒ 与「自检与产品链路抢资源 → 自检说谎」同一条纪律。

### ③ 输入来源：**只有本机用户能设**，无远程入口
`script` 来自 ① Mod 的 `sidecar_script` 配置 ② 缺省 `<config 目录>/docs/examples/voice-sidecar/voice_sidecar.py`（:262 的错误文案逐字说明）。
而 `sidecar_script` 只能经 `POST /api/v1/mods/voice-input/config` 写入 —— 该路由**loopback + Origin 校验**
（B0045 已核 mutating 路由的前置校验）⇒ **无远程入口**；且 F-0062-01 已核没有任何 Mod 能写 `mods.json`。
⇒ **结论：不是可远程利用的提权面**，但是**一份应当被记账的能力不对称**。

## ⭐ 记一条**能力台账**（非发现）
> 5 个在册 Mod 的宿主能力：`external-input`（`say_tx` + token 比对）· `persona`（`apply_settings` +
> `session_prompts`）· `voice-input`（**`Command::new`** + `say_tx`）· `memory`（LLM HTTP + 自己的 store）·
> `director`（LLM HTTP（默认关）+ `cues`）。
> **只有 `voice-input` 拥有进程执行能力**；其参数只能由本机用户经 Mod 设置写入。
⇒ 审计后续 Mod 时可**直接对照这张表**，新增能力时会立刻显眼。

### ④ 扫描口径的**第三种失效形态**已确认（规则 3 变体 2/7）
B0104 的模式列表里**既没有 `Command::new` 也没有 `std::net` / `fs::read`**，
⇒ 那次「全域扫描」**漏掉了整个进程执行面**。
**结论不变**（没有 Mod 写宿主配置、没有 Mod 读 `.env`），但**「进程执行」这一维度当时是空白**。
⇒ 这补实了 B0109 立的那条规则：**grep 做全域扫描前，模式列表必须按 API 族枚举**
（文件读 4 族 / 文件写 4 族 / env 4 族 / 进程 3 族 / 网络 3 族）。

## 未核实项
1. `voice-input` 其余部分未读（`sidecar.rs` 的 `build_sidecar_argv` / 转写路径 / token 传递）
2. `memory/{summary.rs,store.rs,summary_store.rs,commands.rs}` 本体未读
3. `persona` 的 `sessions.rs` 全文与 `PersonaCard::parse_json` 未读
4. `wallpaper`(729，**已封存**) · `template` 未读
5. 各 Mod 的 `settings_spec` 与实际读取的键是否一致未核
