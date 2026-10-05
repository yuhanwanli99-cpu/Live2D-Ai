# BATCH-0634 落盘 · B0633 那条「读屏会念两遍」不成立

## 跑的命令（全部只读）
```
sed -n '148,172p' lib/ui/inline_notice.dart | grep -nE "Button|Semantics|Exclude|action\."
```

## 逐行
```
:155  action.primary
:156      ? FilledButton.tonal(onPressed: action.onPressed, child: Text(action.label))
:160      : TextButton(     onPressed: action.onPressed, child: Text(action.label))
```

## 三个可核点
1. 两个分支都是 Flutter **原生按钮**、而原生按钮**自带语义节点**
   => **⇒ B0633 提的「读屏会念两遍」不成立**（不是缺陷）
   => 而真正的后果是「**按钮也在 live region 里**」
   => ⇒ 那恰恰是规矩第 3 条想要的：「提示出现时读屏要念出来」
      ⇒ 提示 + 它的动作**一起被播**，正是「live region」该有的行为
2. ⇒ 而「一个提示 = 消息 + 动作」这个组合**整体成为 live region**
   ⇒ 与 B0529 核的「先状态落定再对外说」同族
   （**先立住语义，再往里填内容**）
3. ⇒ 而「两分支只有 onPressed + label 不同」⇒ 与 B0497 核的
   「三行同形、靠复制成立」**同族、但这次是**刻意**的
   （**唯一的差别就是这个字段**，所以就该由这个字段分派）

## 结论
B0633 留的那条待核**结清，且是我把结论说反了**：
不是「按钮要不要 exclude」，而是「**按钮本来就该在 live region 里**」。
⇒ 可提炼：**问「会不会重复播」之前，先问「它应不应该被播」** ——
   后者的答案若是「应该」，那么前的担心就不成立。

## 未核
EmphasizedText 解析规则 · 模型库分区 body 其余部分 · feature.yml · PULL_REQUEST_TEMPLATE.md
