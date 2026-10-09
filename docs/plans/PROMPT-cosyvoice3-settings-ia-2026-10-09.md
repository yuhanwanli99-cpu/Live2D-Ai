# CosyVoice3 内置拉起 + 设置分区 · 交给下一个 agent

> 立档：2026-10-09。状态：**活**。真源是本文件。
> 不要提交、不要打 tag、不要推送、不要升版本。不要改仓库里的 `mods.json`、`.env`、`live2d-ai.toml`。
> 不要改 `/home/skystar/CosyVoice 3.0`，不要停 `127.0.0.1:8080` 上已经在跑的垫片。
> 前端把「人设」改名为「模型对话」。字段、后端、角色卡导入不动。
> 设置里具体控件下的功能介绍全部去掉，不要另写一段换上。数字填入只留一行范围。
> 另加第二个 Mod：`本地tts_MeloTTS`。模型从官方仓库拉，不从本机拷。
> 静默错误账本已落盘，本任务不修那 8 条。

## 口径

现在出声是在仓库外执行 `cd "/home/skystar/CosyVoice 3.0" && bash scripts/3-start.sh`。产品路径改成：**已启用的 Mod 自己拉起 Fun-CosyVoice3-0.5B**。模型就是这一款 0.5B（Hugging Face `FunAudioLLM/Fun-CosyVoice3-0.5B-2512`），不是 CosyVoice2，也不是再去调用外面的 `3-start.sh`。

界面上的 Mod 名字用这串原文：`本地tts_CosyVoice3-0.5B`（`ModDescriptor.name` 和 settings spec 的 title）。稳定 id **仍是** `local-tts`。中文、下划线和点不要写进 id：`registeredModIds()` 从 crate 名把 `_` 换成 `-` 来对工厂，URL 和缺省 manifest 断言也认 `local-tts`。

拉起配方放在本仓库的 Mod 根目录 `crates/live2d-ai-mod-local-tts/engine/`（启动脚本 + 一份说明它读哪份权重的配置）。缺省 `launch_mode` 从 `on_apply` 改为 `with_app`：Mod 已启用时，开机（`Boot`）和用户打开开关（`Enable`）都 spawn；`Apply` 和命令 `launch` 照旧 spawn。`program` 缺省指向这份仓库内脚本。脚本用自己的路径找到旁边的权重目录，禁止回落到 `/home/skystar/CosyVoice 3.0`。权重不在、解释器不在、spawn 失败，都返回 `Err`，宿主现有路径标 `Failed`。不重试、不改 `[tts]`、不改走云端、不把失败写成成功。`apply_settings` 次数仍是 0。语音合成页仍是 `[tts]` 的唯一写入者。

### 许可证（本轮结论，按它做，不要再议）

- CosyVoice 代码：`vendor/CosyVoice/LICENSE` 是 Apache-2.0。再分发必须带这份 LICENSE 和 NOTICE，改过的文件要标明。
- 权重：Hugging Face 模型卡 `FunAudioLLM/Fun-CosyVoice3-0.5B-2512` 标的是 apache-2.0（2026-10-09 核对）。可以再分发，随包带上许可证、模型卡出处和修订号。不要改完权重还写成官方原件。
- Matcha-TTS 是 MIT。只有真的拷进仓库的那部分才需要附上版权声明。
- **不要进 git**：`/home/skystar/CosyVoice 3.0/voices/` 里的个人参考录音、`shim/voices.yaml` 的私人音色、`.venv`。这些不是 Apache 作品。
- 本仓库没有 Git LFS（没有 `.gitattributes`）。`llm.pt` 等是数 GB，GitHub 单文件 100MB。**不要 `git add` 权重。** 合规的做法是：Mod 的 `engine/` 里提交 LICENSE 副本、NOTICE、模型卡链接和修订号，以及一个下载脚本，把 `Fun-CosyVoice3-0.5B-2512` 下到 `crates/live2d-ai-mod-local-tts/weights/`（该目录进 `.gitignore`）。下载失败就让启动脚本以非 0 退出，Mod 显示这条错误。
- 不要把 `vendor/CosyVoice` 整棵 Python 树拷进本仓库。`*.py` 计入 `rust-ratio` 分母，门槛是 ≥95%。启动脚本只许是一层薄封装。

### 第二个 Mod：本地tts_MeloTTS（开箱）

与上面那个 CosyVoice 并列，再注册一个 Mod。界面名字用原文 `本地tts_MeloTTS`。稳定 id 用 `local-tts-melo`（不要用中文，也不要占用已经定下的 `local-tts`）。工厂数 6 → 7，`mod_count_is_six` 改成 `mod_count_is_seven`。**不要**新加一条 desktop 的 Cargo 依赖：`RATCHET_DESKTOP_DEPS` 已经是 23，再加一条边会红。第二个工厂放在现有的 `live2d-ai-mod-local-tts` crate 里导出。缺省 manifest 仍只启用 `external-input`。开箱指的是：**启用之后不用填程序路径，也不用再下载语音 checkpoint**。不要默认打开，免得和 `127.0.0.1:8080` 上已有的进程抢端口。

本机没有可用的 MeloTTS 权重。`/home/skystar/tts/MeloTTS` 只有约 23MB 源码，不要拷它，也不要用 `miniconda3/envs/melo`。

- 代码来源：`https://github.com/myshell-ai/MeloTTS`，tag `v0.1.2`，MIT（Copyright (c) 2024 MyShell.ai）。LICENSE 原文放进 Mod 的 `melo/` 目录。
- 权重不在 GitHub Release 里。官方 `melo/download_utils.py` 的中文项指向 Hugging Face `myshell-ai/MeloTTS-Chinese`：`checkpoint.pth`（约 208MB）和 `config.json`。只拉这一套中文，不要英/日/韩/法/西。模型卡同样是 MIT。
- `config.json` 用普通 git。`checkpoint.pth` 超过 GitHub 单文件 100MB，**禁止**裸 `git add`。仓库目前没有 Git LFS。只为这一枚文件加 LFS（`.gitattributes` 里一条），指针进 git，文件本体由 LFS 存。CosyVoice 那 9GB 仍然不进 LFS、不进 git。
- 启动脚本只认仓库内 `crates/live2d-ai-mod-local-tts/melo/weights/` 里的这两份。缺文件就非 0 退出。声明地址用 `http://127.0.0.1:8091/v1`，不要占用 8080，也不要写 `[tts]`。
- 薄封装把 MeloTTS 的输出收成主链已经在用的 OpenAI 兼容 `POST /audio/speech`（pcm）。采样率按请求来；做不到就让这一次请求失败，不要回一套错的 pcm 还报 200。不要改 `tts.rs`。
- Python 包和 `.venv` 不进 git（`*.py` 会计进 rust-ratio，`.venv` 也不是 MIT 作品的一部分）。启用后第一次启动可以按 tag `v0.1.2` 从 GitHub 装进 Mod 目录里的 venv；安装失败必须 `Err`，Mod 显示这条错误。中文推理还会用到 `bert-base-multilingual-uncased`，本轮不把它放进仓库。缺它就 Failed，并在错误里写出缺的是它，不要静默下载完还当成已经开箱。

`stop_child` 已知问题 S-000 见 `docs/audit/2026-10-09-silent-errors/FINDINGS.md`。本任务**不修** S-000 到 S-007。新路径不要再写一种「`wait()` 失败就把句柄丢掉、对外却像停干净了」。可以继续调用现有的 `stop_child`。

### 设置分区

现行一级是：人设、模型库、对话、语音合成、外观与互动、扩展、诊断、开发模式。改完是 **7 项**，顺序固定：

| 顺序 | 一级 | 里面有什么 |
| --- | --- | --- |
| 1 | 模型对话 | **只改前端显示名**。枚举仍是 `SettingsSection.persona`。仍是系统提示词 + 记住几轮。角色卡仍在扩展里的 persona 卡片 |
| 2 | 模型库 | 控件不动。下面的介绍句按「三级介绍」删 |
| 3 | 模型服务 | 取消一级「对话」和一级「语音合成」，合成这一项。页内两组，每组再分普通 / 开发者 |
| 4 | 主题 | 从「外观与互动」拆出。只留现在「外观」那一组：配色 + 背景 |
| 5 | Live2D 动作 | 舞台与口型（缩放、口型灵敏度、口型同步、待机小动作、开发者档位）+ 允许拖动与缩放 |
| 6 | 扩展 | 不动入口。导演卡片见下 |
| 7 | 开发模式 | 开关仍在这一项，以便没开时也能打开自己。诊断内容收进这里 |

### 模型对话（仅前端）

导航 label 和这一页的标题从「人设」改为「模型对话」。`SettingsSection.persona` 这个枚举值不要改名，跳转和 `byIndex(0)` 仍落在它上面。两个字段保持：系统提示词、记住几轮。不改 `[persona]`、不改 persona Mod 的导入命令、不把角色卡搬回这一页。按钮「导入为全局人设」保持。`kPersonaTakeoverNotice` 里指回设置页的「人设」改成「模型对话」（关掉会回到「模型对话」里写的提示词）。历史文档里的「人设」不改。

### 去掉三级功能介绍

一级是分区，二级是页内分组（语言模型 / 语音合成、外观那几张 `GroupCard`、普通 / 开发者）。三级是具体控件。这些控件上、以及分组标题下的功能介绍写得不好，**全部去掉**，不要改写成新的一段。

删的是 `description` / `fieldHelp` 这类说明句，包括搬迁时会带过去的，也包括没改结构的页（模型对话、模型库、背景、诊断、开发模式、扩展字段帮助）。现成例子：`这段话每轮都会先交给模型。`、`留空 = 没有额外人设…`、`对话用 DeepSeek…`、`尾部 / 可省略…`、`本服务没有音色清单…`、`PCM 规格…`、`配色与背景。只影响本机显示…`、`1.00 为标定值…`。测试若钉死其中某一句，改成断言这句**不在**界面上。

留下的只有：控件自己的名字、开关、输入框、输入框里的 placeholder（往哪填，不是介绍）、错误文案和按钮。

**数字填入只留范围。** 有 `min`/`max` 的填入项，说明改成一行 `最小值-最大值`，取控件上已经写死的数，不要改夹持，不要加单位，不要加「0 = 不限制」或后果。用户点名的例子：输出 token 上限现在是 `min: 0`、`max: 32768`，说明只写 `0-32768`。标签仍是「输出 token 上限」，不要改成「输入」。同一规则用在其余数字项上，数从该控件现有的 min/max 抄：记住几轮 `0-200`，采样率 `8000-192000`，声道 `1-2`，模型缩放 `0.5-2.0`，口型灵敏度 `0.2-3.0`，动作幅度三条 `0.2-2.2`。小数按源码常量写，不要另取一档。没有 min/max 的文本框不写范围。

另外三条是操作规则，不是介绍，留下：

- 语音合成的服务地址旁只留一句：扩展 `本地tts_CosyVoice3-0.5B` 只拉起进程，不会改写这个地址。不要再写「云端接口」或「DeepSeek」。
- `kDirectorTakeoverNotice` 原句留下。
- 密钥「留空表示不修改（服务端不回传密钥）」留下。

一级导航的 `SettingsSection.description` 仍要一句短的（现有测试：非空、不重复、不包含自己的 label）。「模型对话」的这句改成不含「人设」、也不含「模型对话」四字的短句，例如 `系统提示词与记住的轮数`。不要写成功能介绍。

「模型服务」页内的分级（用现成的 `SectionHeader` / `GroupCard`，不要新设计系统）：

- 普通 · 语言模型：服务地址、模型名、输出 token 上限、思考、密钥绑定、连通性自检。这些控件从 `llm_section.dart` 搬来，语义不改。
- 开发者模式 · 语言模型：没有现成的高级字段就不要造。不要把已经删掉的「密钥变量名」「表演层」「谁负责动作」加回来。
- 普通 · 语音合成：服务地址、音色、模型名、密钥、连通性自检。地址旁只留上面那一句事实，其余介绍不搬。
- 开发者模式 · 语音合成：采样率、声道、服务端静音（只读）。自检仍然不合成。

`SettingsSection.tts.description` 那句 `音色和出声的服务` 会随枚举一起走。新分区的 description 用中文短句，不要为了凑旧测试把 `TTS` 这个词写进说明，也不要写成功能介绍。枚举从 8 项改成 7 项之后，改 `settings_sections_test.dart`：`byIndex(0)` 仍是 `persona`，label 是「模型对话」，以及所有按旧顺序写死的下标。`去语音合成设置` 这类跳转改到「模型服务」，并落到语音这一组（用 `Key`，不要靠滚动碰运气）。关掉开发者模式时，若人还停在已经不存在的「诊断」上，仍 `_gotoSection(SettingsSection.persona)`，用户看见的标题是「模型对话」。

### 外观拆开，动作强度归导演

`appearance_section.dart` 里现在有四块：外观、舞台与口型、动作幅度、互动。

- 「主题」只拿「外观」。
- 「Live2D 动作」拿「舞台与口型」和「互动」。
- **动作幅度**（头部 / 身体 / 表情三条，以及「本模型覆盖」「恢复跟随全局」）从这两个分区拿掉，放到 `settings/mods/director_panel.dart`。数据仍是 `live2d-ai.toml` 的 `[action]` / `[action.models.<id>]`，仍走现有 PATCH 和 `ActionScalesSyncer`。不要把这三键写进 director 的 `mods.json` 配置，不要接回 `action_tx`。理由：手势、表情、头部动作不是单 LLM 主链的资产，强度旋钮跟产出这些动作的导演 Mod 放在一起。
- 导演卡片现有那句 `kDirectorTakeoverNotice` 保留。那 8 个 `hiddenKeys` 继续隐藏。
- 滑条逻辑搬过去，不要复制出第二套。`director_panel.dart` 现在 136 行；搬完连同下面的调试块仍须 ≤800。塞不下就把滑条放进同库 `part`，不要新开超过 800 行的库文件。

### 诊断与导演调试

- 取消一级「诊断」（`SettingsSection.diagnostics`）。`DiagnosticsSection` 嵌进开发模式页，**只有** `devMode == true` 时渲染。开发模式这一项本身仍然始终出现在导航里（它是开关的唯一入口，藏起来就再也打不开）。
- `dev_tools_developer.dart` 里的 `DirectorObserverSection`（导演可观测四栏）从开发模式页移走，改挂在导演 Mod 卡片下面，作为下级块。`devMode == false` 时这块不渲染（现有 `director_observer_test.dart` 的 `devMode: false` 语义保留）。核心链的开发模式页不再出现导演调试。
- 观测仍然零新 WS 帧、不落盘。不要把导演的词表、二路地址、清空账本加回产品面。

## 要改的文件

- `crates/live2d-ai-mod-local-tts/`：`DESCRIPTOR.name`、spec 标题、`launch_mode` 缺省改为 `with_app`、空 `program` 改为仓库内脚本路径。缺文件或权重时 `Err`。更新因此变红的单测（原先钉死缺省 `on_apply`、空 `program` 即 `Err` 的那几条）。
- 新建 `crates/live2d-ai-mod-local-tts/engine/`（CosyVoice 薄启动脚本、LICENSE 副本、NOTICE、模型卡链接与修订号、下载脚本）和 CosyVoice `weights/` 的 gitignore。脚本失败必须非 0 退出。
- 同 crate 再导出 `local-tts-melo`：`melo/` 下放 MIT LICENSE、`config.json`、LFS 跟踪的 `checkpoint.pth`、薄启动脚本。安装 venv 的目录 gitignore。
- Flutter：`settings_sections.dart`（`persona` 的 label 改为「模型对话」）、`persona_section.dart` 的页标题、`persona_panel.dart` 里指回这一页的那句、`shell_settings.dart` 的 `switch`、`llm_section.dart` / `tts_section.dart` 收成「模型服务」页（可以留原文件当 part，不要复制字段逻辑）、`appearance_section.dart` 拆成主题与 Live2D 动作、`director_panel.dart` 接收动作幅度和导演可观测、`dev_tools_developer.dart` 去掉导演块并在 `devMode` 下挂诊断。上述页面上的三级介绍按上一节删除。
- 活文档只改现行句：`settings_sections.dart` 头上的分区顺序、`AGENTS.md` 里「Flutter 外观与互动有滑条」改成滑条在导演卡片、`tts-is-core.md` 文末那节的缺省 `on_apply` 改成 `with_app`。历史段落不改。

## 禁止

- 改「模型对话」页的两个字段；把角色卡导入搬回这一页；改 Rust / toml 里的 persona。
- 给删掉的介绍换一套新文案。允许留下的只有：输入框 placeholder、三条操作规则、数字项的一行 `min-max`。
- 恢复描边强度、舞台单图轮播、铺法、密钥变量名编辑、表演层开关、「谁负责动作」。
- Mod 写 `[tts]`；失败后改地址、重试、看门狗、回落到仓库外的 `3-start.sh`。
- 裸 `git add` CosyVoice 权重、MeloTTS 的 `checkpoint.pth`（必须走 LFS）、个人录音、`.venv`；拷贝整棵 CosyVoice 或 MeloTTS 的 Python 树。
- 从 `/home/skystar/tts/MeloTTS` 或 conda 环境 `melo` 拷文件。修 FINDINGS.md 里的 S-000…S-007。
- 接回 `action_tx`；把 `local-llm` / `wallpaper` / `pet-desktop` 挂回来。
- 新 WS 帧；把导演 8 个隐藏键画回产品面。
- 把 `RATCHET_SRC_RS_500` 或 `PLAN_DESKTOP_DEPS` 调高。新的或变厚的 Dart 库文件超过 800 行。
- 仓库根目录 `flutter analyze`。恒真测试。提交、推送、升版本。再写一份计划。

## 验收

都要 exit 0，并贴出最后约 30 行。没跑的不要写成绿。

```bash
cargo test -p live2d-ai-mod-local-tts
cargo fmt --all -- --check
cargo run -p xtask -- rust-ratio
cd /home/skystar/Live2D-Ai/shell/flutter
/home/skystar/flutter/bin/flutter analyze
/home/skystar/flutter/bin/flutter test \
  test/settings_sections_test.dart \
  test/font_subset_test.dart \
  test/director_panel_test.dart \
  test/director_observer_test.dart \
  test/restart_notice_test.dart
```

`HTTP_PROXY` 把桌面用例说成 `llm_upstream_502` 时，去掉代理再跑，不要改断言。`18080` 已开就看这五件事：导航第一项是「模型对话」，没有「人设」「对话」「语音合成」「外观与互动」「诊断」；「模型服务」里语言模型和语音合成分成两组；「主题」和「Live2D 动作」分开，动作幅度不在这两页；具体控件下没有功能介绍（抽查模型对话、模型服务、主题、Live2D 动作）；开发模式关掉时看不到诊断和导演调试，打开后诊断在开发模式页、导演调试在扩展里的导演卡片下。不要启用带真实进程的 Mod，也不要为此下载权重。服务没开就写明浏览器没验。新文案先过 `font_subset_test.dart`：缺字就改字，不要先改 `assets/fonts/*.ranges.txt`。

## 提示词（整段复制给下一个 agent）

```text
你在 /home/skystar/Live2D-Ai，分支 main。先读 docs/plans/PROMPT-cosyvoice3-settings-ia-2026-10-09.md，按它实现。不要提交，不要推送，不要升版本。不要改 mods.json、.env、live2d-ai.toml，不要改 /home/skystar/CosyVoice 3.0，不要停 8080。

TTS：产品拉起改为已启用的 local-tts Mod 自己 spawn Fun-CosyVoice3-0.5B（HF FunAudioLLM/Fun-CosyVoice3-0.5B-2512）。不要再调用仓库外的 scripts/3-start.sh，也不要做 CosyVoice2。ModDescriptor.name 和 spec title 改为原文 本地tts_CosyVoice3-0.5B。稳定 id 仍是 local-tts。配方放在 crates/live2d-ai-mod-local-tts/engine/。缺省 launch_mode 改为 with_app（Boot 与 Enable 都 spawn）。program 缺省指向仓库内脚本，脚本只认自己旁边的 weights 目录。权重或解释器缺失、spawn 失败都返回 Err，不重试、不写 [tts]、不回落云端或仓库外路径。apply_settings 仍是 0。

许可证按计划书那一节：CosyVoice 代码与权重都是 Apache-2.0，engine/ 里带 LICENSE、NOTICE、模型卡链接和修订号，另加下载脚本把权重下到 crates/live2d-ai-mod-local-tts/weights/ 并 gitignore。不要 git add 那份权重、个人录音、.venv。不要拷贝整棵 CosyVoice Python 树（rust-ratio 要 ≥95%）。Matcha 只有真拷贝时才附 MIT 声明。

再注册一个 Mod，显示名原文 本地tts_MeloTTS，id 为 local-tts-melo，工厂放在现有 local-tts crate 里（不要新加 desktop 依赖）。mod_count 改为 7。缺省 manifest 仍只有 external-input。代码从 GitHub myshell-ai/MeloTTS tag v0.1.2（MIT）取，不要用本机 /home/skystar/tts/MeloTTS。中文权重从 Hugging Face myshell-ai/MeloTTS-Chinese 取 checkpoint.pth 和 config.json，不要下其他语言。config.json 普通入库；checkpoint.pth 约 208MB，只加一条 Git LFS，禁止裸 git add。启动只认仓库内 melo/weights，声明地址 http://127.0.0.1:8091/v1，不写 [tts]，不改 tts.rs。输出要接成现有的 OpenAI 兼容 /audio/speech pcm，采样率对不上就让该次请求失败。venv 不入库；第一次安装失败必须 Err。BERT 不入库，缺了就 Failed 并写明缺它。不修 FINDINGS.md 的 S-000 到 S-007，也不要新写一种 wait 失败丢句柄还报成功。

设置一级改成 7 项，顺序：模型对话、模型库、模型服务、主题、Live2D 动作、扩展、开发模式。模型对话只改前端显示名：导航和页标题用这四个字，枚举仍是 SettingsSection.persona，两个字段不改，后端和角色卡导入不改。按钮「导入为全局人设」不动。kPersonaTakeoverNotice 里指回设置页的「人设」改成「模型对话」。

取消一级「对话」和一级「语音合成」，合成「模型服务」：普通层是现在的语言模型字段和语音合成字段，开发者模式层只留语音的采样率、声道、服务端静音。不要加回已删的密钥变量名、表演层、「谁负责动作」。地址旁只留一句：扩展 本地tts_CosyVoice3-0.5B 只拉起进程，不会改写这个地址。去语音合成的跳转改到这一页的语音组。

三级功能介绍全部去掉：具体控件和 GroupCard / SectionHeader 上的 description、扩展字段的 fieldHelp。不要换写新的一段。标签、开关、输入框和 placeholder 留下。有 min/max 的数字填入只留一行范围，从该控件现有上下限抄，格式如输出 token 上限的 `0-32768`；不要加「0 = 不限制」或后果，也不要改标签、不要改夹持。kDirectorTakeoverNotice 和「密钥留空表示不修改」留下。钉死旧介绍句的测试改成断言这些句子不在界面上，并断言 token 上限旁是 `0-32768`。一级 SettingsSection.description 仍要一句短的，且不能包含自己的 label，也不能写成介绍段落。

外观与互动拆开：主题 = 现在的配色和背景；Live2D 动作 = 舞台与口型 + 允许拖动与缩放。动作幅度三条和本模型覆盖从这两页拿掉，放到 director_panel.dart。数据仍是 [action] toml 和现有 PATCH / ActionScalesSyncer，不要写进 director 的 mod 配置，不要接回 action_tx。kDirectorTakeoverNotice 和 8 个 hiddenKeys 保持。

取消一级「诊断」。DiagnosticsSection 放进开发模式页，仅 devMode 为真时渲染。开发模式导航项始终在。DirectorObserverSection 从 dev_tools_developer.dart 移到导演 Mod 卡片下级，devMode 为假时不渲染。开发模式页不再出现导演调试。零新 WS 帧，观测不落盘。

更新因此变红的测试和计划书点名的三处活文档。历史段落不改。禁止恒真测试、仓库根目录 flutter analyze、调高 RATCHET_SRC_RS_500 或 PLAN_DESKTOP_DEPS、Dart 库文件超过 800 行。

验收并贴最后约 30 行：
cargo test -p live2d-ai-mod-local-tts
cargo fmt --all -- --check
cargo run -p xtask -- rust-ratio
cd shell/flutter && /home/skystar/flutter/bin/flutter analyze && /home/skystar/flutter/bin/flutter test test/settings_sections_test.dart test/font_subset_test.dart test/director_panel_test.dart test/director_observer_test.dart test/restart_notice_test.dart
18080 已开就按计划书五件事点一眼；不要拉起真实进程，不要下载权重。没开服务就写明浏览器没验。
```
