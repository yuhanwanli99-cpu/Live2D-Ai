# BATCH-0613b 落盘（极简）· P2 候选撤销：两套判等各管一层，不混用

## 编号说明
`BATCH-0613.md` 已存在（内容是 `ignite.sh` 尾部四个入口）⇒ 本批改落 `BATCH-0613b.md`。

## 命令（只读）
```
ls AUDIT-REPO/BATCH-0613.md
grep -rn "sameAs" shell/flutter/lib shell/flutter/test | grep -v "bool sameAs"
grep -rn "backgroundItemIdentity" shell/flutter/lib shell/flutter/test
sed -n '473,477p' shell_prefs.dart
sed -n '968,972p' display_prefs.dart
sed -n '939,944p' / :1006,1012p  appearance_background.dart
```

## `sameAs` 的两处调用（都在持久层）
```
app/shell_prefs.dart:475
    if (prefs.backgrounds.any((BackgroundItem b) => b.sameAs(item))) {
      _setImageMessage(forShell: true, text: '已经有这个图案了', failed: false);
settings/display_prefs.dart:968-971
    if (backgrounds.length != other.backgrounds.length) return false;
    for (int i = 0; i < backgrounds.length; i++) {
      if (!backgrounds[i].sameAs(other.backgrounds[i])) return false;
```
⇒ 用途 A：**加同一项前先查重**（用户操作）
⇒ 用途 B：**两个 prefs 的背景库是不是同一份**（`==` 实现里比较列表）

## `backgroundItemIdentity` 的两处调用（都在 UI 层）
```
appearance_background.dart:941-943
    final String identity = backgroundItemIdentity(row);
    return _LibraryRow(key: ValueKey<String>('lib-$identity-$index'), …
appearance_background.dart:1007-1009
  void _tap(int index) {
    final String identity = backgroundItemIdentity(widget.items[index]);
    if (_managing) { _toggle(identity); return; }
（另 :336 / :859 / :879 三处同形）
```

## 四个可核点
1. **B0612b 第 3 点的 P2 候选撤销。**
   两套判等**各管一层**：`sameAs`（bool）管**持久层**（查重 + prefs 相等）；
   `backgroundItemIdentity`（String）管**UI 层**（`ValueKey` + 选中态）。
   **判据（最值钱）**：两处「看起来重复」的判等，先看它们的**调用层**；
   UI 层的 key 与持久层的相等不是同一件事。
   这是 B0440 第 10 形态的对偶：「有两套判等、只看定义会以为重复」。
2. **`:941` 的 key 写的是 `'lib-$identity-$index'`** —— 同时带身份与下标。
   `background_item.dart:97` 的头注写「选中 / 展开都存身份，**不存下标**」。
   **⇒ 这不矛盾**：`ValueKey` 里的 `index` 用来**强制重建那一行**（顺序变了要换 element），
   `identity` 用来**跨重排保持同一张图**。
   判据：`ValueKey` 里同时有「身份」与「位置」时，要能说出两者各管什么 —— 本例说得出来。
   ⇒ 若两者真冲突，头注那句该改成「**选中态不存下标；key 仍带下标**」 ⇒ P3 候选，本批不立。
3. **`shell_prefs.dart:475` 的输出看着挤**（`}` 与 `if` 之间像没换行）⇒
   **本批不下结论**：`sed -n` 配 `cut -c` 会把换行后的缩进显示成同一行的空格（B0440 第 8 形态）。
   判据：`sed` 看到「`}` 和下一个 `if` 挤在一行」时，先换 `awk '{print NR": "$0}'` 或
   `grep -n -A2` 复核，再判是 (a) 文件真的这样（格式坏了）还是 (b) 我的输出宽度造成的错觉。
   **⇒ 未核**（下一批用 `grep -n -A2` 复核）。
4. **正面模式 P35 已在 B0612b 记**（`sealed` + 无 `default` 的 `switch` 是一对），本批无新增。

## 未核
`shell_prefs.dart:475` 是否真格式坏（第 3 点）· `AppShell._settingsSurface` 注入点 ·
`main.dart`（1344）头注 · `_ModConfigTileState` 那 440 行

## 补核（上一轮失控未写，本轮补上）
第 3 点用 `cat -A` 确证：
```
:475 }    if (prefs.backgrounds.any((BackgroundItem b) => b.sameAs(item))) {$
```
`$`（真换行）在 `if` 之后 => `}` 与 `if` 确实同行 => 格式坏了。
`awk 'NR==475 {print length($0)}'` = **75** => 不超 100
=> 真因不是「为躲超长才挤」，是别处并进来的。
`awk 'length($0)>100' shell_prefs.dart` 全文 0 命中。

第 5 点（本轮新发现）：同文件有两��查重写法
```
:246-249
    final String id = backgroundIdOf(dataUrl);
    if (prefs.backgrounds.any(
      (BackgroundItem b) => b is BackgroundImage && b.id == id,
    )) {
:475
    }    if (prefs.backgrounds.any((BackgroundItem b) => b.sameAs(item))) {
```
=> `:247` 手写 `b is BackgroundImage && b.id == id`（**限定 image 类型**）
=> `:475` 用 `b.sameAs(item)`（`item` 声明为 `final BackgroundPattern item`）
=> 两者对**同一 item** 可能不等价：`:247` 只比 image，`:475` 比 pattern 身份
=> **=>⇒⇒⇒ 判据：同一文件里两种查重写法，要问「它们比较的 item 类型是否相同」**
=> **未核**（下一批）

## 未核（第 3、5 点已核，见上）
`AppShell._settingsSurface` 注入点 · `main.dart`（1344）头注 ·
`_ModConfigTileState` 那 440 行
