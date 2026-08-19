/*
 * flat_consumer.cpp — A-V1 PurismCore 平铺侧消费冒烟（现有 native-lib 模式）
 *
 * 与 framework_consumer.cpp 互为对偶：本 TU 仅包含平铺的 PurismCore.h
 * （现有 JNI 层 / purism 自用模式）。期望：编译 PASS。
 * 两个 TU 分别编译后可用 nm 对拍：extern "C" 符号名一致 → 链接层 ABI 兼容
 * （C 链接名不含命名空间，U2 理论点的实证）。
 */
#include "PurismCore.h"

int main()
{
    const csmVersion v = csmGetVersion();
    (void)v;
    return 0;
}
