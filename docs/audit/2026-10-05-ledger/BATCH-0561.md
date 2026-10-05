# BATCH-0561 落盘（极简）· session.rs 尾部（薄包装方法 + 测试）=> mod-system 本体结项

## 薄包装的其余 7 个方法（:301-322）
全部**一个头注一行**「见 [SessionPromptSink::xxx]」，零重复说明：
clear_owner / contributions / set / clear / clear_all / sessions / (前批的 enabled,get)
=> **10 个方法 = 1 个 trait + 1 个 Arc<dyn> + 1 句「见 trait」**

## 测试（:349-476）
:350 #[test] fn sanitize_accepts_the_frontend_id_shape()
  ⇒ 正向：断言**前端传来的 id 形状**被接受（跨语言契约：B0411 规则那一对）
:363 #[test] fn sanitize_rejects_empty_overlong_and_path_like_ids()
:364   assert_eq!(sanitize_session_id(""), None);
:365   assert_eq!(sanitize_session_id("   "), None);          // 纯空白也拒
:366-368 "a".repeat(MAX_SESSION_ID_CHARS + 1) -> None        // 超长拒
:371   // **路径穿越 / 分隔符一律拒——它们将来会变成文件名。**
:372   for bad in ["a/b", "../x", "a b", "会话", "a.b/../c"] {
:373     assert_eq!(sanitize_session_id(bad), None, "{bad:?} 必须被拒");
:375   assert!(sanitize_session_id(&"a".repeat(MAX_SESSION_ID_CHARS)).is_some());
尾部还有一条 `sink.contributions("A")` 的断言（尾部 6 行）

## 三个可核点
1. **负例表是列出来的**（`a/b` / `../x` / `a b` / `会话` / `a.b/../c` 五项）
   ⇒ 与 B0443 核的「**把奇怪输入列出来，而不是让它们落到未定义行为**」**同一手法**。
   **注意第三项 `a b`（含空格）**：空格的拒绝理由是**会被当成分隔/路径**、不是「URL 不合法」
   ⇒⇒⇒⇒⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒
   ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒
   ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
   ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
   ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
   ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
   ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
   ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
2. **边界两侧都断言**：N 拒、N-1 收（`MAX_SESSION_ID_CHARS` 那两行）
   ⇒ 与 B0623 核的「严」**互补**：只测 `N+1` 会漏掉 off-by-one。
3. **断言消息带被拒的值**（`"{bad:?} 必须被拒"`）⇒ 失败时不用重跑就知道是哪一个
   ⇒ 与 B0500「一条禁令要带后果」**同一纪律的测试侧**。

## mod-system 本体结项
7 个源文件全部有头注/逐行判据，全部 ≤500 行，最大 session.rs 476。
未核的只剩 error.rs 本体 + settings.rs 本体 + tests/ 四行。
