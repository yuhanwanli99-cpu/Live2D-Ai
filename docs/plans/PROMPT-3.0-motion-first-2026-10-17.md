# 执行单：3.0 队首 · 外部事件忙时合并（2026-10-17）

> `v0.2.7` 已打，不移动。本片插在动作片之前。
> 不升版本，不打 `0.3.x`，不推送，除非维护者点名。
> 不改 `.env`、`mods.json`、`live2d-ai.toml`。不提高 docs 预算。

## 做这一件事

`POST /api/v1/external/chat` 在主链忙时现在回 `busy` 并丢掉文本。改成：上一条还没处理完时，端口照常收下，先不注入；这一窗口里挤进来的文本按到达顺序合成一条，等上一条结束再推一次。窗口里没有积压时，仍是一条一条注入。鉴权、`mod_disabled`、loopback 不变。不新开 socket，监听仍是 `[web].port`。

## 验收

忙时连送三条，只注入一次，正文是这三条合成的那一条。空窗口仍逐条注入。`cargo fmt --all -- --check`，`cargo test -p live2d-ai-desktop`，`cargo clippy -p live2d-ai-desktop --all-targets -- -D warnings`。

做完本地 commit。不要自己打 `0.3.x`。

## 下一片（本片不要做）

手势槽时间到现在是立即清槽。改成与撤销同一条放回，时长 `ttl_ms × (1 - HOLD_END)`。表情、呼吸、眨眼、`BODY_LAG`、`ATTACK_END`、`HOLD_END` 不动。不新增预设。`action_tx` 继续休眠。测试变红再修绿。渲染面改了要重建 wasm，并写明浏览器没看。

