//! 探针 3：左乘用户变换（NDC 空间）语义验证。验证后删除。
use glam::Affine2;
use glam::Vec2;
use l2d::asset::ModelPackage;
use l2d::model::LoadedModel;

const BAI: &str = "../../assets/models/bai/runtime/bai.model3.json";

#[test]
fn probe_left_multiply_screen_space() {
    let package = ModelPackage::load(BAI).expect("load bai");
    let loaded = LoadedModel::resolve(package).expect("resolve");
    let canvas = loaded.handle().canvas();
    let layout = loaded.manifest().layout;
    let base = Affine2::from_translation(Vec2::new(layout.center_x, layout.center_y))
        * Affine2::from_scale(Vec2::new(layout.width, layout.height))
        * (Affine2::from_translation(-Vec2::ONE)
            * Affine2::from_scale(
                Vec2::new(2.0, 2.0) / Vec2::new(canvas.width_units, canvas.height_units),
            )
            * Affine2::from_translation(Vec2::new(canvas.center_x_units, canvas.center_y_units))
            * Affine2::from_scale(Vec2::splat(canvas.pixels_per_unit)));
    println!(
        "base: sx={:.6} sy={:.6} tx={:+.4} ty={:+.4}",
        base.matrix2.x_axis.x, base.matrix2.y_axis.y, base.translation.x, base.translation.y
    );
    // 左乘（NDC 屏幕空间）：scale=1.1 + offset=0.1
    let user =
        Affine2::from_translation(Vec2::new(0.1, 0.0)) * Affine2::from_scale(Vec2::splat(1.1));
    let combined = user * base;
    println!(
        "左乘 s1.1+ox0.1: sx={:.4} sy={:.4} tx={:+.4} ty={:+.4}",
        combined.matrix2.x_axis.x,
        combined.matrix2.y_axis.y,
        combined.translation.x,
        combined.translation.y
    );
    // 断言：x/y 等比放大（无变形），平移为 NDC 直加
    assert!((combined.matrix2.x_axis.x - base.matrix2.x_axis.x * 1.1).abs() < 1e-5);
    assert!((combined.matrix2.y_axis.y - base.matrix2.y_axis.y * 1.1).abs() < 1e-5);
    assert!((combined.translation.x - (base.translation.x * 1.1 + 0.1)).abs() < 1e-5);
    assert!((combined.translation.y - base.translation.y * 1.1).abs() < 1e-5);
}
