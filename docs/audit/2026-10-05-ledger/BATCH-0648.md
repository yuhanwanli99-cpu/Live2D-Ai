# BATCH-0648 落盘（干净版）· mod-system 头注

## 跑的命令（全部只读）
```
git ls-files crates/live2d-ai-mod-system/ | wc -l      # => 11
sed -n '1,12p' crates/live2d-ai-mod-system/src/lib.rs
```

## 逐字（lib.rs:1-12）
```
:1  //! Live2D-Ai Mod 系统（节点 E E4）。
:3  //! # 定位
:5  //! 主仓库核心链路（TTS + LLM + Live2D + Web UI）通过本 crate 的 trait 接口
:6  //! 暴露给 Mod。**Mod 是静态编译模块**——编译进当前 binary，enable/dis[able] /
:7  //! restart / reload_config 是运行时开关；**不支持运行期安装新 crate**（要装新
:8  //! Mod 需重新构建应用）。
:10 //! # 设计要点（对齐 E0 边界裁决 ADR）
:12 //! - **ModFactory + ModRuntime**（工厂/实例分离）：factory.create(services, ...)
```

## 四个可核点
1. **11 个文件**（我账上写「8 个」**错**）=> 第三次记错分母
2. **「不支持运行期安装新 crate」** =>
   这正是 F-0644-01（P1）的**根源**：
   **用户之所以会往 mods.json 里写一个未知 id，是因为「装 Mod」这条路被写成了「编辑 JSON」**
   => 而写回又会抹掉它（B0646 已核）
3. 头注把**运行时能做什么**列全了（enable / disable / restart / reload_config）
   => 与 B0448 核的「三档降级」**同族**（**能力清单写在头上**）
4. 「工厂/实例分离」+「对齐 **E0 边界裁决 ADR**」= 设计点**带裁决编号**
   => 与 B0492 核的「三处引用 P2 / P6 / rc.2」**同族**

## 未核
mod-system 那 11 个文件的本体（只读了 lib.rs 头 12 行）· ModServices 的字段面（B0489 核过 ModPanelContext）·
其余 9 个 mod crate 本体（voice_input 的 gate.rs 已核）
