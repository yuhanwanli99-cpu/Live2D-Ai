# BATCH-0270 · ⭐ **Phase 4 攻 F-0013-01**：最强一击失败，且**③ 加重**——文档叫你手写这个文件

Phase 4 · **证伪 / 自相矛盾** —— 攻第四条 P1

## 跑的命令（全部只读）
```
grep -rn "mods.json" AGENTS.md docs/ crates/.../mod_registry.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**F-0013-01 的 Phase 4 结果 = ③ 加重**
### ① 攻法
若 `mods.json` 是**内部产物**、不该由人编辑 ⇒ 「未知 id 丢失」无害 ⇒ **该推翻**

### ② ⭐ 攻击失败，而且是**被文档**挡回来的
```
docs/external-input.md:125  **启用**：`mods.json` 里 {"mods":{"external-input":{"enabled":true…}}
docs/external-input.md:128  **停用**：`POST …/disable`（**或改 `mods.json`**）
docs/external-input.md:127  **0.2.0-rc.1 起缺省启用**（`mods.json` **不存在时的内建 manifest**）
```
⇒ ⇒ **`mods.json` 明确被文档当作可手写的配置面**（两处给出 JSON 片段，
且「**或改 `mods.json`**」把手改列为**与 API 并列**的选项）⇒ ⇒ **攻击失败**

### ③ ⇒ 而机制正好接上：`:252-253` 整份从 `self.entries` 重建
⇒ **手写的 id 不在 `entries` 里** ⇒ **第一次落盘**（任何一次配置保存 / 启停）**就把它静默抹掉**
⇒ ⇒ ⭐ **③ 加重**：不是「可能发生」，而是「**文档正好叫你这么做**」

### ④ 反证（试过并排除）
- 「文件由程序原子写，用户不会去手改」—— **不成立**：
  **原子写只保证「程序写的时候」不撕裂，不禁止人先写**；而文档明确邀请人写
- ⇒ ⇒ 顺带一条**同类提醒**：`persona-mod-cards.json`（B0220 核过）
  走的是「**规范化后落盘 + tmp+rename**」⇒ **那类**「程序独有的文件」不该照 `mods.json` 的重建法处理

## 未核实项
1. Phase 4 待攻 P1：F-0001-02 · F-0020-01 · F-0046-01 · F-0049-01
2. **那四个「未知 id」在文档里有没有**被点名（若有 ⇒ 可点名受害者；若无 ⇒ 仍成立）
3. 能否点名会在持锁期间 panic 的 Mod（B0268 敞口）· `refresh_from_disk` 的文件变更路径（B0266 敞口）
4. 面板余面：`dev_tools_section` ~1845 · `live2d_stage` ~600 · `memory_panel` ~545 ·
   `persona_panel` ~520 · `message_bubble` ~470 · `chat_panel` ~465 · `error_banner` ~15
5. 其余 6 处 `mod_count_is_*` 逐条对账未核 · `ErrorResponse` 定义未读
6. `onReady` 是否间接清过 `presetStatus`（F-0263-01 反证 (a)，**结论不依赖它**）
7. **两侧措辞同步无机制**（B0260 敞口）· `chat_session.dart` 的 `maxSessions=50` 施加点未读
8. 前端 `.dart` 仍 181 未读（**机械值**）；Mod crates 42 未读；`mod-system` 余 8 文件未读
9. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
10. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
11. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
