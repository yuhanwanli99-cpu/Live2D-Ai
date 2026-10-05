# BATCH-0081 · 「三处拦截」实现 + 难形态测试（第 6 次模式 H 复查）—— **0 条新发现**

Phase 1 · 域覆盖 · 前端 `settings/`

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `shell/flutter/lib/settings/settings_controller.dart` — 409（定点 44-120 头注段 + **322-371**：
   `discard` / `confirmLeave` / `clearError` / `edit` / `pruneAgainstRemote`）
2. `shell/flutter/test/settings_controller_test.dart` — （**枚举全部 36 条用例名**）

## 跑过的命令（全部只读）
```
sed -n '44,120p' settings_controller.dart | grep -n "拦截|///"
grep -n "三处拦截|拦截点|onBeforeUnload|discard|beforeLeave|setField" settings_controller.dart
sed -n '322,371p' settings_controller.dart
git ls-files 'shell/flutter/test' | grep -i "settings|draft"
grep -n "test('" settings_controller_test.dart
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**0 条新发现**；模式 H **第 6 次复查零命中**
### 实现侧：头注承诺**逐条落地**
1. **`confirmLeave`（:332-340）是「三处拦截」的唯一入口** ⇒ 三处调用点共用**一个实现**，
   与 `_applyPrefs`（B0078 核过）同一形状（漏斗化）
2. **`edit`（:351-361）** 写完立刻 `pruneAgainstRemote`，理由写明：
   「与远端原值相同就**撤回该字段的改动**……否则『点开又改回去』会**永久显示「未保存」**」
   ⇒ 一个**已诊断的 UX 缺陷** + 结构性修法
3. **`pruneAgainstRemote`（:363-371）公开**，理由：「公开是为了**让测试能直接构造草稿后调用**」
   ⇒ **可测性是设计输入**（与 `ActionScalesSyncer` 的抽取、`field_runtime.rs` 的纯逻辑拆分同一纪律）
4. `discard()` 幂等（`if (!dirty) return`），同时清草稿与 outcome

### 测试侧：**难形态全部被钉住**（36 条用例中挑关键的 11 条）
| 头注承诺 / 实现性质 | 对应用例名 |
|---|---|
| 头注：「只改一个字段会不会清空同段其它字段」 | `只改一个字段，补丁里**只有**那个字段（不误伤同段其它字段）` |
| 头注 1：dirty 与键序无关 | `dirty 与键序无关（同样两个字段，顺序不同结论相同）` · `TriKeep 不计入 dirty` |
| 头注 2：保存用服务端回填的 view | `保存成功后用**服务端回填的** view（不是本地草稿）` |
| 头注 3：保存失败不清草稿 | `400 url_invalid → failed + error 文案 + **草稿还在**` |
| `edit` 的 prune 行为 | `改一个字段 → dirty；改回原值 → 不 dirty` |
| 未加载时不得误报未保存 | `未加载时 dirty 恒为 false（避免加载失败后弹「未保存」）` |
| 红线 6 前端侧（B0080 已核） | `上屏文案带上服务端错误码（messageWith）` |
| **结局映射的诚实性** | `persisted:false → noChange（不谎报「已保存」）` · `未识别的 apply_status **不谎报已生效**（保守按需重启）` · `缺 apply_status（旧服务端）→ 也按需重启处理` |
| `confirmLeave` 三分支 | `不 dirty → 直接放行，且不问` · `dirty + 选「留下」→ 不放行` · `dirty + 选「放弃」→ 放行且草稿清空` |

⇒ 尤其值得记的两条**诚实性**用例：`persisted:false → noChange（不谎报「已保存」）`
与 `未识别的 apply_status → 保守按需重启` —— 它们守的是**「不说谎」**而不是「功能正确」，
与 AGENTS.md「自检说谎比没有自检更坏」同源，且**用例名就是判据**。
⇒ 模式 H 累计：6 次复查、**0 命中**（3 处历史命中仍是那 3 例，均在早期批次）⇒
**本仓「难形态未被钉住」不是普遍形态**。

## 未核实项
1. `settings_controller.dart:120-322` / `:372-409` 未读
2. **三处拦截的三个调用点**只核了「统一入口 `confirmLeave` 存在」，
   **未核三处是否真的都调了它**（模式 H 的变体：**入口存在 ≠ 三处都接上**）
3. `shell_admin.dart` 余段 / `main.dart` 其余 ~1000 行未读
4. `background_hydration.dart:140-255` 未读；`appearance_background.dart`(1551) 未读
5. 其余 Mod 面板（4 个 / 2284 行）未读

## 本批新增
**0 条**
