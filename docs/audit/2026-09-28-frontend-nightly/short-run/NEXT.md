# NEXT

## 当前：BATCH-0005（进行中）
- Phase 1 队列位置：5/11
- 主题：lib/ui/ 上半 —— 聊天与消息呈现面
  - 候选文件（先 wc -l 定边界）：chat_panel.dart · message_bubble.dart · chat_composer.dart · session_sheet.dart · theme.dart（buildAppComponents）· error_banner.dart · compact_settings_page.dart · stage_corner_controls.dart · connection_pill.dart
- 维度重点：
  - C 三态：chat_panel 的加载/空/错误三态是否齐全；流式光标（自绘竖条，不引字形）是否仍在
  - D 重渲染：消息列表是否 builder 化、气泡 build 内是否新建对象（TextStyle/Padding 每条每帧）
  - E 契约：气泡消费的 WS 字段（text_delta / reasoning_delta / text_fallback / audio）是否与 ws_frame.dart 一致；`ignore:` 强转清单
  - F 无障碍：气泡 Semantics 标签、发送/静音按钮 aria、对比度
  - J 测试缺口：message_bubble/chat_panel 测试是否只测纯函数而漏 widget 交互
  - 交叉：F-0001-1（错误出路）宿主接线、session_sheet 的指针垫层（M 红线）
- 假绿灯：message_bubble_test / chat_panel_test / session_sheet_test 扫描 expect(常量,常量) 与 isNotNull 空转
- 已登记：紧凑设置页保活（B1 已证）、无

## 之后
- BATCH-0006：lib/ui/ 下半（shell_backdrop 补审 / audio_bar 补审 / field_row 补审 / 其余共享组件）+ lib/design/
- CONSOLIDATION-01 在 0005 关批时写
