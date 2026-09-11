//! `static_frame_trace` — 节点 C / C5 静止帧上传观测工具（P1-1 任务）。
//!
//! 用途：按 `docs/plans/node-c-c1-c11-formal-audit-2026-08-27.md` C5 节
//! 93-98 行门禁的「可核查判定方法 4.3」——捕获 `ayagami_render=trace` 日志
//! 并按行模式计数，验证「300 个静止帧」内：
//! - 是否有 ArtMesh 全段重写（"ArtMesh #N changed" debug 行 → 0 即早退成功）；
//! - 是否有 mask 纹理重建（"Create clip set N texture" debug 行 → 0）；
//! - 是否有 mask RenderPass（"Render ArtMesh" trace 行 → 0）；
//! - 主 pass draw 数（"Draw ArtMesh" trace 行；仅参考，C5 不强制恒定）。
//!
//! 全程只调 `render_to_view`（阻塞路径，离屏确定性场景允许），不调
//! `update` / 参数 setter / 动作；warm-up 之后模型状态完全冻结。
//!
//! # 观测口径（诚实标注）
//!
//! 1. **Texture / BindGroup create 数**：用 `wgpu::Device::get_internal_counters()`
//!    取差值；wgpu 29 默认不开 `counters` 特性时 HAL 字段为零，函数本身仍可调。
//!    即使 HAL 不上报，源码已保证 bind group 一次性（`renderer.rs:877-901`），
//!    所以 0 是必然结果——双重确认。
//! 2. **BufferWrite 次数 / 字节数**：本工具不直接截获 `queue.write_buffer`；
//!    改为按「early-exit 推理」：若 `ArtMesh #N changed` debug 行 = 0，则
//!    `prepare()` 在 `renderer.rs:1024-1026` 早退，**只有 L962 的
//!    `write_buffer(global_buffer, 0, 64B)` 执行**（在早退前无条件）。
//!    故每帧 BufferWrite = 1，字节 = 64。ArtMesh uniform 整段写 = 0。
//! 3. **TextureWrite / copy**：源码 `prepare()` 内无 `queue.write_texture` 或
//!    `copy_texture_to_texture`（`renderer.rs` 全部命中已在
//!    `docs/verification/node-c-c5-prepare-upload-audit.md` ① 节列出），
//!    故 300 帧累计 = 0。
//! 4. **Mask RenderPass**：由"Render ArtMesh" trace 行计数得到。
//!
//! # 用法
//!
//! ```text
//! cargo run -p l2d --example static_frame_trace -- [model3.json] [size] [warmup] [frames]
//! ```
//!
//! - `model3.json`：默认仓库内 Bai 皮套（与 `render_model` 同一相对路径）；
//! - `size`：画布边长（默认 `1024`）；
//! - `warmup`：warm-up 帧数（默认 `5`，先调 `update(dt)` + `render_frame`）；
//! - `frames`：静止帧循环次数（默认 `300`）。
//!
//! # 输出
//!
//! 打印：warm-up 与 300 帧静止段的日志事件计数 + 与 C5 门禁逐条对照表。
//!
//! # 环境
//!
//! 需要 headless GPU 后端（vulkan / gl / llvmpipe）。无后端时本 example
//! 会返回 `RenderError::NoGpuBackend` 并打印已尝试的后端。

use std::path::PathBuf;
use std::sync::Mutex;

use l2d::asset::ModelPackage;
use l2d::model::LoadedModel;
use l2d::renderer::{FIXED_DT_60HZ, OffscreenRenderer};

/// 默认模型：仓库内 Bai 皮套（相对仓库根；与 `render_model` 保持一致）。
const DEFAULT_MODEL3: &str = "assets/models/bai/runtime/bai.model3.json";
const DEFAULT_SIZE: u32 = 1024;
const DEFAULT_WARMUP: u32 = 5;
const DEFAULT_FRAMES: u32 = 300;

/// 日志捕获器：装到 `log::Log` 全局，记录每条日志行的原始字符串。
#[derive(Default)]
struct LogCapture {
    lines: Mutex<Vec<String>>,
}

impl LogCapture {
    fn new() -> Self {
        Self::default()
    }

    /// 取走当前累积的所有日志行（消费、清空）。
    fn drain(&self) -> Vec<String> {
        let mut g = self.lines.lock().expect("log capture poisoned");
        std::mem::take(&mut *g)
    }

    /// 计数：包含 `needle` 的行数。
    fn count_contains(lines: &[String], needle: &str) -> u64 {
        lines.iter().filter(|l| l.contains(needle)).count() as u64
    }
}

impl log::Log for LogCapture {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        // 仅接收 `ayagami*` 与 `wgpu*` 相关 target；过滤掉其它 crate 的噪声。
        // 真正的级别阈值由 `log::set_max_level` 控制。
        let t = metadata.target();
        t.starts_with("ayagami") || t.starts_with("wgpu")
    }

    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        // 与 env_logger 默认格式保持一致：`<target>: <message>`。
        // 例：`ayagami_render: ArtMesh #3 changed`
        let line = format!("{}: {}", record.target(), record.args());
        self.lines.lock().expect("log capture poisoned").push(line);
    }

    fn flush(&self) {}
}

fn parse_u32_arg(name: &str, raw: Option<&str>, default: u32) -> Result<u32, String> {
    match raw {
        None => Ok(default),
        Some(s) => s
            .parse::<u32>()
            .map_err(|e| format!("非法 {name} `{s}`: {e}")),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let model3_path = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_MODEL3));
    let size = parse_u32_arg("size", args.next().as_deref(), DEFAULT_SIZE)?;
    let warmup = parse_u32_arg("warmup", args.next().as_deref(), DEFAULT_WARMUP)?;
    let frames = parse_u32_arg("frames", args.next().as_deref(), DEFAULT_FRAMES)?;

    println!("=== l2d static_frame_trace (C5 验证) ===");
    println!("model3: {}", model3_path.display());
    println!("size: {size}x{size} | warmup: {warmup} | static frames: {frames}");

    // ---- 1) 安装日志捕获器（仅接收 trace 及更高级别） ----
    // `log::set_logger` 要求 `&'static dyn Log`；本 example 单进程单线程
    // 用法下用 `Box::leak` 即可。捕获器存活到 main 末尾（与进程同寿）。
    let capture: &'static LogCapture = Box::leak(Box::new(LogCapture::new()));
    log::set_logger(capture as &'static dyn log::Log)
        .map_err(|e| format!("无法安装 log capture（已被占用）: {e}"))?;
    // `Trace` 级别会同时放出 `debug!` / `info!` / `trace!`（log 标准定义）。
    log::set_max_level(log::LevelFilter::Trace);

    // ---- 2) 加载 Bai 模型（与 render_model 一致） ----
    let package = ModelPackage::load(&model3_path)?;
    let loaded = LoadedModel::resolve(package)?;

    // ---- 3) 离屏渲染器（headless wgpu） ----
    let mut renderer = OffscreenRenderer::new(size, size)?;
    println!("gpu adapter: {}", renderer.gpu_adapter());
    renderer.load_model(&loaded)?;

    // 截断至此为止的日志（load_model 阶段会产生大量 "Loading textures" /
    // "Done initializing renderer" 之类，与 C5 静止帧观测无关）。
    let _ = capture.drain();

    // ---- 4) warm-up：让模型状态 settle（idle + 物理 + 首帧绘制） ----
    // warm-up 帧里会有大量 ArtMesh 变化（`ArtMesh #N changed`）、mask
    // 重建（`Create clip set N texture`）、主 pass draw 等等——
    // 这些都是「非静止」噪声，要与后续 300 帧静止段分别计数。
    println!("-- warm-up ({warmup} 帧，含 update + render_frame) --");
    for i in 0..warmup {
        renderer.update(FIXED_DT_60HZ)?;
        let _frame = renderer.render_frame()?;
        println!("warm-up frame {i} done");
    }
    let warmup_lines = capture.drain();

    // warm-up 收尾后，artmesh dirty 全部清零，clip.dirty 全部清零
    // （见 ayagami-render `renderer.rs:1623-1655`）。
    // 下一次 `render_to_view` 的 prepare 早退判定应当使 `any_changes=false`。
    let baseline_counters = renderer.core().gpu().device().get_internal_counters();
    println!(
        "HAL 计数（warm-up 末）: buffers={} textures={} bind_groups={} pipelines={} (wgpu 默认未开 counters 特性时这些字段为 0)",
        baseline_counters.hal.buffers.read(),
        baseline_counters.hal.textures.read(),
        baseline_counters.hal.bind_groups.read(),
        baseline_counters.hal.render_pipelines.read(),
    );

    // ---- 5) 静止帧：300 帧只 render_frame（内部走 render_to_view 阻塞），
    //           不动任何状态 ----
    // 不调 update / set_parameter / override_parameter。理论上：
    //   - prepare() 入口 L962 写 64B global_buffer；
    //   - L991-1020 检测 any_changes → false；
    //   - L1024 早退返回 false；
    //   - render() 仍跑（不在早退路径内），主 pass draw ArtMesh 多次。
    println!("-- 静止帧循环（{frames} 帧，仅 render_frame）--");
    let started = std::time::Instant::now();
    for i in 0..frames {
        let _frame = renderer.render_frame()?;
        if i == 0 || i + 1 == frames || (i + 1) % 50 == 0 {
            println!("static frame {i} done");
        }
    }
    let elapsed = started.elapsed();
    let static_lines = capture.drain();

    // ---- 6) 末态 HAL 计数（与 baseline 差值 = 静止段增量的资源创建数） ----
    let final_counters = renderer.core().gpu().device().get_internal_counters();
    println!(
        "HAL 计数（静止段末）: buffers={} textures={} bind_groups={} pipelines={}",
        final_counters.hal.buffers.read(),
        final_counters.hal.textures.read(),
        final_counters.hal.bind_groups.read(),
        final_counters.hal.render_pipelines.read(),
    );

    // ---- 7) 日志事件分类统计 ----
    // 模式与 ayagami-render `renderer.rs` 中 `debug!` / `trace!` 调用点
    // 一一对应：
    //   - "ArtMesh #" + " changed" → L995（ArtMesh 脏标记）
    //   - "Create clip set"       → L971（mask 纹理重建）
    //   - "Render ArtMesh"        → L1268（mask pass 内 draw）
    //   - "Draw ArtMesh"          → L1611（主 pass 内 draw；必经）
    let w_artmesh_changed = LogCapture::count_contains(&warmup_lines, " changed");
    let w_create_clip = LogCapture::count_contains(&warmup_lines, "Create clip set");
    let w_render_artmesh = LogCapture::count_contains(&warmup_lines, "Render ArtMesh");
    let w_draw_artmesh = LogCapture::count_contains(&warmup_lines, "Draw ArtMesh");

    let s_artmesh_changed = LogCapture::count_contains(&static_lines, " changed");
    let s_create_clip = LogCapture::count_contains(&static_lines, "Create clip set");
    let s_render_artmesh = LogCapture::count_contains(&static_lines, "Render ArtMesh");
    let s_draw_artmesh = LogCapture::count_contains(&static_lines, "Draw ArtMesh");

    // HAL 增量（warm-up 末 vs 静止段末）。wgpu 默认未开 counters 特性时
    // 全 0；开了的话能直接看到 buffer / texture / bind_group 的累计增数。
    let baseline_buffers = baseline_counters.hal.buffers.read();
    let baseline_textures = baseline_counters.hal.textures.read();
    let baseline_bind_groups = baseline_counters.hal.bind_groups.read();
    let baseline_pipelines = baseline_counters.hal.render_pipelines.read();
    let final_buffers = final_counters.hal.buffers.read();
    let final_textures = final_counters.hal.textures.read();
    let final_bind_groups = final_counters.hal.bind_groups.read();
    let final_pipelines = final_counters.hal.render_pipelines.read();
    let delta_buffers = final_buffers.saturating_sub(baseline_buffers);
    let delta_textures = final_textures.saturating_sub(baseline_textures);
    let delta_bind_groups = final_bind_groups.saturating_sub(baseline_bind_groups);
    let delta_pipelines = final_pipelines.saturating_sub(baseline_pipelines);

    // ---- 8) 输出原始数据表 + C5 门禁对照 ----
    println!("\n=== 原始数据表 ===");
    println!(
        "warm-up 段（{warmup} 帧）：changed={w_artmesh_changed} | \
         CreateClipSet={w_create_clip} | RenderArtMesh(mask pass)={w_render_artmesh} | \
         DrawArtMesh(主 pass)={w_draw_artmesh}"
    );
    println!(
        "静止段（{frames} 帧）：changed={s_artmesh_changed} | \
         CreateClipSet={s_create_clip} | RenderArtMesh(mask pass)={s_render_artmesh} | \
         DrawArtMesh(主 pass)={s_draw_artmesh} | 耗时 {elapsed:?}"
    );
    println!(
        "HAL 增量（静止段）: buffers={delta_buffers} textures={delta_textures} \
         bind_groups={delta_bind_groups} render_pipelines={delta_pipelines}"
    );
    if delta_buffers == 0
        && delta_textures == 0
        && delta_bind_groups == 0
        && delta_pipelines == 0
        && final_buffers == 0
    {
        println!("（HAL 增量全 0；当前 wgpu 构建未启用 `counters` 特性——");
        println!(" 推理依据：源码 `renderer.rs:870-901` 已保证 bind group 一次性、");
        println!(" mask 重建仅视口变化，本工具视口稳定 {size}x{size}，全程不重建。）");
    }

    // ---- 9) C5 门禁逐条对照（结论性观察，不裁决） ----
    println!("\n=== C5 门禁对照（裁决 95-98 行）===");
    let f = frames as u64;
    // 门禁 1：300 个静止帧记录 BufferWrite / TextureWrite / Mask RenderPass。
    // 我们用日志事件作为代理：
    //   - Mask RenderPass: RenderArtMesh trace 行（mask pass 入口 L1230）
    //   - TextureWrite/copy: 0（prepare 内无 write_texture；mask 重绘是 render pass
    //     而非 write_texture，已被 RenderArtMesh 计数覆盖）
    //   - BufferWrite 字节数：64B × 300（每帧 L962 global_buffer 写）—— 由
    //     `s_artmesh_changed == 0` 推得 prepare 早退成立
    println!(
        "  [1] 静止段 BufferWrite/TextureWrite/MaskRenderPass 计数：\
         changed={s_artmesh_changed}/0，CreateClipSet={s_create_clip}/0，\
         RenderArtMask={s_render_artmesh}/0；\
         global 64B write 由源码 L962 推断为 {f} 次（每帧 1 次）",
    );
    // 门禁 2：稳定 viewport 下 texture create / bind-group create 必须为 0。
    println!(
        "  [2] 静止段 texture/bind-group create：\
         CreateClipSet={s_create_clip}, HAL 增量 textures={delta_textures}, \
         bind_groups={delta_bind_groups} → {}",
        if s_create_clip == 0 && delta_textures == 0 && delta_bind_groups == 0 {
            "通过"
        } else {
            "不通过（需排查）"
        }
    );
    // 门禁 3：静止帧允许 global 64B write，ArtMesh 全段写和 mask pass 应为 0。
    println!(
        "  [3] 静止段 ArtMesh 全段写与 mask pass：\
         changed={s_artmesh_changed}（应为 0），RenderArtMask={s_render_artmesh}（应为 0）→ {}",
        if s_artmesh_changed == 0 && s_render_artmesh == 0 {
            "通过"
        } else {
            "不通过（需排查）"
        }
    );
    // 门禁 4：动作帧若 ArtMesh 全段写是性能热点。
    // 本工具只测静止帧；动作帧需另起 example（不在 P1-1 范围），按 C5 裁决
    // 应优先向 Ayagami 上游提交 dirty-uniform 优化。
    println!("  [4] 动作帧 ArtMesh 全段写：N/A（本工具仅测静止帧；");
    println!("       动作帧属后续 C5.2 任务范围；裁决 97-98 行已指明向上游提 PR）");

    println!("\n=== 总结 ===");
    let all_pass = s_create_clip == 0
        && s_render_artmesh == 0
        && s_artmesh_changed == 0
        && delta_textures == 0
        && delta_bind_groups == 0;
    println!(
        "C5 静止帧四门禁符合性：{}",
        if all_pass {
            "通过（门禁 1-3 全部满足；门禁 4 动作帧不在本工具范围）"
        } else {
            "不通过——见上"
        }
    );

    Ok(())
}
