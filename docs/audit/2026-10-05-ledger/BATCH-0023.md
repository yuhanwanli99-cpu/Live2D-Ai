# BATCH-0023 · **前端首批** —— 平台视图指针（红线 M）+ 高频通道（红线 O）

Phase 1 · 域覆盖 → **转段：前端层**（`shell/flutter/**`，217 文件，此前 0 已审）

## 读过的文件（全部来自 `git ls-files` + `wc -l`）
1. `shell/flutter/lib/live2d/stage_pointer_interceptor.dart` — 99（**全读**）
2. `shell/flutter/test/stage_pointer_interceptor_test.dart` — 172（**全读**）
3. `shell/flutter/lib/app/app_shell.dart` — （片段：528-537 / 718-757 / 839-848 四处接线）
4. `shell/flutter/lib/audio/audio_player.dart` — （片段：77-95 / 423-435）
5. `shell/flutter/lib/live2d/live2d_bridge.dart` — （片段：42 / 72 / 90-91 / 131 / 200-213）
6. `shell/flutter/lib/ui/stage_host.dart` — （片段：6 / 92-96）
7. `shell/flutter/lib/live2d/live2d_stage.dart` — （片段：179 / 536）

## 跑过的命令（全部只读）
```
git ls-files 'shell/flutter/lib/live2d' 'shell/flutter/lib/api' | xargs wc -l | sort -rn
cat shell/flutter/test/stage_pointer_interceptor_test.dart
grep -rn "StagePointerInterceptor(" shell/flutter/lib/
grep -rn -A 3 "StagePointerInterceptor(" shell/flutter/lib/ | grep -c "enabled"
grep -rn "CollapsiblePanel" shell/flutter/lib/
grep -rln "StagePointerInterceptor" shell/flutter/test/
grep -n "enabled" shell/flutter/test/stage_pointer_interceptor_test.dart      # → 无输出
grep -rn "Duration(milliseconds: 30|Ticker|addListener" shell/flutter/lib/audio/audio_player.dart
grep -rn "GlobalKey" shell/flutter/lib/live2d/*.dart shell/flutter/lib/ui/stage_host.dart
grep -rn "mouth|viseme|lip" shell/flutter/lib/live2d/live2d_bridge.dart
```
未跑任何 cargo / flutter / trunk / pnpm 命令（**含 flutter test / flutter analyze**）。

## 维度覆盖（本批）
- 维度 J（关键路径测试缺口）：`StagePointerInterceptor` 的**易**路径有 8 个真回归、
  **难**路径（`enabled`）零覆盖 → F-0023-01
- 维度 D（重渲染）：30Hz 通道确认**不经过 setState**（GlobalKey + postMessage）✔
- 维度 B（对称释放）：`audio_player` 的 `_stopTicker()` 在 :242/:388/:430/:435 四处被调，
  队列空即停 ✔（未逐行核实所有路径）
- 模式 D 第 8 例：`stage_pointer_interceptor.dart:45` 写「把已知的**四处**钉住了」，
  而测试文件实有 **8** 个用例（含三种宿主 × 四类覆盖层）。注释的数字过期。

## 未核实项
1. **模式 B（假绿灯）在 102 个 Dart 测试文件里的复查尚未开始**——本批只读了 1 个测试文件。
   这是 Phase 2 维度 J 的主战场，**下一批起排**。
2. `live2d_stage.dart`(666) / `live2d_bridge.dart`(547) / `render_events.dart`(700)
   **未逐行读**——红线 N（舞台保活：iframe 不得离开 Widget 树）需要在 `live2d_stage.dart` 里核实。
3. 红线 K/L（离线优先 / 中文字体子集）本批未查。
4. 红线 O 只核了常量与链路面，**未逐行**验证三处实现无 `setState`。
5. `app_shell.dart`(已知 >1000 行的结构债之一) 只读了四处接线片段。

## 本批新增
P0 0 · P1 0 · **P2 1** · P3 0 ｜ 另：**红线 M / O 验证通过**（M 有真回归，O 架构成立）
