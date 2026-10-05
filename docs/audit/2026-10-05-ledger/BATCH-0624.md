# BATCH-0624 落盘（极简）· ⭐ registry 的消费方式超出预期：测试名直接写「新增档位必须显式改测试」

## 编号
`BATCH-0624.md` **不存在**（首次占用）⇒ 直接落盘。

## 命令（只读）
```
ls AUDIT-REPO/BATCH-0624.md
grep -rn "AppDurations.registry" shell/flutter/ --include=*.dart
sed -n '222,232p;242,252p' test/design_tokens_test.dart
```

## 消费点
```
test/design_tokens_test.dart:139   'AppDurations': AppDuration…（某张表里的一行）
test/design_tokens_test.dart:226   expect(AppDurations.registry.length, 4);
test/design_tokens_test.dart:245   遍历四个 registry 做同值检查
```
⇒⇒⇒⇒ **「只服务测试」经核成立** —— 生产代码里 `registry` 零引用

## 逐字（`design_tokens_test.dart:222-251`）
```
:222 group('声明侧自洽', () {
:223   test('每个家族的登记表长度与预期一致（**新增档位必须显式改测**…
:224     expect(AppRadius.registry.length, 7);
:225     expect(Space.registry.length, 9);
:226     expect(AppDurations.registry.length, 4);
:227     expect(AppRhythms.registry.length, 3);
:228     expect(Motion.names.length, 2);
:229     expect(AppFontSizes.registry.length, 8);
:230     expect(Breakpoints.registry.length, 2);
:231     // 四套配色（2026-09-11）。`AppPalette` 不再是「平的令牌表」，
:232     // 所以它退出了 kTokenFamilies —— 它的结构由下面「配色族」…
:242   for (final Map<String, Object> reg in <Map<String, Object>>[
:243     AppRadius.registry, Space.registry, AppDurations.registry, AppRhythms.registry,
:244   ]) {
:245     final List<Object> values = reg.values.toList();
:246     expect(values.toSet().length, values.length, reason: '**存在同值档位**');
:247   }
```

## 四个可核点
1. **⭐⭐⭐⭐⭐ ⇒⇒⇒⇒ `:223` 的测试名把「登记表」的**用意**写成了半句话**
   「新增档位**必须显式改测试**」⇒⇒
   **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 判据（本批最值钱）：
   `registry` 的价值不是「方便遍历」，而是**让新增变成一次显式决定**——
   遍历会自动覆盖新项，但**长度断言会让新项立刻把测试变红**
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ 换句话说：这张表把
   「我加了一个新档」从**静默的扩充**变成**必须承认的改动**
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒⇒⇒ ⇒ 判据（可提炼）：
   **一张好使的不变式测试要同时具备两个方向** ——
   ① 遍历（新项自动进断言）② 计数（新项让测试变红）**。
   只做 ① 是「宽松通过」，只做 ② 是「逐条点名」，本仓两条都有。**
2. **⭐⭐⭐⭐⭐ ⇒⇒⇒⇒ `:242-247` 遍历四个 registry 断言「**存在同值档位**」**
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 判据（本批正面，可复用）：
   **令牌表该有一条「同一个家族里不允许同值」的不变式** ——
   同值意味着「两个名字选哪一个都一样」，命名就失去了意义
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 这与 B0605b 核的
   `NavMetrics`「不为负且**有序**」是同一族的不变式** ⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒⇒⇒ ⇒⇒⇒⇒⇒ 正面模式 P40（新）：
   **令牌表的守卫 = 计数（registry.length）+ 互异（toSet）+ 关系（有序）三条。**
3. **⚠⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒收窄 B0622 第 ③ 点**：我写「`fast` 与 `interruptedHold`
   都是 120 ms ⇒⇒ 同值不同名要问是不是同一语义」⇒⇒
   **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 核完发现它们**不在同一个家族** ——
   `fast(120)` 在 `AppDurations`（4 档：120/200/320/600）、
   `interruptedHold(120)` 在 `AppRhythms`（3 档）⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ 该测试只断言
   **家族内**互异 ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒⇒⇒⇒ ⇒ 结论**：
   **跨家族同值是允许的**（动效时长 120 与节奏停顿 120 语义本就不同）
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒⇒⇒⇒ ⇒⇒ 判据（升级，取代 B0622 第 ③ 点那半句）：
   **互异性只在家族内成立** —— 判「同值是不是问题」要先问「同一个家族吗」**
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒⇒⇒⇒ ⇒⇒ 这是「收窄」而不是「推翻」：
   跨家族同值仍值得一句说明（同 B0601 的两个 480），但它不是缺陷**
4. **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ 而 `:231-232` 是**又一次「不做什么 + 为什么」**：
   「`AppPalette` 不再是『平的令牌表』，所以它**退出了 `kTokenFamilies`**」
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 判据：一张「家族清单」被修改时，
   要写「谁退出了、为什么它的形状变了」** ⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ ⇒⇒ 与 B0601 核的 `nav_host.dart:47-52`
   「rail 已删 + 不要加回来」**同族** ⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒⇒⇒ ⇒⇒ ⇒⇒⇒⇒⇒⇒ ⇒ 正面模式：
   **「集合成员退出」比「成员消失」更需要解释** —— 消失是删掉，退出是判定它不再同类。**

## 未核
`kTokenFamilies` 那张表的完整形状（`:139` 那行）· `Motion.names`（非 Map 的家族怎么登记）·
`AppRhythms` 的 3 档具体值
