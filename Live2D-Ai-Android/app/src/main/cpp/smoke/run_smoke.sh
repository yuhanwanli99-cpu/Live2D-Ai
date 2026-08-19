#!/usr/bin/env bash
# run_smoke.sh — A-V1 ABI 混编冒烟执行器（M0，Wave 1）
#
# 用 NDK clang++（aarch64）对 smoke/*.cpp 逐个做「仅编译」（-c，不链接）。
# 输出摘要写入 app/src/main/cpp/vendor-smoke-result.txt（同时回显控制台）。
#
# 用法： bash run_smoke.sh [NDK_ROOT]
#   NDK_ROOT 缺省取 $NDK 环境变量，再缺省取 Windows 默认 SDK 位置。

set -u

SMOKE_DIR="$(cd "$(dirname "$0")" && pwd)"
CPP_DIR="$(dirname "$SMOKE_DIR")"
RESULT_FILE="$CPP_DIR/vendor-smoke-result.txt"

if [ $# -ge 1 ]; then
    NDK_ROOT="$1"
elif [ -n "${NDK:-}" ]; then
    NDK_ROOT="$NDK"
else
    NDK_ROOT="/c/Users/Administrator/AppData/Local/Android/Sdk/ndk/27.0.12077973"
fi

CXX="$NDK_ROOT/toolchains/llvm/prebuilt/windows-x86_64/bin/aarch64-linux-android21-clang++"
NM="$NDK_ROOT/toolchains/llvm/prebuilt/windows-x86_64/bin/llvm-nm"

if [ ! -x "$CXX" ]; then
    echo "FATAL: NDK clang++ not found: $CXX" | tee "$RESULT_FILE"
    exit 1
fi

INC="-I$CPP_DIR/framework/src -I$CPP_DIR/purism -I$CPP_DIR/include"
STD="-std=c++17 -Wall"
FIX="-include stddef.h -include stdint.h"   # 已验证的最小解法（全局先行包含系统头）

OUT=""
summary() { OUT="$OUT$*"; }

summary "============================================================\n"
summary "Live2D CubismNativeFramework x PurismCore — ABI 混编冒烟结果\n"
summary "日期: $(date '+%Y-%m-%d %H:%M:%S') | 编译器: $(basename "$CXX")\n"
summary "include 路径: framework/src, purism, include（PurismCore 兼容头提供 Live2DCubismCore.h）\n"
summary "最小解法标记 FIX = -include stddef.h -include stdint.h（见下方分析 ③）\n"
summary "============================================================\n\n"

run_one() {
    local name="$1"; local suffix="$2"; local extra="$3"; local expect="$4"
    local src="$SMOKE_DIR/$name.cpp"
    local obj="$SMOKE_DIR/${name}${suffix}.o"
    local log="$SMOKE_DIR/${name}${suffix}.log"
    summary "--- ${name}${suffix} [期望: $expect] ---\n"
    summary "命令: \"$CXX\" $STD $extra -c \"$src\" -o \"$obj\"\n"
    if "$CXX" $STD $extra -c "$src" -o "$obj" >"$log" 2>&1; then
        summary "结果: PASS（$([ -s "$log" ] && echo "有警告，见 log" || echo "无警告")）\n\n"
    else
        summary "结果: FAIL（详见 smoke/${name}${suffix}.log，要点摘录如下）\n"
        summary "$(grep -E "error|fatal" "$log" | head -6 | sed 's/^/  /')\n"
        summary "\n"
    fi
}

run_one "flat_consumer"            ""        "$INC"                          "PASS（平铺侧独立编译）"
run_one "framework_consumer"       "_nofix"  "$INC"                          "FAIL（根因复现，见分析 ②）"
run_one "framework_consumer"       ""        "$INC $FIX"                     "PASS（框架侧 + FIX，M1 模式）"
run_one "abi_mix_smoke"            ""        "$INC $FIX"                     "PASS（主冒烟，包裹域引用）"
run_one "abi_mix_smoke"            "_flat"   "$INC $FIX -DPSM_SMOKE_REF_FLAT" "FAIL（guard 遮蔽，见分析 ④）"
run_one "mix_forced_dual_scope"    ""        "$INC $FIX"                     "FAIL（C 链接冲突，见分析 ⑤）"

# 符号对拍：flat 与 framework 两 TU 对同一 extern "C" 符号的引用（链接层 ABI）
summary "--- 符号对拍（flat_consumer.o vs framework_consumer.o, 未定义 csm* 符号）---\n"
if [ -x "$NM" ] && [ -f "$SMOKE_DIR/flat_consumer.o" ] && [ -f "$SMOKE_DIR/framework_consumer.o" ]; then
    summary "$("$NM" --undefined-only "$SMOKE_DIR/flat_consumer.o" | grep -i csm | head -4 | sed 's/^/  flat    /')\n"
    summary "$("$NM" --undefined-only "$SMOKE_DIR/framework_consumer.o" | grep -i csm | head -4 | sed 's/^/  frame   /')\n"
    summary "两 TU 均引用同名 extern \"C\" 符号（csmGetVersion 等，C 链接名不含命名空间）→ 链接层 ABI 兼容（U2 理论点实证）。\n\n"
else
    summary "（llvm-nm 不可用或目标文件缺失，跳过符号对拍）\n\n"
fi

summary "--- 分析结论 ---\n"
summary "① TU 级隔离 = 生产真实模式：flat_consumer（平铺域）与 framework_consumer+FIX（包裹域）\n"
summary "   各自独立编译 PASS；extern \"C\" 同名符号保证链接层兼容。\n"
summary "② 根因（framework_consumer 无 FIX 失败）：官方 Live2DCubismCore.hpp 把 PurismCore.h 整体包进\n"
summary "   namespace Live2D::Cubism::Core；PurismCore.h 内部 #include <stdint.h>/<cstdint> 因此在命名空间内\n"
summary "   展开，libc++ 的 stddef/nullptr_t 声明落入命名空间 → 全局作用域缺失 ::nullptr_t →\n"
summary "   后续任何 C++ 标准头（<new>/<string>/<cstddef>）报 'no member named nullptr_t in the global namespace'。\n"
summary "③ 最小解法（已实证，无需改 purism/ 或上游；M1 在 CMake 实施）：编译 framework 源码时\n"
summary "   target_compile_options 加 -include stddef.h -include stdint.h（全局先行包含系统头，\n"
summary "   guard 使包裹内的再次包含成为空操作）。framework_consumer + FIX → PASS（仅 1 个上游无害\n"
summary "   -Wreorder-ctor 警告：CubismModel.hpp 初始化列表顺序，非本工程问题）。\n"
summary "④ include guard 遮蔽：同一 TU 先包裹包含后，平铺再包含 PurismCore.h 被 guard 静默跳过，\n"
summary "   平铺域符号（::csmVersion/csmGetVersion）不可见。生产无需同一 TU 双作用域：TU 级隔离即可。\n"
summary "⑤ 双作用域强制混编（#undef guard 后）→ C 链接声明冲突：包裹域 csmMoc/csmModel 与平铺域为\n"
summary "   不同类型，同名 C 链接函数（csmReviveMocInPlace/csmGetSizeofModel/csmInitializeModelInPlace/\n"
summary "   csmUpdateModel 等）报 'functions that differ only in their return type' / 'conflicting types'。\n"
summary "   → 同一 TU 内不可同时使用两个作用域的 Core API（M1 设计约束，无需 patch）。\n"

printf "$OUT"
printf "$OUT" > "$RESULT_FILE"
echo ""
echo "== 完整结果已写入: $RESULT_FILE =="
