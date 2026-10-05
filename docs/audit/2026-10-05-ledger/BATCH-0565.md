# BATCH-0565 落盘（无装饰）· settings.rs

## 头注 settings.rs:1-3
//! Mod 设置数据协议（纯数据 schema，前端统一渲染）。
//! 禁止 Mod 注入 HTML/JS/DOM —— 前端通过 fields 生成控件。

## 类型
settings.rs:7   pub struct SelectOption { value: String, label: String }
settings.rs:14  pub enum ModSettingField { Bool / String / Number / Select }
settings.rs:26  String.secret: bool  (前端渲染为 password 输入；内容不落日志)
settings.rs:29  String.default: Option<String>
settings.rs:40  Number { key, label, min: f64, max: f64 }
settings.rs:40  Select { key, label, options, default: Option<String> }
settings.rs:65  pub struct ModSettingsSpec { mod_id, title, version: u32, fields }

## String.default 头注原文要点
缺省值（可空）：config 里没有这个键时，前端表单回填它，保存时按它写入。
避免两件事：(1) 面板显示空 / 服务端却按缺省走；(2) 点一次保存就把缺省语义写成显式空值。
为什么是可选：绝大多数 String 字段的缺省语义是空（宿主给默认语言、回落 zh-CN、
sidecar 路径由 host 推导），不需要写进协议；只有少数（语音唤醒词）必须让用户看见并能改。

## Select.default 头注原文要点
用于「服务端缺省不是第一个选项」的场景 —— 举的例子是导演的动作预设（缺省是 smile，不是 none）。

## 三个可核点
1. 两个 default 头注都写了「防什么」：String 防显示与服务端分裂、防保存覆盖缺省；
   Select 防缺省落在第一项。与 B0500「一条禁令要带后果」同族。
2. default 用 Option 而不是裸 String，且头注说清为什么大多数字段不需要它。
   与 B0629 的 primary: bool 缺省同向。
3. 重要：Select.default 举的例子是「导演的动作预设」，而 director 正是 F-0637-01
   里 AGENTS 声称「已于 0.1.0-rc.2 删除」的那个 crate。
   即：mod-system 的数据协议里仍留着 director 的字段用例，与 AGENTS 的「已删除」矛盾。
   这是 F-0637-01 的第 5 处反证（前三处：workspace members / desktop 依赖 / 五源文件 /
   注册表第 5 位）。仍记为该条的一部分，不另立新发现。

## mod-system 本体完全结项
lib 55 / status 55 / registry 25 / descriptor 36 / factory 142 / settings 155 /
topics 164 / services 279 / session 476 / error 65 —— 全部 <=500，全部有头注。
判据类函数 owner_merge_rank / is_enabled / as_str / sanitize_session_id /
compose_session_prompt 全部带用途注释。

## 未核
mod-system 的 tests/ 四行 · 其余 9 个 mod crate 本体 · shared/ 其余 8 个 json
