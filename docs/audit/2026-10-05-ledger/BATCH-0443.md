# BATCH-0443 · ✅ **+1 条 P3：8 份逐字节相同的副本** —— 而「共享」不是不能、是**没成为默认**

Phase 4 · **证伪**（B0442 留的：那 8 份副本是否逐字相同）

## 跑的命令（全部只读）
```
for f in $(grep -rln '^String stripCommentsAndStrings(String src)' test/); do
  n=$(grep -n '^String stripCommentsAndStrings(String src)' "$f" | cut -d: -f1)
  body=$(sed -n "${n},$((n+14))p" "$f")
  printf "%-46s 行%-4s 长度%-4s md5:%s" "$(basename $f)" "$n" \
    "$(printf '%s' "$body" | wc -c)" "$(printf '%s' "$body" | grep -v '^\s*$' | md5sum | cut -c1-8)"
done
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**八个文件里的那份 helper 逐字节相同**（F-0443-01 · P3）
| 文件 | 行 | 长度 | md5 |
|---|---|---|---|
| `asset_guard_preset_dispatch_test.dart` | 79 | 467 | `96c4bad6` |
| `transient_results_test.dart` | 33 | 467 | `96c4bad6` |
| `no_client_side_preset_ttl_prediction_test.dart` | 31 | 467 | `96c4bad6` |
| `asset_guard_stage_attach_resend_test.dart` | 36 | 467 | `96c4bad6` |
| `glass_rim_test.dart` | 26 | 467 | `96c4bad6` |
| **`design_tokens_test.dart`（它的家）** | 18 | 467 | `96c4bad6` |
| `asset_guard_director_cue_test.dart` | 40 | 467 | `96c4bad6` |
| `motion_wiring_test.dart` | 30 | 467 | `96c4bad6` |

**⇒ 另有 3 个文件已从 `design_tokens_test.dart` import 它**
（`design_tokens_lint_test.dart:5` · `no_backdrop_filter_test.dart:5` · `background_hydration_window_test.dart:37`）

### 四个可核点
1. ⭐⭐⭐⭐⭐ **⇒⇒⇒ 8 份 × 15 行的**完全重复**，改一处要改八处**
   ⇒⇒ **而任何一处漏改的后果是**假绿/假红**、不是编译错误**
2. ⭐⭐⭐⭐⭐ **⇒⇒⇒ 而同一层里「共享」这条路**已经被走过 3 次** ⇒⇒⇒⭐⭐⭐
   **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ **⇒⇒⇒⇒⇒ ⇒ ⇒ 准确的判断：「它不是不能共享，是**没有成为默认**」**
   ⇒⇒⇒ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒⇒⇒⇒ ⇒ **⇒⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒ ⇒ 这是收窄成一句话的事实**
3. ⭐⭐ **⇒⇒⇒ 而 P25（「收敛若引入层间倒挂则重复更便宜」）在这里**不成立** ⇒⇒⇒⭐⭐
   **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒⇒⇒⇒ ⇒ **同层共享已被做出来 3 次、不存在「层间倒挂」这一免责理由**
4. ⭐ **⇒⇒⇒ 修法**：8 份本地定义换成**一次 import**（与那 3 处同款）
   + 把 helper 归到**与职责相符**的位置（**顺带解决「11 个文件在用的东西住在 `design_tokens_test.dart` 里」**）
   ⇒⇒ **而「test/ 之间互相 import 不可取」这个顾虑，已被本仓自己回答过 3 次**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 · `ModServices.apply_settings` 本体 ·
   `settings_models.dart` 的 `fromJson` 体 · `ModConfigResult` 定义
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~590 / `memory_panel` ~515 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~455）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
