/// 单条消息气泡（L3）：角色标签、流式光标、失败态、复制。
///
/// 只依赖纯模型 [ChatMessage]（`chat/chat_message.dart`）与**纯 Dart** 的
/// [parseChatMarkdown]——**不** import `chat_controller.dart`，那样会把
/// `package:web` 拖进来，让整个气泡不可测。
///
/// # 2026-09-11（P2-3）改了三件事
///
/// 1. **正文渲染轻量 Markdown**。模型输出里的 `**粗体**` / `` `行内码` ``
///    / `- 列表` 过去是原样显示符号的。解析规则刻意只有三条，
///    理由写在 `chat/chat_markdown.dart` 的头注（不引依赖、不解释标题与
///    代码块、落单记号一律原样保留）。
/// 2. **宽度约束改成「相对可用宽度」**。过去写死 `maxWidth: 460`，
///    而聊天列只有 **320（medium）/ 340（expanded）** 宽——那个约束
///    **从来没有生效过**，气泡一直是被外面的 `Expanded` 压着的。
///    写死的大数字比不写更坏：它让读代码的人以为「这里控制着宽度」。
/// 3. **加了复制按钮**。气泡里的文字本来是 `SelectableText`（能选中），
///    但在 320 px 的窄列里拖选一句话很别扭；模型给出的代码片段更是
///    必然要拿出去用的。
library;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../chat/chat_markdown.dart';
import '../chat/chat_message.dart';
import '../chat/turn_liveness.dart';
import '../design/tokens.dart';
import 'soft_motion.dart';
import 'theme.dart';

class MessageBubble extends StatelessWidget {
  const MessageBubble({
    required this.message,
    this.onRetry,
    this.announcement,
    super.key,
  });

  final ChatMessage message;

  /// 失败态下的「重试」；`null` 时不显示。
  final VoidCallback? onRetry;

  /// **节流后**的播报文本（`LiveRegionThrottle.announcement`）。
  ///
  /// 传 `null` = 这条气泡不参与 live region（历史消息不该被反复播报）。
  /// 之所以不让气泡自己看 `message.text`：`text_delta` 是毫秒级的，
  /// 直接挂 `liveRegion` 会把读屏淹掉——节流必须在**外面**做完再喂进来。
  final String? announcement;

  @override
  Widget build(BuildContext context) {
    final AppColors colors = appColorsOf(context);
    final bool onlyReasoning =
        message.text.trim().isEmpty && message.reasoning.trim().isNotEmpty;

    // ── 系统提示：**不走气泡那条路**（2026-09-11）──
    //
    // 「模型整轮没有返回文字」是一条**事实陈述**，不是角色说的话。把它画成
    // assistant 气泡就是重犯 2026-09-11 那个错（伪造台词）；把它删掉就是
    // 2026-09-11 到 09-12 之间那个错（界面上什么都没有，与坏了无法区分）。
    // 第三条路：居中、小字、弱色的**系统行**——看得见，且一眼看出不是回复。
    if (message.role == ChatRole.system) {
      return _SystemRow(text: message.text, color: colors.contentMuted);
    }

    final bool isUser = message.role == ChatRole.user;
    final bool failed = message.isPlaceholder;

    // 谁说的**有独立的面色**，不是只靠左右对齐：`bubbleUser` 是强调色淡淡
    // 压在面板上，助手用的是中性面。两者同色的话，灰度截图与低视力用户
    // 就只剩「靠哪边」这一个通道了。
    final AppPalette palette = appPaletteOf(context);
    final Color background = failed
        ? palette.dangerSurface
        : (isUser ? palette.bubbleUser : palette.bubbleAssistant);
    final Color foreground = failed ? palette.danger : palette.ink;

    final bool placeholderOnly = message.text.isEmpty && message.streaming;

    return Padding(
      padding: const EdgeInsets.symmetric(
        horizontal: Space.s3,
        vertical: Space.s1,
      ),
      child: Column(
        crossAxisAlignment: isUser
            ? CrossAxisAlignment.end
            : CrossAxisAlignment.start,
        children: <Widget>[
          // ── 角色标签：**2026-09-11 按用户要求去掉可见文字** ──
          //
          // 用户原话：「把助手和用户这两个不显示在对话或者删除」。
          //
          // 去掉之后，「谁说的」由**两条本来就存在的通道**表达：
          // ① 气泡面色（`bubbleUser` 是强调色淡底、`bubbleAssistant` 是中性面）；
          // ② 左右对齐。标签只是第三条，去掉它不会丢信息。
          //
          // 但**语义标签保留**（下面那个 `Semantics` 的 `label`）：
          // 读屏用户没有「左右对齐」这个通道，去掉会真的分不清谁在说话。
          Semantics(
            container: true,
            label: message.role.label,
            child: const SizedBox(width: double.infinity),
          ),
          Semantics(
            // `liveRegion`：**只在流式中的助手气泡**上开。读屏会优先播报
            // 这里的 `label`（已节流），历史消息保持普通节点。
            liveRegion: announcement != null && message.streaming,
            // 2026-09-13：label 里补一句「含思考 N 字」。
            //
            // 两个理由，都很具体：
            // 1. **可达性**：`excludeSemantics: true` 会把整条气泡折成一个节点，
            //    而思考折叠区就在这条气泡里——不写进 label，读屏用户**完全不知道
            //    有思考**（连「有个可展开的开关」都听不到）。
            // 2. 把「思考是否真的到了渲染层」变成可观测事实：不写进 label，
            //    只能靠截图肉眼看，任何基于 DOM 的自动检查都看不到它（本条
            //    feature 自己的验证就踩过这个坑）。
            //
            // **不播报思考正文**：它通常比回复长 5–20 倍（实测 1200+ 字），
            // 读屏会淹掉真正的回复。只报「有多少字」。
            // 已知缺口：折叠开关本身对读屏不可达（`excludeSemantics` 会连它一起
            // 折掉），要修得把思考区从这条气泡的语义子树里拆出来，留 rc.3。
            label: <String>[
              '${message.role.label}说：${announcement ?? message.text}',
              if (message.reasoning.trim().isNotEmpty)
                '（含思考 ${message.reasoning.characters.length} 字）',
              // rc.3 N0：把「这段正文没有语音收尾」也变成可被读屏与自动检查
              // 看见的事实（与「含思考 N 字」同一条纪律）。
              if (message.unfinished) '（未收尾）',
            ].join(),
            excludeSemantics: true,
            child: Container(
              // **相对宽度**，不是写死的 460（见文件头注 ②）。
              // 0.92 留一点边距，让长句子不贴着面板边缘。
              constraints: BoxConstraints(
                maxWidth: MediaQuery.sizeOf(context).width * 0.92,
              ),
              decoration: BoxDecoration(
                color: background,
                borderRadius: BorderRadius.circular(AppRadius.lg),
                border: failed
                    ? Border.all(color: palette.dangerBorder)
                    : null,
              ),
              padding: const EdgeInsets.symmetric(
                horizontal: Space.s3,
                vertical: Space.s2,
              ),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  // ── 思考（推理模型；2026-09-13）──
                  //
                  // 位置在正文**之上**：思考先于答案发生，顺序与模型一致。
                  // 正文为空时它默认展开并带一句说明——那种情况（思考吃掉了
                  // 输出预算）正是用户最需要看到它的时刻。
                  if (message.reasoning.trim().isNotEmpty)
                    _ReasoningSection(
                      text: message.reasoning,
                      streaming: message.streaming,
                      onlyReasoning: message.text.trim().isEmpty,
                      mutedColor: colors.contentMuted,
                      borderColor: colors.hairline,
                    ),
                  Row(
                    mainAxisSize: MainAxisSize.min,
                    crossAxisAlignment: CrossAxisAlignment.end,
                    children: <Widget>[
                      // 只有思考、没有正文时**不**摆一个「…」占位：那会让人以为
                      // 还在等回复，而实际上这一轮已经收口了（说明行会讲清楚）。
                      if (message.text.isNotEmpty || !onlyReasoning)
                        Flexible(
                          child: _MessageBody(
                            text: placeholderOnly ? '…' : message.text,
                            color: foreground,
                          ),
                        ),
                      // 流式光标：只在**正文已经来了**的时候显示，
                      // 否则会与上面的「…」重复。
                      //
                      // **为什么是画出来的竖条，而不是 `Text('▍')`**
                      // （2026-09-11，无头浏览器真机点火时抓到）：`▍`（U+258D）
                      // **不在自托管的中文子集里**（子集 22 036 码点，实测不含它），
                      // 而它**每次流式回复都会上屏** → CanvasKit 找不到字形就去
                      // `fonts.gstatic.com` 拉回退字体（实测到该请求，HTTP 200）。
                      // 这正踩中本项目的硬约束「中文字体必须自托管，否则**断网即
                      // 豆腐块**」——有网时完全看不出来。竖条是纯绘制，零字体依赖，
                      // 宽度/高度也都取令牌。
                      if (message.streaming && message.text.isNotEmpty)
                        Padding(
                          padding: const EdgeInsets.only(left: OpticalNudge.thin),
                          child: SizedBox(
                            width: OpticalNudge.thin,
                            height: Space.s4,
                            child: ColoredBox(color: foreground),
                          ),
                        ),
                    ],
                  ),
                  // ── 兜底正文的说明行（rc.3 N0，2026-09-13）──
                  //
                  // 位置在正文**之下**：它是对这段文字的注脚（「没有语音收尾」），
                  // 不是内容本身。只在 `unfinished` 且确有正文时出现——
                  // 空气泡另有失败/系统行那条路。
                  if (message.unfinished && message.text.trim().isNotEmpty)
                    Padding(
                      padding: const EdgeInsets.only(top: Space.s1),
                      child: Text(
                        kUnfinishedTurnCaption,
                        style: Theme.of(context).textTheme.bodySmall?.copyWith(
                          color: colors.contentMuted,
                        ),
                      ),
                    ),
                  // ── 底部动作条：复制（+ 失败时的重试） ──
                  //
                  // 只在**非流式**时出现：流式期间文本还在变，复制到的会是
                  // 半句话（按「一句一单元」的口径，半句话本身也不该被当成
                  // 一轮的产出）。
                  if (!message.streaming && (message.text.isNotEmpty || failed))
                    _BubbleActions(
                      text: message.text,
                      onRetry: failed ? onRetry : null,
                      muted: colors.contentMuted,
                    ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }
}

/// 「思考」折叠区（推理模型的 `reasoning_content`；2026-09-13）。
///
/// # 为什么默认折叠
///
/// 思考通常比正文长 **5–20 倍**（实测 1200+ 字 vs 20 字回复）。默认展开会把
/// 真正的回复挤出屏幕——用户要的是「先看到回复，需要时才看它怎么想的」。
///
/// **例外**：正文为空时默认展开（`onlyReasoning`）——那种情况思考就是本轮唯一
/// 的内容，折叠起来等于什么都没显示（那正是用户报过的「无模型返回」）。
///
/// # 为什么用自绘的开关而不是 `ExpansionTile`
///
/// `ExpansionTile` 自带 Material 的边距/分隔线/水波纹，与本项目「圆角统一、
/// elevation 全 0、文字做按钮」的外观约定冲突（见 `AGENTS.md` 前端层约定），
/// 而且它会拖进一个不小的组件树。这里只需要「一行可点的标题 + 一段可折叠文字」。
class _ReasoningSection extends StatefulWidget {
  const _ReasoningSection({
    required this.text,
    required this.streaming,
    required this.onlyReasoning,
    required this.mutedColor,
    required this.borderColor,
  });

  final String text;
  final bool streaming;
  final bool onlyReasoning;
  final Color mutedColor;
  final Color borderColor;

  @override
  State<_ReasoningSection> createState() => _ReasoningSectionState();
}

class _ReasoningSectionState extends State<_ReasoningSection> {
  bool? _expandedOverride;

  bool get _expanded => _expandedOverride ?? widget.onlyReasoning;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final int chars = widget.text.characters.length;
    return Padding(
      padding: const EdgeInsets.only(bottom: Space.s2),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          // 标题行：整行可点（`InkWell` 之外用 `GestureDetector`——这里在
          // 气泡内部，父层已有自己的手势处理，水波纹反而会串味）。
          GestureDetector(
            behavior: HitTestBehavior.opaque,
            onTap: () => setState(() => _expandedOverride = !_expanded),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: <Widget>[
                Icon(
                  _expanded ? Icons.expand_less : Icons.expand_more,
                  size: 16,
                  color: widget.mutedColor,
                ),
                const SizedBox(width: Space.s1),
                Text(
                  widget.streaming
                      ? '思考中…（$chars 字）'
                      : _expanded
                      ? '思考（$chars 字）'
                      : '已思考 $chars 字（点开看）',
                  style: theme.textTheme.bodySmall?.copyWith(
                    color: widget.mutedColor,
                  ),
                ),
              ],
            ),
          ),
          if (_expanded)
            Padding(
              padding: const EdgeInsets.only(top: Space.s1),
              child: Container(
                width: double.infinity,
                padding: const EdgeInsets.all(Space.s2),
                decoration: BoxDecoration(
                  // 左侧竖线 + 弱色字：一眼看出「这是旁注，不是角色说的话」。
                  border: Border(
                    left: BorderSide(color: widget.borderColor, width: 2),
                  ),
                ),
                child: Text(
                  widget.text,
                  style: theme.textTheme.bodySmall?.copyWith(
                    color: widget.mutedColor,
                  ),
                ),
              ),
            ),
          if (widget.onlyReasoning && !widget.streaming)
            Padding(
              padding: const EdgeInsets.only(top: Space.s1),
              child: Text(
                kReasoningOnlyCaption,
                style: theme.textTheme.bodySmall?.copyWith(
                  color: widget.mutedColor,
                ),
              ),
            ),
        ],
      ),
    );
  }
}

/// 系统提示行：居中、小字、弱色，**没有气泡底**。
///
/// 三个通道同时与对话气泡区分（不靠单一通道表意，规格 §6.4）：
/// ① 居中（用户靠右、角色靠左）；② 小一号字级；③ 弱色（`contentMuted`）。
/// 没有复制 / 重试按钮——它不是内容，是状态说明。
class _SystemRow extends StatelessWidget {
  const _SystemRow({required this.text, required this.color});

  final String text;
  final Color color;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.symmetric(
        horizontal: Space.s3,
        vertical: Space.s1,
      ),
      child: Semantics(
        container: true,
        // 读屏也要听得出这是系统提示，而不是角色说的话（`excludeSemantics`
        // 后由本 label 独占播报）。
        label: '${ChatRole.system.label}提示：$text',
        excludeSemantics: true,
        child: Center(
          child: Text(
            text,
            textAlign: TextAlign.center,
            style: theme.textTheme.bodySmall?.copyWith(color: color),
          ),
        ),
      ),
    );
  }
}

/// 正文：有记号就走 Markdown，没有就当纯文本。
///
/// 走捷径那条不是微优化：`SelectableText` 与 `SelectableText.rich` 在选择
/// 与复制行为上并不完全一致（后者会把 span 边界带进选区），纯文本消息
/// 没有任何理由付这个代价。
class _MessageBody extends StatelessWidget {
  const _MessageBody({required this.text, required this.color});

  final String text;
  final Color color;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final TextStyle? base = theme.textTheme.bodyMedium?.copyWith(color: color);

    if (!hasChatMarkdown(text)) {
      return SelectableText(text, style: base);
    }

    // 行内码的底色：取 `surfaceContainerHighest`（「比面板稍微突出一点」
    // 的语义槽位），不新造颜色。浅色主题下它也自动是深一档的灰。
    final Color codeBackground = theme.colorScheme.surfaceContainerHighest;

    TextSpan spanOf(MdSpan span) {
      if (span.code) {
        return TextSpan(
          text: span.text,
          style: (base ?? const TextStyle()).copyWith(
            fontFamily: 'monospace',
            backgroundColor: codeBackground,
          ),
        );
      }
      if (span.bold) {
        return TextSpan(
          text: span.text,
          style: (base ?? const TextStyle()).merge(
            const TextStyle(fontWeight: FontWeight.w600),
          ),
        );
      }
      return TextSpan(text: span.text);
    }

    final List<MdBlock> blocks = parseChatMarkdown(text);
    return SelectableText.rich(
      TextSpan(
        children: <InlineSpan>[
          for (int b = 0; b < blocks.length; b++) ...<InlineSpan>[
            if (b > 0) const TextSpan(text: '\n'),
            if (blocks[b].bullet) const TextSpan(text: '· '),
            for (final MdSpan span in blocks[b].spans) spanOf(span),
          ],
        ],
      ),
      style: base,
    );
  }
}

/// 气泡底部的动作条（复制 / 重试）。
class _BubbleActions extends StatelessWidget {
  const _BubbleActions({
    required this.text,
    required this.onRetry,
    required this.muted,
  });

  final String text;
  final VoidCallback? onRetry;
  final Color muted;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(top: OpticalNudge.thin),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          if (text.isNotEmpty) _CopyButton(text: text, muted: muted),
          if (onRetry != null)
            TextButton(onPressed: onRetry, child: const Text('重试')),
        ],
      ),
    );
  }
}

/// 「复制」：点一下把这条消息的原文放进剪贴板，并**就地**显示结果。
///
/// 复制的是**原文**而不是渲染后的文本：用户要的是模型给的那串字
/// （含 `**` 与反引号），拿去别处用。把记号吃掉反而会让人以为自己看错了。
class _CopyButton extends StatefulWidget {
  const _CopyButton({required this.text, required this.muted});

  final String text;
  final Color muted;

  @override
  State<_CopyButton> createState() => _CopyButtonState();
}

class _CopyButtonState extends State<_CopyButton> {
  bool _copied = false;

  Future<void> _copy() async {
    await Clipboard.setData(ClipboardData(text: widget.text));
    if (!mounted) return;
    setState(() => _copied = true);
    // 「停多久」走 `AppRhythms`（节奏），不是 `AppDurations`（过渡时长）——
    // 那 4 档的语义是**一次过渡有多快**。这里借 `interruptedHold`：
    // 它是「瞬时状态保持多久」的那一档，与「已复制」要的是同一个量级，
    // 而**再立一个同值令牌是本项目明确不要的**（见该令牌的注释）。
    await Future<void>.delayed(AppRhythms.interruptedHold);
    if (!mounted) return;
    setState(() => _copied = false);
  }

  @override
  Widget build(BuildContext context) {
    return SoftSwap(
      child: TextButton.icon(
        // 换 key 才能让 `SoftSwap` 认出「内容变了」而做交叉淡入。
        key: ValueKey<bool>(_copied),
        onPressed: _copy,
        icon: Icon(_copied ? Icons.check : Icons.copy_rounded, size: 14),
        label: Text(_copied ? '已复制' : '复制'),
        style: TextButton.styleFrom(foregroundColor: widget.muted),
      ),
    );
  }
}
