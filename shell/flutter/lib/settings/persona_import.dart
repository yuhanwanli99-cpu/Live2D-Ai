/// 角色卡导入：**PNG 内嵌卡 + JSON 卡**，纯 Dart、零新依赖（规格 §13.3-4）。
///
/// # PNG 内嵌卡是什么
///
/// 酒馆卡常见形态是一张 PNG 头像，人设 JSON 以 **base64** 形式塞在 PNG 的
/// `tEXt` 块里、关键字为 `chara`。所以导入一张图就能带出整套人设，
/// 不需要用户手工找 JSON 文件。
///
/// # 为什么手写 PNG 分块解析而不是引图片库
///
/// PNG 的分块格式极简（长度 + 类型 + 数据 + CRC），我们要的只是**读一个块**，
/// 不解码像素。引一个图片库只为拿一个 `tEXt` 是典型的「顺手加一个」——
/// 而用户裁决明确要求「目前就核心工程最简最易维护管理即可」。
///
/// # 诚实记录的局限
///
/// - 只读 `tEXt`（未压缩）与 `iTXt` 中 **未压缩**（`compression_flag == 0`）的块。
///   `iTXt` 若压缩（flag=1）需要 zlib 解压：Web 上 `dart:io` 的 `ZLibCodec`
///   不可用，引 `package:archive` 就为了这个不值得 → **如实报错**，不返回半个卡。
/// - 不校验 CRC（我们只读不写；损坏的块会在 base64/JSON 解析处被挡住）。
library;

import 'dart:convert';
import 'dart:typed_data';

import 'persona_card.dart';

/// PNG 签名（8 字节）。
const List<int> kPngSignature = <int>[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

/// 导入结果。
class PersonaImportResult {
  const PersonaImportResult({
    this.card,
    this.error,
    this.source = 'unknown',
  });

  /// 解析出的卡（失败为 `null`）。
  final PersonaCard? card;

  /// 失败原因（给人看的一句话）。
  final String? error;

  /// `json` / `png` / `unknown`。
  final String source;

  bool get ok => card != null;
}

/// 从任意字节导入：**自动判 PNG 还是 JSON 文本**。
///
/// 用户可能拖进来一张 PNG，也可能拖进来一个 `.json`——不该让他先想清楚
/// 「这是哪种卡」。按魔数判断，两条路都走同一条解析。
PersonaImportResult importPersonaBytes(Uint8List bytes) {
  if (_looksLikePng(bytes)) return importPersonaFromPng(bytes);
  // 不是 PNG：当文本试。
  String text;
  try {
    text = utf8.decode(bytes);
  } catch (_) {
    return const PersonaImportResult(error: '这个文件既不是 PNG 也不是 UTF-8 文本');
  }
  return importPersonaFromJsonText(text);
}

/// 从 JSON 文本导入（`.json` 角色卡）。
PersonaImportResult importPersonaFromJsonText(String raw) {
  final PersonaCard? card = parsePersonaCardJson(raw.trim());
  if (card == null) {
    return const PersonaImportResult(error: '这不是可识别的角色卡 JSON');
  }
  return PersonaImportResult(card: card, source: 'json');
}

/// 从 PNG 字节导入（读 `tEXt`/`iTXt` 里关键字为 `chara` 的 base64 JSON）。
PersonaImportResult importPersonaFromPng(Uint8List bytes) {
  final _CharaExtract extracted = _extractPngCharaPayload(bytes);
  if (extracted.error != null) {
    return PersonaImportResult(error: extracted.error, source: 'png');
  }
  final String? payload = extracted.base64Payload;
  if (payload == null) return const PersonaImportResult(error: '这张 PNG 里没有角色卡数据（没有 chara 块）', source: 'png');

  // 酒馆有的卡直接塞未编码的 JSON（少见但存在）——两种都容忍。
  final String text;
  try {
    final List<int> decoded = base64.decode(payload.trim());
    text = utf8.decode(decoded);
  } catch (_) {
    // 不是 base64 → 可能本来就是 JSON 文本。
    if (payload.trimLeft().startsWith('{')) {
      final PersonaImportResult asJson = importPersonaFromJsonText(payload);
      return PersonaImportResult(
        card: asJson.card,
        error: asJson.error,
        source: 'png',
      );
    }
    return const PersonaImportResult(error: 'PNG 里的 chara 数据不是合法的 base64', source: 'png');
  }

  final PersonaCard? card = parsePersonaCardJson(text);
  if (card == null) {
    return const PersonaImportResult(error: 'PNG 里的角色卡 JSON 解析失败', source: 'png');
  }
  return PersonaImportResult(card: card, source: 'png');
}

/// PNG 里 `chara` 负载的抽取结果（内部用）。
class _CharaExtract {
  const _CharaExtract({this.base64Payload, this.error});

  final String? base64Payload;
  final String? error;
}

/// `chara` 关键字（酒馆约定）。
const String kCharaKeyword = 'chara';

/// 走一遍 PNG 分块，取出 `chara` 的文本负载。
///
/// 返回的 [String] 是**块里的原始文本**（通常是 base64）。
_CharaExtract _extractPngCharaPayload(Uint8List bytes) {
  if (!_looksLikePng(bytes)) return const _CharaExtract(error: '不是 PNG 文件');
  int offset = kPngSignature.length;
  while (offset + 8 <= bytes.length) {
    final int length = _readUint32(bytes, offset);
    final String type = String.fromCharCodes(bytes, offset + 4, offset + 8);
    final int dataStart = offset + 8;
    final int dataEnd = dataStart + length;
    if (dataEnd + 4 > bytes.length) {
      return const _CharaExtract(error: 'PNG 数据不完整（块长度超出文件）');
    }

    if (type == 'tEXt') {
      final String? value = _readTextChunk(bytes, dataStart, dataEnd);
      if (value != null) return _CharaExtract(base64Payload: value);
    } else if (type == 'iTXt') {
      final _ITxt? itxt = _readITxtChunk(bytes, dataStart, dataEnd);
      if (itxt != null) {
        if (itxt.compressed) {
          // 明确说清为什么不行，而不是返回一个空卡。
          return const _CharaExtract(
            error: '这张 PNG 的角色卡是压缩的 iTXt（zlib），当前不支持；'
                '请用未压缩的 tEXt 卡或直接导入 JSON',
          );
        }
        return _CharaExtract(base64Payload: itxt.text);
      }
    }
    if (type == 'IEND') break;
    offset = dataEnd + 4; // 跳过 CRC
  }
  return const _CharaExtract();
}

/// `tEXt`：`keyword` + `0x00` + `text`（都是 Latin-1）。
String? _readTextChunk(Uint8List bytes, int start, int end) {
  final int nul = _indexOfZero(bytes, start, end);
  if (nul < 0) return null;
  final String keyword = latin1.decode(bytes.sublist(start, nul));
  if (keyword != kCharaKeyword) return null;
  return latin1.decode(bytes.sublist(nul + 1, end));
}

/// `iTXt` 的最小结构（只为拿文本，语言标签/译名关键字按规范跳过）。
class _ITxt {
  const _ITxt({required this.text, required this.compressed});
  final String text;
  final bool compressed;
}

_ITxt? _readITxtChunk(Uint8List bytes, int start, int end) {
  final int nul = _indexOfZero(bytes, start, end);
  if (nul < 0) return null;
  final String keyword = latin1.decode(bytes.sublist(start, nul));
  if (keyword != kCharaKeyword) return null;
  int p = nul + 1;
  if (p + 2 > end) return null;
  final bool compressed = bytes[p] == 1;
  p += 2; // compression_flag + compression_method
  final int langEnd = _indexOfZero(bytes, p, end);
  if (langEnd < 0) return null;
  final int translatedEnd = _indexOfZero(bytes, langEnd + 1, end);
  if (translatedEnd < 0) return null;
  final String text = utf8.decode(
    bytes.sublist(translatedEnd + 1, end),
    allowMalformed: true,
  );
  return _ITxt(text: text, compressed: compressed);
}

int _indexOfZero(Uint8List bytes, int start, int end) {
  for (int i = start; i < end; i++) {
    if (bytes[i] == 0) return i;
  }
  return -1;
}

bool _looksLikePng(Uint8List bytes) {
  if (bytes.length < kPngSignature.length) return false;
  for (int i = 0; i < kPngSignature.length; i++) {
    if (bytes[i] != kPngSignature[i]) return false;
  }
  return true;
}

/// 大端 32 位（PNG 全是大端）。
int _readUint32(Uint8List bytes, int at) =>
    (bytes[at] << 24) | (bytes[at + 1] << 16) | (bytes[at + 2] << 8) | bytes[at + 3];
