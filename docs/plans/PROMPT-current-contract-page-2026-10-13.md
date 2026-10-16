# 执行单：一页现行契约（2026-10-13）

> 交给工作 agent。本单就是范围。不要再调研，不要扩大。
> 树：`/home/skystar/Live2D-Ai`，分支 `main`。工作区里已有未提交改动，留着，不要还原，不要另开分支。
> 不要改 `.env`、`mods.json`、`live2d-ai.toml`。不要点火，不要启停 18080。
> 不要 commit、不要 tag、不要 push、不要改版本号。不要提高 `DOCS_BUDGET_LINES`。
> 不要改 `CHANGELOG.md`、`docs/releases/**`、`docs/audit/**`、`docs/legacy/**`。
> 不要改 Rust / Dart / 脚本。本单只动下面点名的三份活文档，再加新建的那一页。

## 0. 做完是什么样

新人打开仓库，先读一页就知道产品现在是什么。`AGENTS.md` 开头有指针。`docs/README.md` 里把人指去旧表演层的那几句，改成指去这一页。历史段落一行不改。

## 1. 写 `docs/architecture/CURRENT.md`

不超过 120 行。用下面这些事实，不要从旧文档再摘一段贴进来。

- 产品是本地优先的皮套接入服务：对话 → 按句 TTS → 口型跟音频时钟 → Live2D 渲染。桌宠不是底座。身份这句话用「本地优先的皮套接入服务」。
- 标签 `v0.2.5-rc.1` 是已经打过的收口。工作区还有未提交的后续（等本地 TTS 再开浏览器、表演倾向、长数字静音、保存热重载、事件流）。版本号以 `Cargo.toml` 为准，本单不改它。
- 出厂不设人设，`system_prompt` 为空。角色卡仍是 `live2d-ai-mod-persona`，用户自己启用后导入。
- 导演关闭：不出导演动作。导演开启且接管生效：第二路按句清洗，屏幕和口型用这一句的原文切片，动作也按这一句原文。不是按用户输入打五类情绪。
- 表演倾向三档：克制 / 平常 / 主动，缺省平常。保存走「保存并重载」，后端热重载，进程不重启。只有主动档在模型交回空数组时回退按句规则表。
- 连续 17 位及以上的数字，送 TTS 的那份换成「一长串数字」，屏幕上仍是原句。本地 TTS 回「最长支持 16 位」时该句静音，整轮继续。
- 出厂 TTS 是本机 Melo，`127.0.0.1:8091`。`./scripts/ignite.sh` 等到本机 `GET /models` 为 200 才打印 `http://127.0.0.1:18080/app/` 并打开浏览器。
- 在册 Mod 六个：`external-input`、`persona`、`voice-input`、`memory`、`director`、`local-tts-melo`。缺省启用前两个里的 `external-input` 与 `local-tts-melo`。记忆是本地 JSONL 加词重叠，缺省关。`wallpaper`、`pet-desktop`、`local-llm` 已删除，禁止挂回。`action_tx` 休眠。舞台动作走 `action_cue` → 渲染面 `preset`，不走 core reducer。
- 开发工具「B 事件流」是前端已收帧，不是第二路模型的原文。默认不列出「口型时钟」「段结束」。
- 文末三行：这一页和源码冲突时以源码为准；和更旧的架构文冲突时以这一页为准；小改动不新写交接书、台账或发布说明。

## 2. 两处指针

- `AGENTS.md` 标题下、第一段引用之前，加不超过 8 行：现行契约见 `docs/architecture/CURRENT.md`。下面的长文是沿革，不改。
- `docs/README.md`「架构与契约（当前）」里，把表演层那条「主模型不负责表演 / director staging 降为遗留」改成指向 `CURRENT.md` 的一行。同文件开头「当前执行计划」那句改成指向 `CURRENT.md`，不要再把 2026-10-01 的收口计划写成当前计划。
- `docs/DOC-MAP.md` 的表加一行：现行一页契约 = `docs/architecture/CURRENT.md`。真源优先级那句在 `docs/architecture/*` 之前插入 `CURRENT.md`。不要改表里其它行。

## 3. 验收

```bash
cargo run -p xtask -- code-stats --check --only docs
test $(wc -l < docs/architecture/CURRENT.md) -le 120
```

两条都要退出码 0。回报：改了哪些文件、`CURRENT.md` 的行数、docs 门禁的当前行数。

## 4. 明确不做

不搬 `docs/audit/**`，不删 `dist/`，不改过期架构文的正文，不重写 `AGENTS.md` 的历史节。
