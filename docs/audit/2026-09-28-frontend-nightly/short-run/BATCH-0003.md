# BATCH-0003 — Phase 1 · 设置分区 UI 域（lib/settings/sections/ 全量）

## 本批读过（全读/关键段全读）
| 文件 | 行 | 一句话 |
|---|---|---|
| lib/settings/sections/dev_tools_section.dart | 1874 | DebugPanels 计时器/监听对称性**齐**（dispose/didUpdateWidget 逐条核过）；_ModConfigTile 有 re-seed 吞草稿问题；Diagnostics 无泄密 |
| lib/settings/sections/appearance_section.dart | 1489 | 表现层纪律好；_LibraryManager 索引选择+重排=删错项；缩略图每帧重解码；D1/DEC-2/6/7 现场=已登记 |
| lib/settings/sections/director_observer_section.dart | 668(头200+feed) | 单例 feed 不落盘（标识符交集守卫真可失败）；stage-clock 30ms 灌环问题已被 D44 折叠补丁修掉（证伪一条候选） |
| lib/settings/sections/llm_section.dart | 189 | 自检成功不显示（F-0003-2）；密钥只上不下 ✓；maxTokens=4096 教训在文案 ✓ |
| lib/settings/sections/tts_section.dart | 185 | 静音只读 ✓（红线 P 在位）；同 F-0003-2；resultIsError 文案启发式 |
| lib/settings/sections/env_key_field.dart | 185 | 安全面教科书级 ✓；但 '**立即生效**' 走裸 Text（F-0003-4） |
| lib/settings/sections/persona_section.dart | 74 | 正常 |
| lib/settings/sections/pane_helpers.dart | 39 | eff* 单判据 ✓ |
| 补读 | — | ui/field_row.dart FieldActionRow+_FieldShell(错误槽) · ui/shell_backdrop.dart(160-224 _imageLayer) · SDK MemoryImage ==/hash 实现(本地 3.47 源码实测) · test/field_row_test 425-479 · asset_guard_tts_config 150-180 · director_observer_test 380-400 |

## 发现
- **F-0003-1（P1）** 壳背景图/缩略图零备忘化：每次宿主重建重跑 base64Decode + MemoryImage 恒 miss 全量重解码（流式回复期间=帧率级）。
- **F-0003-2（P1）** FieldActionRow 成功结果永不显示（只有错误槽）+ resultIsError 用显示文案猜错误（违反自家「码是契约」红线）。
- **F-0003-3（P2）** 背景库多选存**下标**、重排不重对齐 → 重排后批量删除删错图；注释自相矛盾。
- **F-0003-4（P2）** EnvKeyField 成功消息 `**立即生效**` 走裸 Text，星号必上屏；绕过了窗口式星号扫描。
- **F-0003-5（P3）** 动作调试块「归零」不清面板侧基础表情（与表情调试块同名按钮行为分叉）。
- **F-0003-6（P2）** _ModConfigTile：didUpdateWidget 按对象身份 re-seed 吞用户未保存输入；保存/失败消息在切分区（tile 重建）时静默消失。

## 证伪/登记核对（重要）
- stage-clock 30ms 灌满 200 环 → **已被 D44 折叠补丁防住**（ObserverBuffer.add 按 (type,seq) 去重，render_events.dart:253+）。候选撤销。
- DirectorObserverFeed 无落盘路径：标识符交集守卫（director_observer_test:390）为真断言（剥注释+标识符集合，非裸字符串）。
- asset_guard_tts_config：行为断言（点一次回调一次 / 无「试听」/ 草稿段隔离），非空转。
- DebugPanels 三个 Timer + 状态监听：initState/didUpdateWidget/dispose 对称，全部核对过（B 维度本域过）。
- dev 诊断快照不泄密（status 只含码/模型/base_url；日志尾 200 行是设计用途）。
- D1（≥2 项点缩略图=复选不是预览）、DEC-2（两块轮播并存）、DEC-7（显隐相反）现场均为**已登记**，不重报。
- 相关修正：F-0001-2 的「预览→轮播下一拍跳回」主 UI 路径被 D1 挡住（预览只在 1 项时可达，而 1 项不轮播）→ F-0001-2 降级为潜在（jumpTo 未接线的事实仍在，害处改计为「清空/删图后 stale 索引靠 clamp 掩盖」）。留待 CONSOLIDATION-01 正式改写。

## 本批未核实
- F-0003-1 的实际掉帧幅度（真机未测；恒 miss 是 SDK 源码级事实，帧率×代价 是推断）。
- dev 面板 600ms sweep 与 30ms stage-clock 同时开的观感（未跑）。
- ReorderableListView.builder 在 Flutter 3.47 的 onReorderItem 语义是否仍「已移走调整过」（reorder_background_items 注释如此声明；SDK 层未逐行核）。
