# BATCH-0567 落盘（无装饰）· 核「导演的动作预设」归属 => 属于 director，反证不成立

## 跑的命令（只读）
grep -rn "动作预设" --include=*.md --include=*.rs --include=*.dart .
grep -rn "smile" crates/live2d-ai-mod-director/src/*.rs

## 结果
1. 命中 crates/live2d-ai-mod-director/src/lib.rs:1 与 :182：
   lib.rs:1-5 头注：「把每一轮的输入正文经纯函数推导成
     {emotion, intent, suggested_tts:{speed,pitch}, preset_id}，写日志 + state_json
     其中 **preset_id 就是本轮要演的动作预设**（表情 / 短动作）。」
   lib.rs:182 「**动作预设表**（emotion|intent -> preset_id，2026-09-15）。
     pub presets: PresetTable」
2. smile 出现在 director 自己的 presets.rs：
   presets.rs:6   表情包（expression）：smile
   presets.rs:34  "smile",  （预设名清单里的一项）
   presets.rs:97  | happy | smile | 表情 |
   presets.rs:108 happy: "smile".to_string()

## 结论
「动作预设」「smile」「缺省不是 none」这三项**全部属于 director Mod**。
=> F-0637-01 补充四**成立**：settings.rs:45 的头注确实指向一个真实存在、
   且**被 AGENTS 声称已删除**的 crate 的活字段。
=> 反证（写的是「若属另一个未实现的设计则本条不成立」）**不成立** -> 该条保留 P1。

## 另有一处需要记账的事实
lib.rs:7-8：「驱动舞台的唯一通道是 ModServices.cues -> host 广播 WS action_cue」，
「只读状态面 latest.preset_id 仅供 direct... **前端拉取它驱动舞台的那条通道已退役**（阶段3 / D12）」
而 presets.rs 的日期是 2026-09-15 —— **比 AGENTS 说的「0.2.0-rc.1 已删除」还晚**。
lib.rs:1 的日期是 2026-09-14（与注册表第 5 位的注释同一天）。
=> 六处证据的日期全部 >= 2026-09-14，没有一处支持「已于 rc.2（2026-09-12）删除」。

## 未核
presets.rs 其余内容 · director 的 arbiter.rs / decision.rs / ledger.rs ·
mod-system 的 tests/ 四行 · 其余 8 个 mod crate 本体 · shared/ 其余 8 个 json
