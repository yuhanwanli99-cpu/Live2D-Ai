# BATCH-0563 落盘（极简）· error.rs 六个变体 => mod-system 本体全核

## error.rs（65 行）
:1 //! Mod 错误类型。
:6 /// Mod 系统错误。
:7 #[derive(Debug, Clone, PartialEq, Eq)]
:8 pub enum ModError {
  六个变体：
:9  IncompatibleApi { mod_id, got: u32, expected: u32 }   /// Mod 声明了不兼容的 API 版本
:15 Init { mod_id, message }                              /// Mod 初始化/注册失败
:17 Event { mod_id, message }                             /// Mod 事件处理失败
:19 InvalidSettings { mod_id, message }                   /// 设置 schema 非法
:24 UnsupportedCommand { command: String }                /// Mod 不认识 host 发来的一次性命令
:26 Other(String)                                         /// 其它

## 三个可核点
1. **五个带 `mod_id` 字段的变体 + 一个只带 `command` 的 + 一个 `Other(String)`**
   ⇒⇒ **⇒ 「谁出的错」在前五个变体里是**结构化字段**，不是拼进 message 的字符串
   ⇒⇒ 与 B0651 核的 api_version 拒绝路径**同一纪律**（那里 message 也带两个数）。
2. **:24 的头注写明了「为什么单独一个变体」**：
   「与 ModError::Other **分开**是为了让 host 能回一个**可处置**的码
    `unsupported_command`（**409**）而不是笼统的「Mod 错误」。」
   ⇒⇒ **⇒ 变体的存在理由是「宿主要不要能回一个可处置的码」**
   ⇒⇒ 与 B0560 核的「这 10 个方法都是 1 句『见 trait』」**互补**：
   **那边是「不复制说明」，这边是「不合并后果」。**
3. **`IncompatibleApi` 单独存 `got` 与 `expected` 两个 u32**（不是拼进 message）
   ⇒⇒ 而 :310 那条 `format!` 才把它们变成文案 ⇒⇒ **结构与呈现分离**。

## mod-system 结项
7 个源文件全部核完：lib 55 / status 55 / registry 25 / descriptor 36 / factory 142 /
settings 155（未核本体）/ topics 164 / services 279 / session 476 / error 65。
全部 ≤500；全部有头注；判据类函数（owner_merge_rank / is_enabled / as_str /
sanitize_session_id / compose_session_prompt）**都带用途注释**。
剩余未核：settings.rs 本体 · mod-system 的 tests/ 四行。
