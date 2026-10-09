//! 纯数据：launch_mode / argv / 声明地址 / 引擎预检。**不碰进程、不碰 host**。
//!
//! 拆出来的理由（除体量外）：这些判据是「删掉实现就必须红」的那一半，
//! 它们必须是**纯函数**——可以被单测直接喂非法输入，不需要真的 spawn。
//!
//! # 2026-10-09（本轮）三处口径变更
//!
//! 1. **缺省 launch_mode = with_app**：Mod 已启用时，开机（Boot）与用户打开
//!    开关（Enable）都 spawn；Apply 与命令 launch 照旧。
//! 2. **空 program 不再等于「没有可执行文件」**：回落**本仓库内**的引擎脚本
//!    （编译期由 CARGO_MANIFEST_DIR 锚定），**禁止**回落到仓库外的
//!    /home/skystar/CosyVoice 3.0。
//! 3. **spawn 前预检**（[LaunchConfig::preflight]）：脚本不在、权重不在、
//!    解释器不在 → 返回 Err，宿主把 Mod 标 Failed。**不重试、不改地址、
//!    不回落云端、不把失败写成成功。**

use std::path::{Path, PathBuf};

use live2d_ai_mod_system::StartCause;

/// local-tts（CosyVoice 垫片）的缺省声明地址（**只进 state_json，不写 [tts]**）。
///
/// 这是 CosyVoice 垫片的文档地址（docs/architecture/cosyvoice3-tts-integration.md），
/// 供用户在界面上对照「我该把这个进程起在哪」。它**不是** spawn 失败后的回退地址——
/// 出声地址的唯一来源永远是 live2d-ai.toml 的 [tts]（见 docs/architecture/tts-is-core.md）。
pub const DEFAULT_BASE_URL: &str = "http://127.0.0.1:8080/v1";

/// local-tts-melo 的缺省声明地址：**刻意避开 8080**（那里的 CosyVoice 垫片
/// 可能已经在跑，两者抢端口只会让「谁在服务」变得不可知）。
pub const MELO_BASE_URL: &str = "http://127.0.0.1:8091/v1";

/// CosyVoice3 引擎的缺省程序：**本仓库内**的薄启动脚本（编译期绝对路径）。
pub const COSYVOICE_PROGRAM: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/engine/start.sh");
/// CosyVoice3 权重目录（**进 .gitignore**，由 engine/download.sh 下载）。
pub const COSYVOICE_WEIGHTS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/weights");

/// MeloTTS 引擎的缺省程序：**本仓库内**的薄启动脚本。
pub const MELO_PROGRAM: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/melo/start.sh");
/// MeloTTS 权重目录（config.json 普通入库；checkpoint.pth 走 Git LFS）。
pub const MELO_WEIGHTS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/melo/weights");

/// 一个「仓库内引擎」的静态配方：程序、权重、解释器、声明地址。
///
/// 放在这里而不是散在 lib.rs：预检是**纯函数**判据，必须能被单测直接喂
/// 不存在的路径（不需要真的 spawn、也不需要真的下权重）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngineSpec {
    /// 缺省 argv[0]（仓库内脚本，编译期绝对路径）。
    pub program: &'static str,
    /// 权重目录（仓库内）。
    pub weights_dir: &'static str,
    /// 权重目录里**必须存在**的文件名。
    pub required_files: &'static [&'static str],
    /// 权重目录必须至少有一个该后缀的文件（None = 不额外要求）。
    pub required_suffix: Option<&'static str>,
    /// 解释器相对引擎目录的路径（先查它，再查 PATH 上的 python3 / python）。
    pub interpreter: &'static str,
    /// 缺省声明地址（只展示）。
    pub base_url: &'static str,
    /// 缺文件时给用户的**可执行**提示（下载脚本 / LFS 拉取）。
    pub hint: &'static str,
}

/// CosyVoice3（FunAudioLLM/Fun-CosyVoice3-0.5B-2512）的配方。
pub const COSYVOICE_ENGINE: EngineSpec = EngineSpec {
    program: COSYVOICE_PROGRAM,
    weights_dir: COSYVOICE_WEIGHTS_DIR,
    // 权重文件名不做逐字断言（HF 布局会变）：**目录 + 至少一个 *.pt**。
    // 下载脚本（engine/download.sh）把权重平铺进这个目录。
    required_files: &[],
    required_suffix: Some("pt"),
    interpreter: ".venv/bin/python",
    base_url: DEFAULT_BASE_URL,
    hint: "先跑 crates/live2d-ai-mod-local-tts/engine/download.sh 下载 Fun-CosyVoice3-0.5B-2512 权重",
};

/// MeloTTS 中文（myshell-ai/MeloTTS-Chinese）的配方。
pub const MELO_ENGINE: EngineSpec = EngineSpec {
    program: MELO_PROGRAM,
    weights_dir: MELO_WEIGHTS_DIR,
    // 这两份是计划书点名的：config.json 普通入库，checkpoint.pth 走 Git LFS。
    required_files: &["config.json", "checkpoint.pth"],
    required_suffix: None,
    // 隐藏目录（.venv）：xtask 的 rust-ratio 不统计隐藏目录，第三方 Python 包
    // 也不会被算进第一方源码。
    interpreter: ".venv/bin/python",
    base_url: MELO_BASE_URL,
    hint: "仓库内 crates/live2d-ai-mod-local-tts/melo/weights/ 缺这两份权重（checkpoint.pth 走 Git LFS，先 git lfs pull）",
};

/// launch_mode 的两个取值（settings_spec 的 Select 选项，**值**是稳定契约）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchMode {
    /// 只在「保存并应用」（StartCause::Apply）与命令 launch 时 spawn。
    OnApply,
    /// **缺省**：随应用启动——Boot / Enable / Apply / 命令 launch 都 spawn。
    WithApp,
}

impl LaunchMode {
    /// Select 的稳定值（前端与 mods.json 里存的就是它）。
    pub const ON_APPLY: &'static str = "on_apply";
    /// 见 [Self::ON_APPLY]。
    pub const WITH_APP: &'static str = "with_app";
    /// 缺省值（键缺失 / null 时用它）。2026-10-09 起是 with_app。
    pub const DEFAULT: LaunchMode = LaunchMode::WithApp;

    /// 稳定值字符串。
    pub fn as_str(self) -> &'static str {
        match self {
            LaunchMode::OnApply => Self::ON_APPLY,
            LaunchMode::WithApp => Self::WITH_APP,
        }
    }

    /// 解析；**未知值返回 None**（调用方必须报错，不得折成缺省）。
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            Self::ON_APPLY => Some(LaunchMode::OnApply),
            Self::WITH_APP => Some(LaunchMode::WithApp),
            _ => None,
        }
    }

    /// 这一次 start（给定原因）要不要 spawn。
    pub fn spawns_on(self, cause: StartCause) -> bool {
        match self {
            LaunchMode::WithApp => true,
            LaunchMode::OnApply => matches!(cause, StartCause::Apply),
        }
    }
}

/// 解析后的有效配置。
///
/// args_json **保留原始字符串**：非法内容不能在这里就吞掉——按口径它要留到
/// 「这一次真要 spawn」时才返回 Err。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchConfig {
    /// 拉起时机。
    pub mode: LaunchMode,
    /// argv[0]，按路径原样执行（**不**经过 shell）。
    pub program: String,
    /// JSON 字符串数组的原文（缺键 = []）。
    pub args_json: String,
    /// 工作目录（空 = 不设 current_dir）。
    pub workdir: String,
    /// 声明地址（只展示）。
    pub base_url: String,
    /// 仓库内引擎配方；Some = 走内置脚本（预检生效）。
    ///
    /// **用户显式给了 program 时为 None**：那是用户自己的程序，
    /// 我们不知道它的权重与解释器在哪，也就不替它做预检（spawn 自己会报错）。
    pub engine: Option<EngineSpec>,
}

impl LaunchConfig {
    /// 按**指定引擎**解析（local-tts = [COSYVOICE_ENGINE]，
    /// local-tts-melo = [MELO_ENGINE]）。
    ///
    /// 唯一会在这里失败的是 launch_mode：非法值必须在 create 就 Err
    ///（禁止折成缺省），因为「用户以为填了 with_app」而实际按 on_apply 跑，
    /// 是最难查的一类静默偏差。
    pub fn from_json_with_engine(
        config: &serde_json::Value,
        engine: EngineSpec,
    ) -> Result<Self, String> {
        let mode = match config.get("launch_mode") {
            None | Some(serde_json::Value::Null) => LaunchMode::DEFAULT,
            Some(serde_json::Value::String(raw)) => LaunchMode::parse(raw).ok_or_else(|| {
                format!(
                    "非法 launch_mode：{raw:?}（只认 {} / {}）",
                    LaunchMode::ON_APPLY,
                    LaunchMode::WITH_APP
                )
            })?,
            Some(other) => {
                return Err(format!("launch_mode 必须是字符串，实际是 {other}"));
            }
        };
        let program = string_field(config, "program");
        let user_program = !program.trim().is_empty();
        let base_url = string_field(config, "base_url");
        Ok(Self {
            mode,
            program: if user_program {
                program
            } else {
                engine.program.to_string()
            },
            args_json: args_json_field(config),
            workdir: string_field(config, "workdir"),
            base_url: if base_url.trim().is_empty() {
                engine.base_url.to_string()
            } else {
                base_url
            },
            // 用户自己给了程序 = 不替他预检（见字段头注）。
            engine: if user_program { None } else { Some(engine) },
        })
    }

    /// 按 CosyVoice3 引擎解析（等价于 [Self::from_json_with_engine] +
    /// [COSYVOICE_ENGINE]；保留这个名字给既有调用方与单测）。
    pub fn from_json(config: &serde_json::Value) -> Result<Self, String> {
        Self::from_json_with_engine(config, COSYVOICE_ENGINE)
    }

    /// 组装 argv（含 argv[0]）。
    ///
    /// args_json 不合法时**只在这时**报错（口径：坏参数留到「这次真要 spawn」）。
    pub fn argv(&self) -> Result<Vec<String>, String> {
        if self.program.trim().is_empty() {
            return Err("program 为空：没有可执行文件（argv[0]），无法拉起".to_string());
        }
        let args = parse_args_json(&self.args_json)?;
        let mut argv = Vec::with_capacity(args.len() + 1);
        argv.push(self.program.clone());
        argv.extend(args);
        Ok(argv)
    }

    /// 非空才给 current_dir。**不创建目录**（目录不存在 → spawn 自己报错，
    /// 这是 std::process::Command 的既有行为，不在这里预检、也不 mkdir）。
    pub fn workdir_path(&self) -> Option<PathBuf> {
        let trimmed = self.workdir.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(PathBuf::from(trimmed))
        }
    }

    /// **spawn 前的预检**（口径：脚本不在 / 权重不在 / 解释器不在 → Err）。
    ///
    /// 只对内置引擎生效（engine.is_some()）；用户自带程序不做预检。
    /// 失败消息**必须可执行**：带具体路径与「下一步做什么」。
    pub fn preflight(&self) -> Result<(), String> {
        let Some(engine) = self.engine else {
            return Ok(());
        };
        preflight_engine(
            Path::new(&self.program),
            Path::new(engine.weights_dir),
            engine.required_files,
            engine.required_suffix,
            engine.interpreter,
            engine.hint,
        )
    }
}

/// 预检本体（**纯函数 + 真实路径**，所以单测能直接喂临时目录，不必依赖
/// 仓库里到底有没有下载权重）。
///
/// 三条判据按「用户下一步该做什么」排序：脚本 → 权重 → 解释器。
/// 都只读文件系统，**不执行任何东西**。
pub fn preflight_engine(
    program: &Path,
    weights_dir: &Path,
    required_files: &[&str],
    required_suffix: Option<&str>,
    interpreter_rel: &str,
    hint: &str,
) -> Result<(), String> {
    if !program.is_file() {
        return Err(format!("启动脚本不存在：{}。{hint}", program.display()));
    }
    if !weights_dir.is_dir() {
        return Err(format!("权重目录不存在：{}。{hint}", weights_dir.display()));
    }
    for name in required_files {
        let file = weights_dir.join(name);
        if !file.is_file() {
            return Err(format!("权重文件缺失：{}。{hint}", file.display()));
        }
    }
    if let Some(suffix) = required_suffix
        && !has_file_with_suffix(weights_dir, suffix)
    {
        return Err(format!(
            "权重目录里没有 *.{} 文件：{}。{hint}",
            suffix,
            weights_dir.display()
        ));
    }
    let engine_dir = program.parent().unwrap_or_else(|| Path::new("."));
    if find_interpreter(&engine_dir.join(interpreter_rel)).is_none() {
        return Err(format!(
            "找不到 Python 解释器（{interpreter_rel} 或 PATH 上的 python3/python）：{hint}"
        ));
    }
    Ok(())
}

/// 目录里有没有后缀为 suffix 的文件（只查顶层；下载脚本把权重平铺在这一层）。
fn has_file_with_suffix(dir: &Path, suffix: &str) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    entries.flatten().any(|e| {
        e.path()
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.eq_ignore_ascii_case(suffix))
    })
}

/// 先查引擎自己的 venv，再查 PATH 上的 python3 / python。
///
/// 顺序是刻意的：venv 里那份才是**装了 MeloTTS / CosyVoice 依赖**的那一份；
/// 系统 Python 只在「venv 还没建」（MeloTTS 首次启用的引导路径）时兜底。
fn find_interpreter(venv_python: &Path) -> Option<PathBuf> {
    if venv_python.is_file() {
        return Some(venv_python.to_path_buf());
    }
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        for name in ["python3", "python"] {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// args_json → argv（**不含** argv[0]）。
///
/// - 空白 / 缺键 → []（表单里「清空」等价于「没有参数」）；
/// - 必须是 JSON **字符串**数组：带空格的一项仍是一个元素（这正是用 JSON
///   而不是「按空格切」的原因）；
/// - 非法内容返回带原文的错误，**不吞**。
pub fn parse_args_json(raw: &str) -> Result<Vec<String>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let value: serde_json::Value =
        serde_json::from_str(trimmed).map_err(|e| format!("args_json 不是合法 JSON：{e}"))?;
    let array = value.as_array().ok_or_else(|| {
        "args_json 必须是 JSON 字符串数组（如 [--port, 8080] 的 JSON 形态）".to_string()
    })?;
    let mut out = Vec::with_capacity(array.len());
    for (index, item) in array.iter().enumerate() {
        let s = item
            .as_str()
            .ok_or_else(|| format!("args_json 第 {} 项不是字符串：{item}", index + 1))?;
        out.push(s.to_string());
    }
    Ok(out)
}

/// 取一个字符串字段（缺键 / 非字符串 → 空串）。
fn string_field(config: &serde_json::Value, key: &str) -> String {
    config
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// 取 args_json 原文（缺键 / null → []；真数组也接受，非法形态留给 spawn 报错）。
fn args_json_field(config: &serde_json::Value) -> String {
    match config.get("args_json") {
        None | Some(serde_json::Value::Null) => "[]".to_string(),
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
    }
}
