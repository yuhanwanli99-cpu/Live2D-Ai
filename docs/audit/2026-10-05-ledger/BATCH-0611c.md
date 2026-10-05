# BATCH-0611c 落盘（极简）· 「依据」列指的调研 survey **不在仓里**（三份候选全排除了）

## 编号
`BATCH-0611.md` 与 `BATCH-0611b.md` 均已存在 ⇒ 本批落 `BATCH-0611c.md`。

## 命令（只读）
```
ls AUDIT-REPO/BATCH-0611.md / BATCH-0611b.md
ls docs/research/ | wc -l                      -> 23
ls docs/research/ | grep -iE "ui|survey|web|front"
grep -rln "缺陷 3|survey §8" docs/             -> 只有 web-ui-spec-v3.md 自己
grep -n "^## |缺陷 3" docs/research/ui-design-control-surface-2026-09.md
for f in ui-design-flutter-architecture-2026-09.md neko-ui-alignment-and-gaps.md; do
  grep -c "缺陷 3" docs/research/$f; grep -n "^## 8|^## 9" docs/research/$f; done
```

## 结果
```
docs/research/ 共 23 份；名字含 ui/survey/web/front 的有 4 份：
  industry-emotion-tts-survey.md
  neko-ui-alignment-and-gaps.md
  ui-design-control-surface-2026-09.md
  ui-design-flutter-architecture-2026-09.md

ui-design-control-surface-2026-09.md 的章节：
  :18 ## 0. 方法与基线   :68 ## 1. …   :254 ## 2. …   :386 ## 3. …
  :493 ## 4. …           :608 ## 5. …   :677 ## 6. …   :744 ## 7. …   -> 到 7 为止
  「缺陷 3」命中 0

ui-design-flutter-architecture-2026-09.md：「缺陷 3」0、无 §8/§9
neko-ui-alignment-and-gaps.md：「缺陷 3」0、无 §8/§9

grep -rln "缺陷 3|survey §8" docs/  ->  只有 web-ui-spec-v3.md 自己
```

## 四个可核点
1. **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ 「依据」列指的「调研 survey」**在仓里找不到对应文件**
   ⇒⇒ 三份最像的候选（`ui-design-control-surface` / `ui-design-flutter-architecture` /
   `neko-ui-alignment-and-gaps`）**逐一排除**：`缺陷 3` 全部 0 命中、
   §8/§9 不存在（第一份只到 §7）⇒⇒
   **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 结论：spec 的「依据」列指向**仓外材料** ⇒⇒
   **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 严重度候选 P3**（引用不可复查），**本批不立**
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 留给下一批的判据**：
   「**不可复查的引用**」比「无引用」弱（B0440 第 6 形态）——**它有名字、有节号，
   只是名字指的那份东西不在** ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 这正是
   B0608b 说的「能查到」与「只有线索」之间的差别。**
2. **⭐⭐⭐⭐⭐ ⇒⇒⇒⇒ 而 B0610c 第 ③ 点说「依据列指调研 survey 的具体节号」——
   **节号是真的，指的东西不在** ⇒⇒
   **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 修正 B0610c 的第 ③ 点**：
   那里我把它当**正面**证据（「有节号可查」），本批实测是**「有节号但查不到」** ⇒⇒
   **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 判据（升级，取代 B0610c 第 ③ 点的一半）**：
   **引用质量分三档，不是两档** ——
   ① 有节号 + 文件在（可复查）
   ② **有节号 + 文件不在（看似可复查、实则不可）** ← 本例
   ③ 无节号 + 文件不在（= B0591 的 Gitleaks 那档）
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 判据：**判断引用质量要先确认「目标存在」，
   再看「引用是否精确」—— 顺序反了会把 ② 判成 ①。**
3. **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ 而这与 B0606b/B0609c 那一轮构成**第三个同族案例**：
   - Gitleaks（B0591）：**承诺了、没兑现**
   - 「原则 P1」（B0609c）：**我以为没兑现、其实兑现了**（假阴性）
   - 调研 survey（本批）：**承诺了、兑现了一半**（节号在、文件不在）
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 三种情况的鉴别**：
   **① 查目标存不存在 ② 查引用点找不找得到 ③ 两者都做，才敢下结论**
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 本审计因此新增一条**：
   **引用类结论必须双向验证**（引用点 → 目标；目标 → 引用点）
   ⇒⇒ ⇒⇒ ⇒ 与 B0576「同一对象在文档里出现多次 ⇒ 先找齐全部出现位置」**正交**：
   那条管「找全」，这条管「找对」。
4. **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ 顺带一条可复用的数字**：`docs/research/` **23 份**
   ⇒⇒ 与 B0602 引 AGENTS 记的「10 份 research」**不一致**
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ ⇒ 记录这个差**（23 vs 10），不急着判
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒ 判据：清单类数字（"有 N 份 X"）
   要实测**，`AGENTS.md` 的变更历史是**当时**的快照** ⇒⇒
   ⇒⇒ **⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒ 与 B0460 核的「覆盖率数字是移动靶」同族。**

## 未核
23 份 research 里是否有一份是 spec 引用对象的**改名版** ·
B0602 引 AGENTS 的「10 份 research」原文 · P2/P3/P5/P6 的判据全文
