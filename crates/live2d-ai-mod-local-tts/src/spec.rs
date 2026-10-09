//! settings_spec（纯数据 schema，前端通用表单渲染）。
//!
//! 本轮**不加专用面板**（口径）：因此这里的 label 是用户能看到的**唯一**说明。
//! 选项值（on_apply / with_app）是稳定契约，标签才是中文。
//!
//! 2026-10-09：**按工厂参数化**——同一个 crate 里有两个引擎
//! （CosyVoice3 与 MeloTTS），各自的 title / 缺省 program / 缺省声明地址
//! 都不同；title 逐字取 ModDescriptor.name。

use live2d_ai_mod_system::{ModSettingField, ModSettingsSpec, SelectOption};

use crate::config::{EngineSpec, LaunchMode};

/// 某个 Mod 的设置 schema。
///
/// **静态提供**（ModFactory::settings_spec）：这些 Mod 缺省停用，用户必须能
/// 「先填好配置、再按启用」——不能为了拿 schema 去跑 start（那会 spawn）。
pub fn settings_spec(descriptor: &crate::ModDescriptor, engine: EngineSpec) -> ModSettingsSpec {
    ModSettingsSpec {
        mod_id: descriptor.id.to_string(),
        title: descriptor.name.to_string(),
        version: 1,
        fields: vec![
            ModSettingField::Select {
                key: "launch_mode".to_string(),
                label: "拉起方式".to_string(),
                options: vec![
                    SelectOption {
                        value: LaunchMode::WITH_APP.to_string(),
                        label: "随应用启动拉起（缺省）".to_string(),
                    },
                    SelectOption {
                        value: LaunchMode::ON_APPLY.to_string(),
                        label: "保存并应用时拉起".to_string(),
                    },
                ],
                // 缺省写进表单：键缺失时也与服务端行为一致（with_app）。
                default: Some(LaunchMode::DEFAULT.as_str().to_string()),
            },
            ModSettingField::String {
                key: "program".to_string(),
                label: "程序（argv0，按路径原样执行；缺省 = 仓库内引擎脚本）".to_string(),
                secret: false,
                default: Some(engine.program.to_string()),
            },
            ModSettingField::String {
                key: "args_json".to_string(),
                label: "参数（JSON 字符串数组）".to_string(),
                secret: false,
                default: Some("[]".to_string()),
            },
            ModSettingField::String {
                key: "workdir".to_string(),
                label: "工作目录（可空）".to_string(),
                secret: false,
                default: None,
            },
            ModSettingField::String {
                key: "base_url".to_string(),
                label: "声明地址（只展示，不写入语音设置）".to_string(),
                secret: false,
                default: Some(engine.base_url.to_string()),
            },
        ],
    }
}
