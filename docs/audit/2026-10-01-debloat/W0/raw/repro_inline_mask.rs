// verifier 独立复现：用 #[path] 直接挂仓库里的 measure.rs（不改仓库任何文件）
#[path = "/home/skystar/Live2D-Ai-fe/xtask/src/code_stats/measure.rs"]
mod measure;
use measure::{inline_test_lines, physical_lines};

fn main() {
    let text = "pub fn a() {}\n#[cfg(test)]\nmod tests_x;\nuse serde::{Deserialize, Serialize};\npub fn b() {}\n";
    println!("[复现案例] physical_lines = {}", physical_lines(text));
    println!("[复现案例] inline_test_lines 实得 = {}   （measure.rs 文档口径：无花括号条目应吃到文件末尾 = 4）", inline_test_lines(text));
    let real = std::fs::read_to_string("/home/skystar/Live2D-Ai-fe/crates/live2d-ai-runtime/src/settings.rs").unwrap();
    println!("[真实文件] settings.rs physical={} inline实得={} （34-37 行两条无花括号 cfg(test) mod；文档口径 = 793）",
        physical_lines(&real), inline_test_lines(&real));
    let perf = std::fs::read_to_string("/home/skystar/Live2D-Ai-fe/crates/live2d-ai-runtime/src/performance/mod.rs").unwrap();
    println!("[真实文件] performance/mod.rs physical={} inline实得={}", physical_lines(&perf), inline_test_lines(&perf));
}
