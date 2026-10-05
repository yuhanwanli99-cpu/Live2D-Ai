# BATCH-0632c 落盘（极简）· ⚠⚠⚠⚠ `stripCommentsAndStrings` 有 **8 份独立定义** + 3 处仍 import 共享那一份

## 编号
`BATCH-0632.md` 与 `BATCH-0632b.md` 均已存在 ⇒ 本批落 `BATCH-0632c.md`。

## 命令（只读）
```
ls AUDIT-REPO/BATCH-0632.md / BATCH-0632b.md
grep -rn "stripCommentsAndStrings" shell/flutter/test/
grep -rn "^String stripCommentsAndStrings" shell/flutter/test/
```

## 8 份独立定义（各自 `^String` 开头）
```
asset_guard_preset_dispatch_test.dart:79
transient_results_test.dart:33
no_client_side_preset_ttl_prediction_test.dart:31
asset_guard_stage_attach_resend_test.dart:36
glass_rim_test.dart:26
design_tokens_test.dart:18
asset_guard_director_cue_test.dart:40
motion_wiring_test.dart:30
```
## 3 处仍 `import` 共享的那一份
```
design_tokens_lint_test.dart:5              import 'design_tokens_test.dart' show stripCommentsAndStrings;
no_backdrop_filter_test.dart:5             import 'design_tokens_test.dart' show …
background_hydration_window_test.dart:37   import 'design_tok…'
```
## 调用密度（部分）
```
glass_rim_test.dart            :77 :87 :101 :360      （4 处）
asset_guard_director_cue_test  :94 :125 :130 :174    （4 处）
design_tokens_test.dart        :100                   （1 处，被外部 import 复用）
```

## 四个可核点
1. **⚠⚠⚠⚠ ⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒ 本批核心：一个「扫源码做门禁」的关键工具
   被**复制了 8 份**，而**另有 3 个文件选择 `import` 共享的那一份**
   ⇒⇒ **⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒ 两种取法同时存在** ⇒⇒
   **⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒ 判据（本批最值钱，可复用）：
   **「同一工具的 N 份拷贝」与「共享它」是**两种不同的失败**，前者更隐蔽** ⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒ 隐蔽在哪**：
   ⇒⇒ ① **拷贝的演化互不可见**（改一份不会让其它 7 份失败）
   ⇒⇒ ② **共享的那份改了会**让 3 个 importer 同时失败**（反而是**可见**的）
   ⇒⇒ ③ **⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒ 8 份拷贝**互相不知情** ⇒⇒
   **⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒ 判据（第二句，可直接用）：
   「拷贝」型重复的**唯一守卫是全部拷贝都加一条「与其它 N 份同步」的注释，
   而这恰恰是拷贝**没有**的东西** ⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒ 严重度候选 P2**（本批**不立** ——
   需先核 8 份**是否真的不一致**；若完全一致，那是 P3「维护性」）
2. **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ `glass_rim_test.dart` **自己有一份**（`:26`）**却仍有 4 处调用自己那份**
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 这排除了「它是死代码、等着被 import 替代」的解释**
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 判据（细节）：
   一个函数既被本文件用、又被别的文件 import 时，两类调用方对它的**期望可能不同** ——
   ⇒⇒ `glass_rim_test` 只需要「有没有某段代码」，
   `design_tokens_lint_test` 需要「这段代码里有没有裸色值」（**必须去掉字符串**）
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 这可能正是 8 份并存的原因：
   早期各测试各写各的，后来新增的门禁改成了 import 共享版，老文件没动** ⇒⇒
   **⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 判据（可复用）：
   **看到「N 份拷贝 + 少数 import」时，先问「新增的那些是否因为**新需求**才统一」** ——
   ⇒⇒ **统一的方向是「新的 import 共享、老的保留拷贝」时，
   就不是遗漏而是迁移未完成** ⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 迁移未完成的成本是：
   3 个文件已迁、5 个没迁，而「该迁」这个决定只存在于某个人的记忆里**
3. **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ 而这些文件**全都是 `*_test.dart`**，且**测试目录里没有行数门禁**
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒ 后果：
   **复制一个 40 行的工具函数不触发任何检查** ⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ 判据（可复用）：
   **「为什么这件事反复发生」有时答案不在纪律里，而在**没有门禁**的地方**
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ 承 B0617b 第 ① 点
   「一个反模式被禁几次 = 它有多容易踩」的同族问法：
   **「一个习惯被重复几次 = 它有多容易长成」**
4. **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ 而本审计正在核的「裸间距规则」正落在这套工具上**
   （B0630b 核的 `design_tokens_lint_test.dart` 是 3 个 importer 之一）
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒ ⇒ 也就是说：
   **「令牌有没有被引用」这个本审计反复引用的判据，其可信度依赖 11 处
   （8 定义 + 3 import）的解析行为是否一致** ⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒ ⇒ 下一批第一件事：diff 那 8 份**

## 未核
8 份实现是否一致（下一批 diff）· 「逐个点名」的时长豁免文件名单
· 8 份里有没有一份带测试（B0631b 的未核）· `refs` 统计会不会把 `'S'` 误计
