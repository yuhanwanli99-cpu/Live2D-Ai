# BATCH-0396 · ⭐⭐⭐ **承诺兑现** —— 而它不是四个字，是一个**带数字、带动作、区分三种原因**的分支

Phase 4 · **证伪**（检查 B0395 承诺的「如实说明『已达上限』」是否真有落点）

## 跑的命令（全部只读）
```
grep -rn "已达上限|上限" lib/ --include=.dart | grep -v display_prefs.dart | head -6   # ⇒ **「已达上限」零命中**
grep -nE "kStagePlaylistMaxChars|kStagePlaylistMaxItems" lib/ -r --include=.dart | head -6
sed -n '344,362p' lib/app/shell_prefs.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**零命中照例不是结论**（换问法：裁剪处有没有说话 ⇒ 有）（0 条新发现）
```dart
if (!result.added) {
  _setImageMessage(forShell: false, text: switch (result.reason) {        // :352-354
    '**empty**'          => '还没有舞台背景图——**先「选择背景图」，再把它加入轮播**',
    '**item_too_large**' => '这张图太大了，不能进轮播列表；**换一张小一点的图**',
    '**limit_reached**'  => '轮播列表已到上限 **$kStagePlaylistMaxItems 张**——**先「清空轮播」再重新加入**',
    '**budget_exceeded**'=> '轮播列表的总长度已达上限（本机存储放不下更多）——**先「清空轮播」再重新加入**',
    _                    => '这张图**没能**加入轮播列表',
  });
}
```
六个可核点：
1. ⭐⭐⭐ **B0395 承诺的「如实说明『已达上限』」真的存在**，且**带上了具体数字**（`$kStagePlaylistMaxItems 张`）
   ⇒ ⇒ **不是「已达上限」四个字，是「已到上限 16 张」**
2. ⭐⭐⭐ **五个分支 = 四个具名 reason + 一个 `_` 兜底** ⇒⇒ **同 B0335「穷举 switch **无** `_`」的对照** ——
   **这里有 `_`** ⇒⇒ **而 `_` 的内容是诚实的**（「这张图**没能**加入」——**不假装知道原因**）
3. ⭐⭐⭐ **四个具名分支每一个都带一个动作**，且**动作名是界面上按钮的原名**（用「」引起来）
   ⇒⇒⇒ **P31 第四次在 UI 侧** ⇒⇒⇒ **照着念就能做**
4. ⭐⭐ **`'empty'` 那条也在** ⇒⇒ **「还没加过就加」也是一种失败**，而它也被逐字说明
5. ⭐⭐ **三种「满」被分开**：`item_too_large`（这张图太大）· `limit_reached`（**张数**满）·
   `budget_exceeded`（**总长**满）⇒⇒ **三件不同的事、三种不同处置**（换图 / 清空 / 清空）
6. ⭐ `_setImageMessage(forShell: **false**, …)` ⇒⇒ **这条消息属于舞台那一侧**
   ⇒⇒ **同 B0394 的「按来源互斥」** ⇒⇒ **两个来源的消息各走各的**

⇒ ⇒⭐ **而本批最值钱的是第 1 点**：
> **B0395 承诺的「如实说明『已达上限』」不是一个词，是一个分支。**
> ⇒⇒ **「承诺是否兑现」的检查必须问到「兑现成什么形状」** ——
> **兑现成一个词，与兑现成「数字 + 动作 + 三种原因分开」，是两件事** ⇒⇒ **后者才叫兑现**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `appendToStagePlaylist` 的**本体**（它怎么算出 `reason` 四态 · `display_prefs.dart:1076-1110` **未读**）
2. `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 · `UiSignals` 定义本体 ·
   `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 ·
   `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义 · `PersonaCardFilePicker` typedef 注释
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~420）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
