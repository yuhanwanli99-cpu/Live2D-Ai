# BATCH-0596 落盘（极简）· 断言体核完：director「缺省停用」被**逐条钉住**，且每条都带理由

## 命令（只读）
```
sed -n '733,750p' crates/live2d-ai-desktop/src/web_api/cli_entry.rs
sed -n '750,775p' 同上
```

## 断言体逐条（:733-767）
```
:733 fn default_mods_manifest_enables_external_input_only() {
:734     let m = default_mods_manifest();
:735-738 assert_eq!(m["mods"]["external-input"]["enabled"], true,
                "external-input 应默认启用（直播弹幕/礼物注入）")
:739-742 assert!(m["mods"].get("local-llm").is_none(),
                "local-llm 已废除启动（0.2.0-rc.1），不得再进缺省 manifest")
:743-746 assert!(m["mods"].get("local-tts").is_none(),
                "TTS 是核心链路（live2d-ai.toml 的 [tts]），不该再以 Mod 形式出…")
       // 0.2.0-rc.2：Wave 1 的 voice-input / wallpaper 只注册、**缺省停用**。
:747-750 assert!(… "voice-input" is_none,
                "voice-input 缺省停用（ASR 后端未接线），不得进缺省 manifest")
:751-754 assert!(… "wallpaper" is_none,
                "wallpaper 缺省停用（mode 缺省 off），不得进缺省 manifest")
       // 0.2.0-rc.3：Wave 2 的 memory 同样只注册、**缺省停用**——
       // 它会写 `persona.system_prompt`，必须由用户明确打开。
:755-760 assert!(… "memory" is_none,
                "memory 缺省停用（会写 persona.system_prompt），不得进缺省 manifes…")
       // Wave 3：director 已注册（第 7 个）但仍**缺省停用**——它是零…
       // 打开与否纯属用户选择，不该进缺省 manifest。
:761-766 assert!(… "director" is_none,
                "director 缺省停用（零投递骨架），不得进缺省 manifest")
:767 }
```

## 三个可核点
1. **⇒⇒⇒⇒ 六条否定断言 + 一条肯定断言**，`director` 那条**在**：
   > "director 缺省停用（**零投递骨架**），不得进缺省 manifest"
   ⇒⇒ **⇒⇒⇒⇒** 而 B0592 核的 W1 写「**director Mod 缺省启用且会产出
   `latest.preset_id` 与 `action_cue`**」⇒⇒ **⇒⇒⇒⇒ 两者在 `default_mods_manifest`
   这一层**直接对立**，且**各有出处**：一个出自代码断言（`:761-766`），
   一个出自派工提示词（W1 必做第 1 条第 ③ 项）。
   ⇒⇒ **⇒⇒⇒⇒** 判据：**代码断言与派工清单对立时，先看两边各自的日期**
   —— `:761` 的注释写「Wave 3」（= 2026-09-14），W1 写「维护者 2026-09-21 裁决」
   ⇒⇒ **⇒⇒⇒⇒ ⇒ 后者更新，但断言还在 ⇒⇒⇒⇒ 这是「改文档没改断言」或
   「派工清单写错」二者之一，本审计不判**（B0271）。
2. **⇒⇒⇒⇒ 一个更硬的观察：断言里 director 的理由是「零投递骨架」** ⇒⇒
   而 `REGISTER-director-v0.md` / `main.rs:73` 用的也是「零投递」这个说法 ⇒⇒
   **⇒⇒⇒⇒** 而 B0585/B0587 核过：`director-rfc.md:48` 已把
   「**零投递 / 不投递 / 缺省停用 / 最小骨架**」等措辞**标为历史记录、不是现行状态**。
   ⇒⇒ **⇒⇒⇒⇒ ⇒ 这条断言的 message 里嵌了一句已被本仓自己判为「非现行」的措辞。**
   ⇒⇒ **⇒⇒⇒⇒** 判据：**断言 message 也会过期**；
   而**它过期时不报警**（assertion 仍绿）⇒⇒ **⇒⇒⇒⇒ 这属「文档债」，不是「测试债」**。
3. **⇒⇒⇒⇒ 六条否定断言每条都带理由**，且理由各不相同：
   `local-llm`（已废除启动）/ `local-tts`（TTS 是核心链路）/
   `voice-input`（ASR 后端未接线）/ `wallpaper`（mode 缺省 off）/
   `memory`（会写 `persona.system_prompt`）/ `director`（零投递骨架）
   ⇒⇒ **⇒⇒⇒⇒** 判据：**一串否定断言若理由相同就该参数化**；
   **理由各不相同就该各写一行** —— 这里选了后者，**代价是行数**，
   **收益是「每条禁掉的东西都带着为什么不禁」**。

## 结论落点
⇒⇒ F-0637-01 仍是 **P1**，但**本批不改它** —— 本批核到的第 1 点
（代码断言 vs 派工清单对立）**另属一处**，与「director 是否还在册」无关。
⇒⇒ 按任务书「同根因合并」：**F-0637-01 讲的是「AGENTS 说删了但六处证据在」**；
**本条讲的是「W1 说启用、断言说停用」** ⇒⇒ **⇒⇒⇒⇒ 不同根因，不并入**，
**只在此处记账**（候选池，等 W1 是否被改过再判）。

## 未核
W1 是否已被执行过（`git log` 查该提示词文件与 AGENTS 的最近改动）·
`PUT /api/v1/mods` 写入方 · `PRODUCT-L1-GOALS-2026-09-15.md` / `action-packs-v0.md`
