# 0.2.3-rc.1 开箱即用 · 交给下一个 agent

> 立档：2026-10-09。状态：**活**。真源是本文件。
> 用户写的版本名是 `0.2.3-rc1`。仓库既有 tag 形状是 `v0.2.1-rc.1`，所以版本串用 **`0.2.3-rc.1`**，tag 用 **`v0.2.3-rc.1`**。
> 这一份**允许**提交、打 tag、推送 `origin`（`https://github.com/yuhanwanli99-cpu/Live2D-Ai.git`）。
> 不要改本机正在用的 `mods.json`、`.env`、`live2d-ai.toml`。不要改 `/home/skystar/CosyVoice 3.0`，不要停 `127.0.0.1:8080`。
> 不要 force-push，不要改写已有历史。`docs/releases/v0.3.0.md` 不要复活。
> 上一份 `PROMPT-cosyvoice3-settings-ia-2026-10-09.md` 里的「不要提交」到此为止，不约束本文件。

## 开箱指什么

克隆仓库、按发布说明起服务之后：

- 舞台能渲染**白**模型，不用再自行导入。
- `本地tts_MeloTTS` 随缺省启用拉起，出声走它。仓库里带着中文 checkpoint。
- CosyVoice3 那个 Mod **不在注册表里**，缺省也不会拉起。代码留在 crate 里，标封存。
- 对话不带任何 API Key。用户自己去 DeepSeek 申请，填进界面。地址和模型名仍是 DeepSeek 官方。

## 版本与上传

三处版本一起改：`Cargo.toml` 的 `version = "0.2.3-rc.1"`，`shell/flutter/pubspec.yaml` 的 `version: 0.2.3-rc.1+16`，两份 README 首屏。`AGENTS.md` 只改开头那句现行版本。新增 `docs/releases/v0.2.3-rc.1.md`，`docs/README.md` 的「当前」链到它。历史发布说明不改。

一次提交，信息说明这是 `0.2.3-rc.1` 开箱。然后 `git tag v0.2.3-rc.1`，`git push origin main`，`git push origin v0.2.3-rc.1`。推送要带上 Git LFS 对象。认证失败就停，把远端原话写进结果，不要猜 token，不要 force。

提交前看 `git status`。不要 `git add -A`。不要提交：`.env`、`mods.json`、`live2d-ai.toml`、`target/`、任何 `.venv/`、`__pycache__/`、CosyVoice 的 `weights/`、个人录音、根目录散落的调研草稿。

## 白模型进仓库

维护者裁决：**进仓库**。作者原条约没有写明可以公开再分发，所以不要改写成「作者已授权」。`assets/models/bai/MODEL_LICENSE.md` 保留原来六条，下面加一节「本仓库的特殊说明」，用这四句的意思，不要再加营销：

- 模型版权归原作者。本项目的代码许可不覆盖这些模型文件。
- 放进仓库只为克隆后舞台能渲染。
- 免责：使用、二创、直播产生的纠纷由使用者自行承担。
- 权利人提出可核验的删除要求后，维护者在 24 小时内从 `main` 去掉这些文件并推送。这个 tag 若已经发出，同一时限内删除远端 tag 引用。不改写更早的历史。

`.gitignore` 现在是 `assets/models/*`。为 `assets/models/bai/` 开例外，并用 `git check-ignore -v` 确认下面这些**会**被跟踪：

- `MODEL_LICENSE.md`
- `runtime/bai.model3.json`、`bai.moc3`、`bai.cdi3.json`、`bai.physics3.json`
- `runtime/bai.16384/texture_00_4096.png`（model3 的 Textures 点的是这一张）

`runtime/bai.16384/texture_00.png` 没有被 model3 引用，不要入库。最大的入库文件约 8.6MB，普通 git 即可，不要给白模型加 LFS。`model_registry.json` 若仍被忽略，默认渲染不要靠它：`BUILTIN_MODEL_URL` 继续能指向这套已入库的 `bai.model3.json`，新鲜克隆不用先导入。

## MeloTTS 自动拉起

`local-tts-melo` 进入 `default_mods_manifest`，`enabled: true`，`launch_mode` 为 `with_app`。缺省程序仍是仓库内 `melo/start.sh`。`external-input` 继续缺省启用。persona / voice-input / memory / director 仍缺省停用。

出厂 `[tts]` 缺省改成这条进程，**Mod 仍然不写 `[tts]`**：

- `base_url = "http://127.0.0.1:8091/v1"`
- `voice = "ZH"`（`melo/weights/config.json` 里 `spk2id` 的键）
- `response_format = "pcm"`
- `sample_rate = 44100`（这份 config 的 `sampling_rate`）
- `channels = 1`
- 不设 TTS 密钥

改的是代码缺省和 `live2d-ai.toml.example`。本机那份 `live2d-ai.toml` 不动。因此变红的测试改成新缺省，不要保留 8080 / 24000 / `alloy` 当出厂值。

`checkpoint.pth` 从 Hugging Face `myshell-ai/MeloTTS-Chinese` 拉到 `crates/live2d-ai-mod-local-tts/melo/weights/checkpoint.pth`。只走已经写好的那一条 Git LFS。推送前确认它是 LFS 指针，不是 208MB 普通 blob。`config.json` 保持普通文件。不要下其他语言，不要拷 `/home/skystar/tts/MeloTTS`。

第一次启动可以按 tag `v0.1.2` 装 venv。装失败或进程马上以非 0 退出时，`start` 必须返回 `Err`，Mod 状态是 `Failed`，错误里带脚本印出的原因。不要显示「已启用」而其实没在听端口。BERT 仍不入库；缺它同样是失败，不是静默下载成功。`serve.py` 对这组缺省（pcm、44100、音色 ZH）必须返回音频。做不到就改到能做，不要回一套错的 PCM 还报 200。

界面上那句操作规则改成：`扩展 本地tts_MeloTTS 只拉起进程，不会改写这个地址。`

## CosyVoice3 封存

`local-tts`（界面名 `本地tts_CosyVoice3-0.5B`）从 `AVAILABLE_MOD_FACTORIES` 拿掉。crate 和 `engine/` 留在树上，头注写明：封存，还要适配，未适配前不要挂回。缺省 manifest 里不能有它。工厂数回到 6：`external-input`、`persona`、`voice-input`、`memory`、`director`、`local-tts-melo`。`mod_count_is_seven` 改回 `mod_count_is_six`，id 集合同步。不要删 `engine/`，不要动仓库外的 CosyVoice 目录，不要停 8080。

`docs/architecture/tts-is-core.md` 文末和 `docs/architecture/mod-product-chain.md` 的现行句改成：出厂出声是 MeloTTS；CosyVoice3 封存、未注册。历史段落不改。

## 对话密钥

仓库不提供、不提交任何 LLM Key。`[llm]` 仍是 `https://api.deepseek.com/v1` 和 `deepseek-flash`。`live2d-ai.toml.example` 把 `api_key_env` 写成 `DEEPSEEK_API_KEY`，值为空。设置里密钥那一行只留一句：`到 DeepSeek 申请自己的 Key，填到下面。` 没有 Key 时错误仍走现有的 401 码，不要换成一套无码文案。

## 禁止

- 把白模型的条约改成「已经获授权再分发」。
- 入库 `texture_00.png`、Bai 以外的模型、CosyVoice 权重、`.venv`、整棵 Python 树。
- 裸 `git add` MeloTTS 的 `checkpoint.pth`。
- 改本机 `live2d-ai.toml` / `.env` / `mods.json`，或让 Mod 在运行时改写 `[tts]`。
- 挂回 CosyVoice 工厂，或默认拉起它。
- 修 `docs/audit/2026-10-09-silent-errors/FINDINGS.md` 里除「子进程立刻非 0 → Failed」以外的条目。
- 调高 `RATCHET_SRC_RS_500` 或 `PLAN_DESKTOP_DEPS`。给白模型或设置文案先改字体子集；缺字就改字。
- force-push。认证失败仍声称已上传。

## 验收

都要 exit 0，并贴最后约 30 行。没跑的不要写成绿。

```bash
cargo test -p live2d-ai-mod-local-tts
cargo test -p live2d-ai-desktop --bin live2d-ai-desktop mod_count_is_six default_mods_manifest
cargo fmt --all -- --check
cargo run -p xtask -- rust-ratio
cd shell/flutter && /home/skystar/flutter/bin/flutter analyze && /home/skystar/flutter/bin/flutter test test/settings_sections_test.dart test/font_subset_test.dart
git check-ignore -v assets/models/bai/runtime/bai.moc3 assets/models/bai/runtime/bai.16384/texture_00_4096.png
git lfs ls-files
```

`git check-ignore` 对那两个白模型文件应**没有**输出（没被忽略）。`git lfs ls-files` 里要有 `checkpoint.pth`。推送成功后写明 `main` 的提交和 tag 是否都在 `origin`。认证失败就写远端原话。

## 提示词（整段复制给下一个 agent）

```text
你在 /home/skystar/Live2D-Ai，分支 main。先读 docs/plans/PROMPT-0.2.3-rc.1-oob-2026-10-09.md，按它做完并推送。版本串是 0.2.3-rc.1，tag 是 v0.2.3-rc.1。可以提交、打 tag、推 origin。不要 force-push，不要改本机 mods.json、.env、live2d-ai.toml，不要动 /home/skystar/CosyVoice 3.0，不要停 8080。

开箱四件事：
1. 白模型进仓库。保留作者原六条条约，另加免责和「权利人要求删除后 24 小时内从 main 去掉并删除已发出的该 tag」。不要写成作者已授权。gitignore 只放行 bai 的运行时文件；model3 引用的 texture_00_4096.png 要入库，未引用的 texture_00.png 不要。BUILTIN 路径指向这套文件，新鲜克隆不用导入。
2. local-tts-melo 缺省启用，with_app，开机拉起。出厂 [tts] 缺省改为 http://127.0.0.1:8091/v1、音色 ZH、pcm、44100、单声道、无密钥。Mod 仍不写 [tts]。checkpoint.pth 用现有 Git LFS 放进 melo/weights/，从 Hugging Face myshell-ai/MeloTTS-Chinese 拉，禁止裸 git add。子进程立刻非 0 退出必须让 start 返回 Err、状态 Failed。BERT 不入库。
3. CosyVoice3（id local-tts）从工厂列表拿掉并封存，crate 和 engine/ 保留，不要挂回，不要默认拉起。工厂数 6，含 local-tts-melo。界面那句操作规则改指 本地tts_MeloTTS。
4. 不提供 LLM Key。example 里 api_key_env = DEEPSEEK_API_KEY，值为空。界面只留一句：到 DeepSeek 申请自己的 Key，填到下面。地址仍是 https://api.deepseek.com/v1，模型仍是 deepseek-flash。

版本写入 Cargo.toml、pubspec.yaml（0.2.3-rc.1+16）、两份 README、AGENTS.md 开头现行句、docs/releases/v0.2.3-rc.1.md。一次提交后推 main 和 tag，LFS 对象要上去。认证失败就停并贴远端原话。

验收命令和排除清单以计划书为准。贴出命令的最后约 30 行，以及 push 的结果。
```
