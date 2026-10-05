# BATCH-0266 · ⭐ **Phase 4 开张**：攻 F-0002-01 ⇒ **机制存活、措辞收窄、触发条件钉死**

Phase 4 · **证伪 / 自相矛盾**（按停止规则换轴）—— 不读新文件，改为攻**我自己的结论**

## 跑的命令（全部只读）
```
grep -rn "file_watcher|FileWatcher" crates/ --include=*.rs | grep -v test
grep -n "supervisor_opt" crates/live2d-ai-desktop/src/web_api/cli_entry.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**F-0002-01 追加 Phase 4 结果**（机制成立，措辞收窄）
### ① 攻法与结果
```
cli_entry.rs:119  let supervisor_opt = if std::path::Path::new(&config_path).is_file() { … };
```
⇒ **`supervisor_opt` 为 `None` ⟺ `live2d-ai.toml` 尚不存在**（第一次运行）
⇒ 而 `FileWatcher` 的**唯一构造点**是 `cli_entry.rs:337`，**且就在那个 `if let Some(sup)` 块内**
⇒ ⇒ **没有第二个 watcher**（B0120 问的「别处有没有做 toml 那件事」：**没有**）⇒ **机制成立**

### ② ⭐ 但影响面是「**窗口**」而非「常态」
① 只在**第一次运行**（toml 尚不存在）② 那一轮**只影响 `.env`**（toml 本身无内容可重载）
③ **自愈**：toml 一落盘、下次启动建出 supervisor ⇒ 监视生效
⇒ ⇒ **我原来的措辞「首跑无热重载」过宽** ⇒
**准确表述：「第一次运行（`live2d-ai.toml` 尚不存在）时，`.env` 的改动要等到下次启动才生效」**
⇒ ⇒ **级别仍 P1**（用户可见：改完 key 不生效且无提示，直到重启）——
**但从「无热重载」降为「首跑窗口内无热重载」** ⇒ ⭐ **这正是 Phase 4 想要的：机制更硬、措辞更准**

### ③ 反证（试过并保留一条）
- (a) 「也许别处会补一次 `refresh_from_disk`」——**未核**（PATCH 路径是**UI 保存时的显式调用**，
  **不是**文件监视）⇒ 记为敞口
- (b) 「首跑用户不会改 `.env`」—— **不成立**：首次配置流程**正是**「建 toml + 写 key」这一步
  ⇒ ⇒ **顺序上极可能正好撞上这个窗口** ⇒ **危害不是假设，是流程默认路径** ⇒ ⇒ **这反而加重了它**

### ④ ⭐ 而 Phase 4 的**判据**（本批确立，用于后续）
> **攻一条发现，要产出三样之一**：① **推翻**（撤回）② **收窄**（机制更硬、措辞更准）
> ③ **加重**（找到更现实的触发路径）⇒ **「没攻动」不算结果** ⇒ 必须写成上面三样之一。
⇒ 本批 = **②收窄 + ③加重**（同一批拿到两样）。

## 未核实项
1. ⭐ Phase 4 继续攻其余 P1：**F-0001-01/02 · F-0002-02 · F-0006-03 · F-0013-01 · F-0020-01 · F-0046-01 · F-0049-01**
2. `refresh_from_disk` 除 PATCH 外**是否还有文件变更路径**（反证 (a) 的敞口）
3. 面板余面：`dev_tools_section` ~1845 · `live2d_stage` ~600 · `memory_panel` ~545 ·
   `persona_panel` ~520 · `message_bubble` ~470 · `chat_panel` ~465 · `error_banner` ~15
4. 其余 6 处 `mod_count_is_*` 逐条对账未核 · `ErrorResponse` 定义未读
5. `onReady` 是否间接清过 `presetStatus`（F-0263-01 反证 (a)，**结论不依赖它**）
6. **两侧措辞同步无机制**（B0260 敞口）· `chat_session.dart` 的 `maxSessions=50` 施加点未读
7. 前端 `.dart` 仍 181 未读（**机械值**）；Mod crates 42 未读；`mod-system` 余 8 文件未读
8. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
9. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
10. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
