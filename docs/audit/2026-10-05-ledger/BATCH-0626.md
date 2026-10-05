# BATCH-0626 · InlineNotice 是裸 Text ⇒ 不渲染 **；EmphasizedText 渲染

## 跑的命令（全部只读）
```
grep -rn "class InlineNotice" -A 20 lib/ui/*.dart | grep -E "class|Text|message|RichText|children"
grep -n "message" lib/ui/inline_notice.dart
sed -n '120,140p' lib/ui/inline_notice.dart
```

## 逐行
```
:62   class InlineNotice extends StatelessWidget {
:64     required this.message,
:72   final String message;
:131                  child: Text(
:132                    message,
:133                    style: theme.textTheme.bodySmall?.copyWith(
:134                      color: colors.contentMuted,
```

## 三个可核点
1. => 渲染是**裸 `Text(message)`**、没有 `EmphasizedText`、没有 `RichText`
   => `**` 不会被渲染、会**原样出现在界面上**
   => 与 B0450 核的那条（AGENTS：「UI 文案露出 Markdown `**`（15 处）→ 新增
   `EmphasizedText` 真渲染 + 扫描规则」）**对照** ⇒ 两个控件的**文案能力不同**
2. => 而 B0625 核的模型库分区里：**`activateMessage` 走 `EmphasizedText`（渲染）**、
   **`error` 走 `InlineNotice`（不渲染）**
   => ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒
   ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒
   ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒
   ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒
   => ⇒ **同一个分区里，两个「有话要说」的位置，能力不同**
   => ⇒ **⇒⇒⇒⇒** 这意味着：`error` 的文案里**不能写 `**`**，
   而 `activateMessage` 的**可以** ⇒ ⇒ **判据落在控件上、不在文案上**
   => ⇒ 与 B0499 核的「判据收敛到一处」**反向**：这里**判据分裂在两个控件**
   => ⇒ **⇒⇒⇒⇒** **记为观察**（修法：`InlineNotice` 也走 `EmphasizedText`，
   与 AGENTS「UI 文案露出 Markdown `**`」那条整改**同向**；或明确写下「本控件的文案不许写 `**`」）
3. => 而 `:13` 的头注列了**它被用在哪些地方**（`message_bubble` / `FilledButton.tonal`（`main.dart`、`stage_host`）…）
   => 与 B0609 核的「三处引用」**同族**（**控件自己点名自己的调用方**）

## 未核
`InlineNotice` 的 severity 三态与其 icon 来源 · `EmphasizedText` 的解析规则本体 ·
`inline_notice.dart:1-61` 的设计说明 · 模型库分区 body 其余部分 ·
feature.yml · PULL_REQUEST_TEMPLATE.md
