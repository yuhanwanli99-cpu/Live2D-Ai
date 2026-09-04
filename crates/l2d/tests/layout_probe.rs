//! 临时探针 2：验证左乘/右乘用户变换的数值语义（定位"模型不见"）。验证后删除。
use glam::Affine2;
use glam::Vec2;
use l2d::asset::ModelPackage;
use l2d::model::LoadedModel;

const BAI: &str = "../../assets/models/bai/runtime/bai.model3.json";

fn scale_of(t: &Affine2) -> f64 {
    (t.matrix2.x_axis.x * t.matrix2.y_axis.y - t.matrix2.x_axis.y * t.matrix2.y_axis.x)
        .abs()
        .sqrt() as f64
}

#[test]
fn probe_user_transform_semantics() {
    let package = ModelPackage::load(BAI).expect("load bai");
    let loaded = LoadedModel::resolve(package).expect("resolve");
    // layout_transform 是 pub(crate)——经由 wasm 侧同款途径：core.transform() 不可用，
    // 此处用 BehaviorThread 等价：从 l2d 公开 API 只能拿 canvas/layout，
    // 所以这里手动复算 layout_transform（公式与 l2d/src/renderer/mod.rs:233 一致）。
    let canvas = loaded.handle().canvas();
    let canvas_dims = Vec2::new(canvas.width_units, canvas.height_units);
    let canvas_center = Vec2::new(canvas.center_x_units, canvas.center_y_units);
    let layout = loaded.manifest().layout;
    let whole_canvas = Affine2::from_translation(-Vec2::ONE)
        * Affine2::from_scale(Vec2::new(2.0, 2.0) / canvas_dims)
        * Affine2::from_translation(canvas_center)
        * Affine2::from_scale(Vec2::splat(canvas.pixels_per_unit));
    let base = Affine2::from_translation(Vec2::new(layout.center_x, layout.center_y))
        * Affine2::from_scale(Vec2::new(layout.width, layout.height))
        * whole_canvas;
    println!("base: scale≈{:.6} tx={:+.4} ty={:+.4}", scale_of(&base), base.translation.x, base.translation.y);

    // 右乘（当前实现）：base * scale(1.1)
    let right = base * Affine2::from_scale(Vec2::splat(1.1));
    println!("右乘 scale1.1 → 合成 scale≈{:.6} tx={:+.4}", scale_of(&right), right.translation.x);

    // 左乘（提议）：user * base —— 世界空间（NDC）作用
    let user = Affine2::from_scale(Vec2::splat(1.1));
    let left = user * base;
    println!("左乘 scale1.1 → 合成 scale≈{:.6} tx={:+.4} ty={:+.4}", scale_of(&left), left.translation.x, left.translation.y);

    // 左乘 offset=0.2 + scale=1.1：世界平移 0.2 NDC —— 用户直觉
    let user_t = Affine2::from_translation(Vec2::new(0.2, 0.0)) * Affine2::from_scale(Vec2::splat(1.1));
    let left2 = user_t * base;
    println!("左乘 offset0.2+scale1.1 → scale≈{:.4} tx={:+.4}", scale_of(&left2), left2.translation.x);
}
