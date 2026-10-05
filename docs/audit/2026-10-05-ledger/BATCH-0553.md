# BATCH-0553 落盘 · ModFactory 三个方法，两个带缺省

## 跑的命令（全部只读）
```
grep -n "pub trait ModFactory" -A 18 crates/live2d-ai-mod-system/src/factory.rs | grep -E "fn |trait "
sed -n '14,20p;28,32p;39,46p' crates/live2d-ai-mod-system/src/factory.rs
```

## 逐字（factory.rs:16-46）
```
:16 /// Mod 工厂（静态注册进 host 的 `AVAILABLE_MOD_FACTORIES`）。
:18 pub trait ModFactory: Send + Sync {
:19   /// 静态描述符。
:20   fn descriptor(&self) -> &'static ModDescriptor;
:28   ///
:29   /// 缺省 `None` = 沿用运行时注册（旧 Mod 不必改）；两者都提供[时以]
:30   /// 运行时注册为准（后者可携带动态字段）。
:31   fn settings_spec(&self) -> Option<crate::settings::ModSettingsSpec> { None }
:39   fn create(&self, services: ModServices, config: serde_json::Value)
:43       -> Result<Box<dyn ModRuntime>, ModError>;
:44 }
```

## 四个可核点
1. **三个方法**：`descriptor`（无缺省）· `settings_spec`（**有缺省 `None`**）· `create`（无缺省）
   => 与 B0490 核的「`dump_state` **不在 trait 里**、而它是 AGENTS 裁决留下的」**对照**：
   **那个裁决有结构性痕迹（少一个方法）**，而这里的缺省**没有**（三个方法都在）
2. **`settings_spec` 的缺省注释是一整段**（「缺省 `None` = 沿用运行时注册（**旧 Mod 不必改**）；
   两者都提供[时以]运行时注册为准（后者可携带动态字段）」）
   => **缺省**被写成**为了谁**（旧 Mod 不用改）+ **两者并存时谁说了算**（运行时）
   => 与 B0491 核的「每条禁令要带后果」**同族**、而这里给的是**迁移理由**
3. **`create` 收 `ModServices`（B0649 核过七字段）+ `config: Value`**
   => 而**它不收任何 host 引用** => 与 B0489 核的「`ModPanelContext` 里没有一个 HTTP client」**同一个判据**：
   **要什么都在这两个参数里、没有隐式依赖**
4. **`trait ModFactory: Send + Sync`** => 而 `create` 返回 `Box<dyn ModRuntime>`（**不带 `Send` 限定**）
   => **未核**：`ModRuntime` 是否要求 `Send`（若不要求、那 trait 的 `Send` 约束就落不到实例上）
   => 记为待核

## 未核
`ModRuntime` trait 的 supertrait 约束 · registry.rs · error.rs · 其余 9 个 mod crate
