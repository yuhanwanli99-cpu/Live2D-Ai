# BATCH-0498 · 三个小节；「没有回显」被拆成三个否定

Phase 1 · 域覆盖 · 前端 .dart —— env_key_field.dart（llm_section 头注点名、本体首次读）

## 跑的命令（全部只读）
```
sed -n '1,12p' lib/settings/sections/env_key_field.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 三个可核点
1. 头注是三个小节，各答一问：它为什么存在 / 与其它字段的区别（安全边界）/ 怎么做。
   - 「rc.2 之前密钥只能在后端进程环境里：界面能改模型名、却改不了 key ⇒ 改完配置还是 401，
     且没有任何地方能改 —— 这是「改不动」的那一个」
2. 「与其它字段的区别（**安全边界**）」这个标题承认了它不是普通字段。
3. 「**没有回显**：本控件**不持有**、**不请求**、**不显示**当前值」—— 三个否定；
   另有「值**只往上走**（PUT /api/v1/env，响应只回 {key, set}）」「obscureText +
   每次保存后立即清空」。

## 可提炼
一个字段的「不泄露」被拆成三个否定（不持有 / 不请求 / 不显示），
而「值只往上走」是**方向**的表述（不是「不返回」）。

## 未核实项
1. env_key_field 的本体（清除时机、失败路径）· wake_gate_open · clean_transcript 尾部 ·
   lower_eq / is_separator
2. apply_bridge_effects 调用顺序（B0279）· stage-bg.dataUrl 发送方（B0277）· main.rs 余约 930 行 ·
   面板余面（覆盖行数是移动靶）
3. 永久不可核 4 条（reqwest timeout / 动作计划 token · _finishTurn 幂等 · GitHub secret scanning）
