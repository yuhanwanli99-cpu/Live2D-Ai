# BATCH-0632b 落盘 · 六个参数四个带缺省；dense 只改一个 margin

（注：`BATCH-0632.md` 已被另一批占用，**编号撞车**、本批另起 `-b` 后缀）

## 跑的命令（只读）
```
sed -n '60,80p' lib/ui/inline_notice.dart | grep -vE "^\s*$"
sed -n '96,112p' lib/ui/inline_notice.dart | grep -nE "dense|onDismiss|padding|if "
```

## 逐字
```
:61  class InlineNotice extends StatelessWidget {
:62    const InlineNotice({
:63      required this.message,
:64      this.severity = NoticeSeverity.danger,
:65      this.actions = const <NoticeAction>[],
:66      this.onDismiss,
:67      this.dense = false,
:68      super.key,
:69    });
:70    final String message;
:71    final NoticeSeverity severity;
:72    final List<NoticeAction> actions;
:73    final VoidCallback? onDismiss;
:74    /// 紧凑形态：用在字段行下面（那里空间小，且下面通常紧跟另一个控件）
:75    final bool dense;

:107   margin: dense ? EdgeInsets.zero : const EdgeInsets.all(Space.s2),
```

## 三个可核点
1. **六个参数、四个带缺省**（`severity` / `actions` / `onDismiss` / `dense`）
   => 与 B0583 `dark: true` · B0584 `restarted = false` · B0629 `primary = false` 同一族
   （**缺省值写在参数表里、不靠调用方记得传**）
2. `severity` 的缺省是 `NoticeSeverity.danger`，而 B0555 核的 Rust 侧 `ModStatus` 缺省是 `Disabled`
   => **同一套「缺省写在类型上」的手法在两层各出现一次**
   => 而**两层的缺省恰好是相反的语义**（UI 侧默认「危险」、Mod 侧默认「没启用」）
   => ⇒ **这不矛盾**：UI 侧的缺省是「没传就是坏消息」、Mod 侧的缺省是「没登记就是没开」
   => 与 B0491「两个不同原因不该共用一处」同族（**两个缺省各有自己的语境**）
3. **`dense` 只改一个 `margin`**（`:107`）
   ⇒ 名字叫「**紧凑形态**」、头注说「用在字段行下面」、而实现**只收外边距**
   ⇒ **未核**：`dense` 有没有同时改字号/内边距（若只改 margin，「形态」这个词偏大）
   ⇒ 记为**观察候选**（不记发现：无行为后果、`:74` 的头注已经说了「空间小」这个用途）

## 编号撞车（新记的一条纪律）
- `BATCH-0632.md` 已被 **B0556 那批**占用（内容是「三条规矩」那一批）
- ⇒ 后续落盘前**先 `ls` 看文件在不在**，在就加后缀
- ⇒ 与 B0440 第 1 种形态（路径/名字不对）**同族**、只是这次撞的是**输出名**不是**输入名**

## 未核
`dense` 是否还改别处 · `InlineNotice` 的 severity 三态 icon 来源（`:118-130`）·
其余 mod-system 两个文件本体 · shared/ 其余 8 个 json ·
setup_linux.sh / ignition-precheck.sh / verify_core_chain.py /
check_public_secrets.py / font_subset_ranges.py 本体 · 面板三份余下部分
