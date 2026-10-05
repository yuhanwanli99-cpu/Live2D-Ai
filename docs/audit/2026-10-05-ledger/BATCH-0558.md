# BATCH-0558 落盘（极简）· compose_session_prompt + SessionPromptSink + NoSessionPrompts

## compose_session_prompt（:128-145）
:128 pub fn compose_session_prompt<'a, I>(contributions: I) -> Option<String>
:130   I: IntoIterator<Item = (&'a str, &'a str)>,
:133   .filter(|(_, text)| !text.is_empty())          // 空文本先滤掉
:135   .map(|(owner, text)| (text, owner_merge_rank(owner)))   // B0557 那条排序契约
:138   if parts.is_empty() { return None; }           // 全空 -> None
:140   parts.sort_by(|a, b| a.1.cmp(&b.1));
:143   parts.iter().map(|(text, _)| *text).collect::<Vec<_>>()   // 按序换行连接
⇒ 三段缺省/兜底齐全：空文本滤掉 -> 全空 None（调用方回落全局）-> 否则按契约排序

## SessionPromptSink（:160-175）
头注把**六个方法分成两组**：
- **产品路径**：set_owned / clear_owned / clear_owner（persona / memory **各碰自己的槽**）
- **遗留 / 测试用**：set（写匿名槽）/ clear（清该会话所有槽）/ clear_all（清一切）
:165 pub trait SessionPromptSink: Send + Sync {
:169   fn enabled(&self) -> bool { true }            // 缺省 true
  头注：「宿主没有注入实现时用 NoSessionPrompts（返回 false）。Mod 据此
        决定要不要**退回全局写回** / 在 UI 上说「本环境不支持会话绑定」」

## NoSessionPrompts（:238-252）
头注：「与 ModSettingsApplier 的默认实现**同一条纪律**：
        **默认拒绝 / 默认不可用**，显式注入才开通。
        这样「**单测里静默命中一个不存在的会话表**」这种假通过不会发生。」
:241 pub struct NoSessionPrompts;
:245   fn enabled(&self) -> bool { false }
:248   fn get(&self, _session: &str) -> Option<String> { None }
:249   fn set(&self, _session: &str, _prompt: &str) {}
:251   fn clear(&self, _session: &str) -> bool { false }

## 三个可核点
1. **`enabled()` 是「能力探测」而不是「有没有数据」**——缺省 true、NoSessionPrompts 给 false，
   而 Mod **据此决定要不要退回全局写回** ⇒ 与 B0626 核的「观测值不是开关」**互补**：
   这里是**能力声明**、且**有默认实现**。
2. **六个方法被头注显式分成「产品路径 / 遗留·测试用」** ⇒ 与 B0443 核的「删除有两种」同族：
   这一次是**标记**而不是删除，但同样回答了「谁还在用哪一条」。
3. **NoSessionPrompts 的理由是「防假通过」**（不是「没有会话表」）
   ⇒ 与 B0448 核的「测试结构上不能失败」**正面对照**：
   **它防的正是「单测里静默命中一个不存在的会话表」那种假通过。**

## 未核
：:266 ModSessionPrompts（真实现）· session.rs 其余 250 余行 · error.rs · settings.rs 本体
