# BATCH-0630 落盘

## 跑的命令（全部只读）
```
grep -n "actions|primary" lib/ui/inline_notice.dart | head -8
grep -rn "primary: true" lib/ --include=*.dart | wc -l
```

## 逐行
```
:51    this.primary = false,
:58   final bool primary;
:66    this.actions = const <NoticeAction>[],
:74   final List<NoticeAction> actions;
:148            if (actions.isNotEmpty)
:154                    for (final NoticeAction action in actions)
:155                      action.primary
```
`grep -rn "primary: true" lib/` ⇒ **0**

## 三个可核点
1. 渲染端是 `for … action.primary`
   => 判据读的是**每个动作自己的标记**，而**没有**「数一数有几个 true」的断言
   => 与 B0497「三行同形、靠复制成立」同族
2. ⇒ 全 `lib/` 零处 `primary: true`
   => 「一条提示里最多一个」这个约束，今天**以「零个」的方式满足**
   => 也就是说：**约束写在头注里、而实际上一个主动作都没用到**
   => 记为观察（修法二选一，各一处：把 `primary` 用起来（挑一处该有主动作的），
   或**把「最多一个」这句删掉**——按 B0403「不可替代 ⇔ 必写」，一个没人用的字段
   与一条没人用的约束**都属于「可删」**）
3. ⇒ 而 `actions` 的缺省是 `const <NoticeAction>[]`（`:66`）
   => 与 B0583 的 `dark: true`、B0584 的 `restarted = false`、B0629 的 `primary = false`
   **同族**：**缺省值写在参数表里、不靠调用方记得传**

## 未核
severity 的底色/描边分派处 · inline_notice.dart:15-29 · EmphasizedText 解析规则 ·
模型库分区 body 其余部分 · feature.yml · PULL_REQUEST_TEMPLATE.md
