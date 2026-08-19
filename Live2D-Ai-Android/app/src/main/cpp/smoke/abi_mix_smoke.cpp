/*
 * abi_mix_smoke.cpp — A-V1 ABI 混编冒烟（M0 主冒烟，U2 闭环）
 *
 * 目的：验证「官方 CubismNativeFramework（framework/src）+ PurismCore（purism/）」
 *       在同一翻译单元内混编能否通过编译（仅编译，不链接）。
 *
 * 参与方：
 *   1) Live2DCubismCore.hpp  官方框架的 Core 命名空间包裹头
 *      （namespace Live2D::Cubism::Core { #include "Live2DCubismCore.h" }）。
 *      其包含的 Live2DCubismCore.h 在本工程由 PurismCore 的兼容头提供
 *      （purism/Live2DCubismCore.h -> PurismCore.h），全部声明进入
 *      Live2D::Cubism::Core 命名空间。
 *   2) CubismFramework.hpp   官方框架主头（内部以 Core:: 前缀消费 Core 类型）。
 *   3) PurismCore.h          PurismCore 平铺 C API（全局作用域）。
 *
 * 编译（见 run_smoke.sh）：
 *   - 默认        ：仅引用包裹域符号 → 期望 PASS（框架侧 M1/M2 真实模式）
 *   - -DPSM_SMOKE_REF_FLAT ：额外引用平铺域符号 → 观察 include guard 遮蔽行为
 */
#include "Live2DCubismCore.hpp" /* 官方框架包裹头 → Live2D::Cubism::Core::csmXxx */
#include "CubismFramework.hpp"  /* 官方框架主头，内部消费 Core:: 类型 */
#include "PurismCore.h"         /* PurismCore 平铺 C API → 全局 ::csmXxx */

int main()
{
    /* 包裹域符号：框架代码的真实消费方式（Core:: 前缀） */
    const Live2D::Cubism::Core::csmVersion wrapped = Live2D::Cubism::Core::csmGetVersion();
    (void)wrapped;

#ifdef PSM_SMOKE_REF_FLAT
    /* 平铺域符号：同一 TU 内直接引用（受 include guard 影响，见 vendor-smoke-result.txt） */
    const csmVersion flat = csmGetVersion();
    (void)flat;
#endif

    return 0;
}
