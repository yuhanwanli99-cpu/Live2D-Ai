/*
 * mix_forced_dual_scope.cpp — A-V1 双作用域强制混编（U2「csmVector2 等类型重复声明」实测）
 *
 * 说明：由于 PurismCore.h / Live2DCubismCore.h 使用传统 include guard，
 *       同一 TU 内两次包含第二次会被静默跳过（见 abi_mix_smoke.cpp 的观察）。
 *       本文件在两次包含之间 #undef guard，强制两个作用域（命名空间包裹域 +
 *       平铺全局域）同时完整展开，实测「类型重复声明 / C 链接声明冲突」是否发生。
 *
 * 本文件不改动 purism/ 与上游 framework/ 的任何代码（#undef 仅作用于本 TU）。
 */
#include "Live2DCubismCore.hpp" /* 包裹域：Live2D::Cubism::Core::csmXxx */

#undef LIVE2D_CUBISM_CORE_H
#undef PURISM_CORE_H

#include "PurismCore.h"         /* 平铺域：全局 ::csmXxx（guard 已清除，强制重展开） */

#include "CubismFramework.hpp"  /* 框架头：消费包裹域 Core:: 类型 */

int main()
{
    const Live2D::Cubism::Core::csmVersion wrapped = Live2D::Cubism::Core::csmGetVersion();
    (void)wrapped;

    const csmVersion flat = csmGetVersion();
    (void)flat;

    return 0;
}
