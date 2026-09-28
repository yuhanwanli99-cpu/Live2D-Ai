# AUDIT STATE — Live2D-Ai-fe 前端夜间长跑审计

## 0. 树确认
- /home/skystar/Live2D-Ai-fe · feat/frontend-redesign · HEAD `5ef879f4`（= 基线代码 +1 docs 提交）
- 禁止树 /home/skystar/Live2D-Ai：未触碰。

## 1. 环境栏
- 服务 18080 在跑（curl 200/302）。产物不审。
- 浏览器只读取证不可用（无 CDP/Chrome）——浏览器级验证一律记验证步骤。
- SDK Flutter 3.47.3；MemoryImage==按 bytes 身份（本地 SDK 源码实读，F-0003-1 证据）。

## 2. 仓库地图（速查）
lib 91 文件/31.4k 行；test ~95/25.6k；part 组合根；条件导入对 ×5；asset_guard_* 4（真守卫，抽查可失败）。

## 3. 已登记项（不重报）
DEC-1/2/3/5/6/7、D1(≥2项不预览)、stagePlaylist 零测试、6 超长文件、AGENTS 版本、§9.7。

## 4. 进度
- Phase 1 · 队列位置 5/11
- 已完成：0001(main+app) · 0002(display_prefs+data) · 0003(sections) · 0004(mods/+controller)
- in_progress：BATCH-0005（lib/ui/ 上半：chat_panel / message_bubble / theme / session_sheet / audio_bar 补审 等）
- 阻塞：无
- **开放计数：P0=0 · P1=4（0001-1 错误出路 / 0002-1 滑杆重发 / 0003-1 壁纸重解码 / 0003-2 自检成功无反馈）· P2=8 · P3=5**
- 同根因组（预 CONSOLIDATION）：
  - 【图像字节无备忘化】F-0002-1（stage-bg 重发）+ F-0003-1（壳解码重跑）——同一资产链路两种浪费。
  - 【启动竞态族】F-0001-3 + F-0002-2（同一水合窗口两形态）。
  - 【桥事件半接线】F-0001-4（progress 无听众）+ F-0003-2（result 通道只有错误槽）。
  - 【下标当身份】F-0003-3（多选删错）与 slideshow 索引族（F-0001-2）同「位置 vs 身份」病。
  - 【派生值不随源刷新】F-0004-1（记忆列表不随会话桶）与 F-0001-2（预览不驱动轮播索引）同「界面里有第二份从服务端取的快照」病。
- 修正记录：F-0001-2 主 UI 路径被 D1 挡（预览只在 1 项可达）→ 降级「潜在/latent」，正式改写在 CONSOLIDATION-01。

## 5. 未核实候选池
- 非 ApiException 逃逸 → _loadAdmin/_testLlm 卡 loading（B7/B4 复核）。
- 渲染面重复 stage-bg 重解码幅度（Rust 侧记账）。
- F-0003-1 掉帧幅度、F-0002-2 窗口长度（需真机）。
- nav_host 裸 AppRadius.xl → design 批。
- ReorderableListView onReorderItem SDK 语义逐行核（B11/SDK 抽查）。
- _zoom 异常吞没 → live2d 批。
- memory record id 是否桶内唯一（影响 F-0004-1 严重度）——Rust 侧，范围外。
- persona_card_picker_web 的 data URL 解析边界 → B11（web/ 批）复核。
- Mod 面板在 ExpansionTile 收起/展开时 State 是否稳定保留 → B5（ui/ 批）顺带。

## 6. 工作队列
Phase 1：1✓ 2✓ 3✓ 4[→]mods/+controller 5 ui/×2 6 design/ 7 chat/+state/ 8 api/ 9 audio/ 10 live2d/ 11 web/+pubspec 12 test/缺口 → Phase 2 → 3 → 4 → 5 → 回 1
（CONSOLIDATION-01 = 0005 关批时）

## 7. 质量自评（累计）
- B1 5/5 硬；B2 3/3 硬；B3 6/6 硬（其中 1 条 P1 的幅度部分为推断、已标注；证伪 1 条候选=stage-clock）；B4 3/3 硬（1 条 P2 代码级可达+双向验证步骤，其余 2 条 P3 确凿）。
- 假绿灯狩猎顺带结果：本 3 批抽查的守卫（asset_guard×2、transient_results、director 持久化交集、field_row、background_store 故障矩阵、display_prefs）均为真可失败；发现 1 处**覆盖缺口型**（FieldActionRow 成功态无用例，已并入 F-0003-2）。
- 覆盖率：lib 全读 37 文件(40.7%)+局部 12；test 抽查 21。
