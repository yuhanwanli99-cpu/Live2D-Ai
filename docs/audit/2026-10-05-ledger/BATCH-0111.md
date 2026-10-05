# BATCH-0111 · ⭐ **F-0111-01（P2）：ASR token 经 argv 传给 sidecar 子进程**

Phase 1 · 域覆盖 · Mod 根第 8 批

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-mod-voice-input/src/sidecar.rs` — （定点 116-139：`build_sidecar_argv`）
2. `crates/live2d-ai-mod-voice-input/src/commands.rs` — （定点 262-306，B0110 已读）
3. `docs/examples/voice-sidecar/voice_sidecar.py` — （定点 483-493：参数与环境兜底）

## 跑过的命令（全部只读）
```
grep -n "fn build_sidecar_argv" -A 32 mod-voice-input/src/sidecar.rs
grep -rn "ps\b|进程表|命令行|argv" mod-voice-input/src/*.rs | grep -i "ps |进程表|可见|leak|泄露"
git ls-files 'docs/examples/voice-sidecar'
grep -n "token" docs/examples/voice-sidecar/*.py
ls docs/examples/voice-sidecar/ ; grep -n "VOICE_INPUT_TOKEN|environ|getenv|add_argument" docs/examples/voice-sidecar/*.py
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## ★ 本批产出：**F-0111-01（P2）** —— 红线 R 在**进程边界**上的一条通道
`sidecar.rs:132-135` 把**明文 token 作为 argv** 传给 sidecar；`commands.rs:293` 逐参数 spawn
⇒ token 出现在 **`/proc/<pid>/cmdline`**，而 `hidepid` **默认 0** ⇒ **同机任何用户/进程可读**
（窗口 = 转写时长，数秒）。
**关键：接收侧已支持更安全的通道** —— `voice_sidecar.py:492` 有
`default=os.environ.get("VOICE_INPUT_TOKEN", "")`，而 Rust 侧**从不设这个环境变量**。
⇒ 修法**一行**（`cmd.env("VOICE_INPUT_TOKEN", …)`，并去掉 argv 里的 `--token`）。
**分量在于与本仓纪律不一致**：密钥不出现在 `GET /api/v1/env`（B0009）· 日志 sink 脱敏（B0017）·
Mod `state_json`（B0105）· 只经 `secrets::lookup` 解析（B0104）—— **argv 是唯一被漏掉的通道**，
且 `grep "ps |进程表|可见|泄露"` 在该 crate **零命中** ⇒ 没有注释说明它被考虑过。
**非远程面**（B0110 已核：token 只能由本机用户经 loopback+Origin 的 Mod config 路由写入）。

## 未核实项
1. `voice_sidecar.py` 全文未读（只核了 483-493 的参数面）
2. `--url` 是否可能带凭据（`user:pass@host`）未核 ⇒ 同一条 argv 通道
3. `memory` 四文件、`persona/sessions.rs`、`wallpaper`(729)、`template` 未读
4. `voice-input` 的其余部分（转写路径、token 传递到 HTTP 请求体）未读

## 本批新增
P0 0 · P1 0 · **P2 1** · P3 0
