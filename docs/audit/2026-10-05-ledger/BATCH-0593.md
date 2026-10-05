# BATCH-0593 落盘（极简）· 「缺省启用 vs 缺省停用」在 main.rs 里已自行注明

## 命令（只读）
```
git ls-files | grep -i "mods.json"                              -> 无（不跟踪）
grep -rn "default_mods_manifest" crates/live2d-ai-desktop/src/ -> cli_entry.rs:612
sed -n '64,80p' crates/live2d-ai-desktop/src/main.rs
```

## main.rs:64-80 的工厂表变更史（逐段）
```
:64 ///
:65 2026-09-14（0.2.0-rc.2，Wave 1 合并）：追加 Wave 1 的 `voice-input` / `wallpape…
:66 两个工厂（均**缺省停用**，`cli_entry::default_mods_manifest` 未收录）。
:68 2026-09-14（0.2.0-rc.3，Wave 2 合并）：追加 `memory`（会话记忆…），同样**缺省停用**。
    数字 5 → 6。**director 当时仍不注册**（Wave 2 只交 RFC）
:71 2026-09-14（Wave 3 合并，未 bump 版本）：追加 `director` **最小骨架** ——
    `TurnPrompt`/`TurnEnded` 产出确定性的 `{emotion,intent,suggested_tts}` 决策，
:73 **只写日志 + `state_json`，零投递**（`action_tx` 仍休眠、不写 `live2d-ai.…）
    数字 6 → 7，**同样缺省停用**（`cli_entry::default_mods_manifest` 未收录…
:74 与 rc.2 删除的那个 director 不同：它**不驱动动作序列**，见
    `docs/architecture/director-mod-v0.md` 与 `docs/architecture/director-rfc.md` §8。
:76 2026-09-14（产品级加强波次，未 bump 版本）：**封存 `wallpaper` + `pet-de…**
:77 ——两者移出本表，数字 7 → **5**。crate 仍留在 workspace（可编译、
    并标 ARCHIVED，**禁止挂回**；理由与恢复条件见…
```

## 三个可核点
1. **B0592 第 3 点的矛盾，解开了** ⇒⇒ W1 写「director Mod **缺省启用**」，
   而 `main.rs:73` 写「**同样缺省停用**（`cli_entry::default_mods_manifest` 未收录）」
   ⇒⇒ **⇒⇒⇒⇒** 两者说的是**不同的东西**：
   - `main.rs` 说的是**「不在 `default_mods_manifest` 里」**（这是代码事实，可核）
   - W1 说的是**「在 `mods.json` 里 `enabled`」**（这是运行时文件，本批未找到）
   ⇒⇒ **⇒⇒⇒⇒** 判据（同 B0592 自己写的那句）：**「缺省」要先问「谁的缺省」**。
   ⇒⇒ **⇒⇒⇒⇒** 而**本审计不判 W1 写错了** —— `mods.json` 不被 git 跟踪
   （`git ls-files | grep mods.json` = 0 命中），它由 `PUT /api/v1/mods` 运行时写入，
   **仓库里查不到它的初值** ⇒⇒ **「缺省启用」这句话在仓库内无法证伪**。
   **⇒⇒⇒⇒** 这本身是一条可记的事实：**一条验收标准引用了一个不被版本控制的文件。**
2. **⇒⇒⇒⇒ main.rs 的注释里有一处「同名词不同物」，且已被前一批撞到**：
   `:74`「与 **rc.2 删除的那个 director** 不同：它**不驱动动作序列**」
   ⇒⇒ 也就是说**代码注释里承认「历史上删过一个 director」**，
   而现在这一个**是另一个东西** ⇒⇒ **⇒⇒⇒⇒ 判据：「删除」在这仓里发生过两次，
   文档只说了第一次**（AGENTS 记的是 rc.2 那次，F-0637-01 的六处证据讲的是第二次）。
   ⇒⇒ **⇒⇒⇒⇒** 这是 B0585 那条「**哪一条被作废，要看它作废的是哪一层**」的又一个实例。
3. **⇒⇒⇒⇒ 数字链完整可查**：5 → 6（+memory）→ 7（+director）→ **5**（-wallpaper -pet-desktop）
   ⇒⇒ **⇒⇒⇒⇒** 每次增减都带日期、原因、和「是否缺省收录」。
   ⇒⇒ **⇒⇒⇒⇒** 判据：**一张注册表的历史，最容易核的证据是它的**数字**变化链**，
   而每一步旁边必须写**为什么**（`:74` 就写了理由，`:77` 写了 ARCHIVED + 禁止挂回）。

## 未核
`mods.json` 运行时初值（仓库内不可查，只能记为不可核）· W1 第 4 条提到的 D1 撤回 ·
`PRODUCT-L1-GOALS-2026-09-15.md` / `action-packs-v0.md` 是否存在 ·
`cli_entry.rs:612` 附近 default_mods_manifest 的实现
