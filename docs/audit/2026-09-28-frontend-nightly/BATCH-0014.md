# BATCH-0014 — Phase 2 · 横扫 B：dispose / 订阅 / 监听对称性

> 账本：`AUDIT-B/`。**本批无新发现**（按 §9 反空转规则，如实记录「无新发现」并换轴）。

## 盘点结果（全仓逐项对账）

| 资源类型 | 构造点 | 释放点 | 判定 |
|---|---|---|---|
| `Timer.periodic` | 5 处（`audio_player:319` / `shell_slideshow:73` / `dev_tools_section:1590,1607`） | 全部在 dispose/停止路径取消（`_stopTicker` / `dispose` / 三个 `_*Timer?.cancel()`） | **全对称 ✓** |
| `StreamSubscription` | `main.dart` 5 个（levels / sentenceStarts / stageClock / statuses / events）+ `chat_controller` 2 个 + `live2d_stage` 1 个 + `live2d_host_web` 1 个 | 全部 `unawaited(x?.cancel())` | **全对称 ✓** |
| `AnimationController` | 5 处（`state_pill` / `streaming_indicator` / `soft_motion.StartupReveal` / `page_cross_fade` / `live2d_stage`） | 各自 dispose | **全对称 ✓** |
| `addListener` | 6 处有效站点 | 5 处 `removeListener` + 1 处是**给自己的 AnimationController**加的（`live2d_stage:250`），随 controller dispose 自动消失 | **全对称 ✓** |
| `TextEditingController` | 13 处 | 13 处一一对应（含 `persona_panel:172` / `env_key_field:57` / `dev_tools_section:281` 这三处变量名不含 "controller" 的） | **全对称 ✓** |
| blob URL / DOM 元素 | `audio_player._create` | `_release`（先 `pause`+`remove`，再 `revokeObjectURL`；顺序注释写明「revoke 在播放结束之后」）+ catch 里补 revoke | **全对称 ✓** |
| 平台视图 / window 监听 | `live2d_host_web` 的 `window.addEventListener('message')` | `dispose` 里 `removeEventListener` | **全对称 ✓** |
| 防抖器 | `settings_models.ModelOverrideCoalescer` / `action_scales_sync` | 各有 `dispose()`，且 main 在 dispose 里**先于** API 客户端关闭调用（注释解释了为什么 flush 是错的） | **全对称 ✓** |

## 本批核对过、确认**正确**的三处顺序（这类顺序错了不报错、只是行为诡异）
1. **「先置意图标志、再停识别器」**：`voice_listen_controller.stop()` 在 `await _recognizer.stop()` **之前**置 `_continuous=false`；`suspendForPlayback` 先置 `_suspended`；`pressStart` 先置 `_ptt`。浏览器在 `abort()` 后**必定**回调 `onend`，若顺序反了，`_onEnd` 会因为「意图标志还是 true」而**把常驻听重新拉起来**——三处都成立，所以这条不会发生。
2. **`_scheduledReconnect` 换 socket**：`_detach` 先摘掉全部回调再 `close()`，所以旧 socket 的迟到 `onclose` 不会驱动状态。
3. **`UiStateTracker._disposed`**：Timer 回调里先查 `_disposed` 再 `notifyListeners`（注释写明「在已 dispose 的 ChangeNotifier 上 notify 会断言失败，生产里表现为『停止对话后偶发崩溃』」）——这正是「cancel() 拦不住已排队的回调」的正解。

## 无新发现的说明
按 §9 反空转规则，本批不重复同一范围、不硬凑条目；下一批换轴（Phase 2 · C：竞态与取消纪律）。**「dispose 对称」这一轴的覆盖率是 100% 逐项人工对账**（不是模式扫描），结论可复用：后续改动若新增资源持有者，应按本批的表逐行补一行。
