/*
 * framework_consumer.cpp — A-V1 框架侧消费冒烟（M1/M2 真实编译模式）
 *
 * 以官方框架自身的方式消费 Core：仅包含框架头（Live2DCubismCore.hpp 包裹头 +
 * 框架类头），所有 Core 访问走 Live2D::Cubism::Core 命名空间。
 * 期望：编译 PASS（这是 M1 把 framework/src 编入 CMake 时的真实形态）。
 */
#include "Live2DCubismCore.hpp"
#include "CubismFramework.hpp"
#include "Model/CubismMoc.hpp"
#include "Model/CubismModel.hpp"

int main()
{
    /* 包裹域符号（框架代码的真实调用方式） */
    const Live2D::Cubism::Core::csmVersion v = Live2D::Cubism::Core::csmGetVersion();
    (void)v;

    /* 包裹域类型（csmVector2 重复声明问题的实测点之一） */
    Live2D::Cubism::Core::csmVector2 vec;
    (void)vec;

    /* 框架类类型可见性抽查（编译期即可验证，不实例化） */
    Live2D::Cubism::Framework::CubismModel* model = 0;
    (void)model;

    return 0;
}
