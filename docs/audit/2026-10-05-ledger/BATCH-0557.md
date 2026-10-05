# BATCH-0557 落盘（极简）· session.rs 头注 + 两个纯函数

## 头注（:1-10）
//! 会话级作用域（**L1 产品级基座，2026-09-15**）。
//! L1 验收要求 **persona 绑定当前会话**（会话 A/B 不串卡、停用还原…）
//! memory 也要**尽量**按会话分桶。
//! 之前「人设」只有一处入口：live2d-ai.toml 的 [persona] system_prompt（**进程级、单例**）
//! persona Mod 只能整段覆写它 —— 那是「全局人设」，不是「会话人设」。
//! 本模块补的是**最小宿主能力**。

## 两个纯函数
- :96 `sanitize_session_id(raw: &str) -> Option<String>`
  头注：「**为什么不容忍其它字符**：会话 id 会被**用作文件名 / 存储键**
  （memory 的 `sessions/<id>.memory.jsonl`）。**等真的落盘了再补闸，
  就是在已经存在路径穿越风险的代码上事后打补丁。**」
- :115 `owner_merge_rank(owner: &str) -> (u8, &str)`
  头注：「组合顺序的排序键：**匿名槽最前，persona 次之，memory 再次，
  其余 owner 归为一组并按其字典序排列**」
  头注：「宿主实现与测试替身**都必须**用它，否则「宿主里一个顺序、
  测试里另一个顺序」会让组合结果变成**两份真相**。」

## 三个可核点
1. **`sanitize_session_id` 拒绝的**理由**是落盘后果**（文件名 / 存储键 + 路径穿越），
   而**不是**「输入不合法」这种空话 => 与 B0500「一条禁令要带后果」同族。
2. **`owner_merge_rank` 是一条**排序契约**、且要求**宿主与测试替身共用**
   => 判据收敛到一处（B0495 核过的那类）=> 这条**就是那个判据**。
3. **头注给了日期**（2026-09-15）=> 与 B0627/B0631 核的「设计点带日期」同族；
   而 `session.rs` 是全仓**最新的**基座（L1 比 rc.1 的 2026-09-14 还晚一天）。

## 未核
：:128 compose_session_prompt · :165 SessionPromptSink · :241 NoSessionPrompts ·
:266 ModSessionPrompts · session.rs 其余 300 余行 · error.rs · settings.rs 本体
