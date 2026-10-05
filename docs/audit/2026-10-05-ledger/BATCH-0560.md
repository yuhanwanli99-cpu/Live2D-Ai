# BATCH-0560 落盘（极简）· ModSessionPrompts 真实现

## 结构（:264-266）
:264 /// 注入给 Mod 的会话能力句柄（**薄包装**，便于 ModServices 克隆）。
:265 #[derive(Clone)]
:266 pub struct ModSessionPrompts {
:267     inner: Arc<dyn SessionPromptSink>,
:268 }
⇒ 唯一字段是 `Arc<dyn SessionPromptSink>`，**没有任何缓存、没有额外状态**。

## 三个构造/查询
:272 /// 用宿主实现构造。
:273 pub fn new(sink: impl SessionPromptSink + 'static) -> Self
:279 /// 空实现（默认）——见 [NoSessionPrompts]。
:280 pub fn disabled() -> Self
:286 /// 见 [SessionPromptSink::enabled]。
:287 pub fn enabled(&self) -> bool
:290 /// 见 [SessionPromptSink::get]。
:291 pub fn get(&self, session: &str) -> Option<String>

## 默认实现侧（B0558 补齐的另一半）
:258 fn active(&self) -> Option<String> { None }
:260 fn set_active(&self, _session: Option<&str>) {}
⇒ 默认实现里 `active` 给 None、`set_active` 什么也不做 ⇒ 缺省即「不绑定会话」。

## 三个可核点
1. **`Arc<dyn Trait>` + `#[derive(Clone)]` 是一对** ⇒ 克隆是**廉价共享**、不是复制；
   而 B0554 核的 `ModRuntime: Send` 保证这个句柄进 Mod 也没问题。
   ⇒ 与 B0489 核的 `ModPanelContext` / B0543 核的 `ModServices` **同一手法**。
2. **每个方法的头注都只写「见 [SessionPromptSink::xxx]」** ⇒ 判据在 trait 上、
   薄包装**不复制一份说明** ⇒ 与 B0500「缺省写在参数表里」**同一条纪律的文档侧**。
3. **`disabled()` 与 `NoSessionPrompts` 配对**：头注直说「空实现（默认）——见
   [NoSessionPrompts]」⇒ 「默认不可用」这条纪律**在类型层有名字**、不是一个裸字段。

## 未核
session.rs 余下 180 余行 · error.rs 本体 · settings.rs 本体 ·
mod-system 的 tests/ 四行 · 其余 9 个 mod crate 本体 · shared/ 其余 8 个 json
