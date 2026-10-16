# 执行单：终端不要打三遍，列表碎片不要单独念，主动档空规则要有点头（2026-10-13）

> **不要执行本文件。** T1（终端回显 / Melo `quiet`）和 T2（碎片句静音）已并入
> [`PROMPT-tts-sentence-continues-2026-10-15.md`](PROMPT-tts-sentence-continues-2026-10-15.md)。
> T3（主动档补点头）和 T4（`ignite.sh` 重编过期 Flutter）本轮不做。

> 交给工作 agent。本单就是范围。不要再调研，不要扩大。
> 树：`/home/skystar/Live2D-Ai`，分支 `main`。已有未提交改动留着，不要还原，不要另开分支。
> 不要改 `.env`、`mods.json`、`live2d-ai.toml`。不要点火，不要启停 18080。
> 不要 commit、不要 tag、不要 push、不要改版本号。不要提高 `DOCS_BUDGET_LINES`。
> 不要改 `CHANGELOG.md`、发布说明、`docs/audit/**`。
> 不要新增 WS 帧。不要改 `CLEAN_SYSTEM_PROMPT` 正文、思考关断、`[performance].enabled` 缺省、`action_tx`、`mod_count_is_six`、cue 的 2000ms。
> `supervisor.rs` 已是 1000 行，本单不要改它。换行仍是句界，不要改 `is_terminator`。
> 界面文案走字体子集。新 Rust `src` 文件 ≤500。

## 0. 今晚这次点火实际发生了什么

用户问「世界与e的联系是？」。进程日志里 `motion=true bias="eager"`。一轮成功，`assistant_replied` 只有一份正文。浏览器不是把整段复制了三遍。

终端里同一句最多出现三次，所以看起来像三遍回答：

1. `crates/live2d-ai-desktop/src/supervisor/handlers.rs` 对 `EngineEvent::TextDelta` 做 `print!("{text}")`，原始流直接打到点火终端。
2. Melo 的 `tts_to_file` 默认 `quiet=False`，会再打印 `Text split to sentences.` 和切出来的小段。调用点在 `melo/serve.py`。
3. `text_delta` 的 tracing 日志再打一遍已上屏的句子。

听感上像重复播放，是因为分句把列表切碎了，每一小段各合成一次：

- `1.` `2.` `3.` `4.` 各自成句。`classify_period` 看到数字后面的 `.`，下一个字符不是数字也不是小写字母，就当成句号。
- 单独一行的 `**` 也成句。成对的 `**` 被换行拆开之后，`clean_for_tts` 留不住这对标记。
- 小标题「英文谐音」念一遍，后面正文再念一遍。这是模型的列表，不是第二遍合成。
- Melo 自己还会按逗号把我们的一句再切开，但那是同一次 HTTP 里拼成一段音频，不是再请求一次。

动作没有，是因为主动档的空数组回退走到了规则表，规则表对这几句都返回 0 条。日志是 `主动档：模型交回空数组，本句改用规则表 sentence_seq=… cues=0`。问号在用户那句上，回复各句没有 `？` / `?`，也没有词表里的情绪词。空 cue 不发 `action_cue`。另有几句 `clean_plan_invalid`，同样没有动作。

事件流仍是旧样子，是因为 `./scripts/ignite.sh --build` 在 `shell/flutter/build/web/index.html` 已存在时不重编 Flutter。新的换行和详情在 Dart 里，浏览器吃的还是旧产物。

## 1. 做完是什么样

1. 点火终端不再回显模型原文，Melo 不再打印切句。tracing 的 `text_delta` 日志留着。
2. 纯列表序号（`1.` `2.` 这种）和洗完没有可念文字的标记句，走已有静音句，不单独发 TTS。正文照上屏。
3. 主动档：模型交回空数组，规则表也是空，而这句洗完还有可念文字时，补一条 `nod`（点头），`ttl_ms` 仍是 2000。克制和平常的空数组仍然不补。列表序号和空标记句不补。
4. `./scripts/ignite.sh --build` 在 Flutter 源码新于 `build/web/main.dart.js` 时重编前端。已有产物且源码没变时不重编。

## 2. 任务

### T1. 终端只留一份

- 删掉 `handlers.rs` 里 `TextDelta` 的 `print!` 和那次 `flush`。错误路径的 `println!` 不动。
- `melo/serve.py` 调 `tts_to_file` 时传 `quiet=True`。

### T2. 碎片句静音

- 在送 TTS 之前判定。洗完（`clean_for_tts`）之后如果只剩空白、纯 `*` `#` `` ` ``，或整句匹配「若干数字 + 一个英文句点」，这句走 `silent_sentence`。屏幕上的字不动。
- 不要把换行从句界里拿掉。不要按字符位置硬切。

### T3. 主动档补点头

- 只在 `MotionBias` 的主动档、模型 `cues` 是空数组、`rule_cues_for_sentence` 也是空、且 T2 判定这句有可念文字时，cue 变成一条 `preset_id=nod`。
- 克制、平常、模型自己给了 id、规则表已经给了 id、`clean_plan_invalid` 走原有失败回退，这四种都不改。
- `nod` 必须来自导演预设允许集，不要新造 id。

### T4. `--build` 会重编过期的 Flutter

- `scripts/ignite.sh`：`index.html` 在，但 `shell/flutter/lib` 或 `shell/flutter/web` 里有文件新于 `shell/flutter/build/web/main.dart.js` 时，走现有的 `flutter build web --release --base-href /app/ --no-web-resources-cdn`。
- 不要因此去重编 wasm。

## 3. 验收

不要点火，不要跑全仓。代理变量按仓库惯例卸掉。

```bash
env -u http_proxy -u https_proxy -u HTTP_PROXY -u HTTPS_PROXY -u ALL_PROXY -u all_proxy \
  cargo test -p live2d-ai-runtime --lib list_marker
env -u http_proxy -u https_proxy -u HTTP_PROXY -u HTTPS_PROXY -u ALL_PROXY -u all_proxy \
  cargo test -p live2d-ai-runtime --test conversation_engine_motion eager
bash -n scripts/ignite.sh
```

测试名可以对上新名字。断言至少包括：`1.` 不进 TTS 文本；`**` 单独成句时不进 TTS；主动档加中性句得到 `nod`；克制档加空数组仍是空；`handlers.rs` 生产代码里不再有 `print!("{text}")`。

回报：改了哪些文件、三条命令的退出码。没跑的写明没跑。

## 4. 明确不做

不改 Melo 的模型，不改口型，不把用户输入拿去打情绪，不给克制/平常补动作，不重写事件流（那一版已经在源码里，差一次前端构建）。
