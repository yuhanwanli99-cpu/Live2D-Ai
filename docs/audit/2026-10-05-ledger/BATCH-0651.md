# BATCH-0651 落盘 (极简版)

## api_version 门禁 (mod_registry.rs:306-320)
```
:306 fn start_one(&mut self, id: &'static str) {
:309   if factory.descriptor().api_version != MOD_API_VERSION {
:310     let msg = format!("Mod {id} 声明 api_version={} 与宿主 MOD_API_VERSION={} 不兼容", ..);
:314     tracing::warn!(target: "mod", "{msg}");
:315-318   e.status = ModStatus::Failed { message: msg.clone() }; e.last_error = Some(msg);
:319     return;
:320   }
```
MOD_API_VERSION = 1 (mod-system/src/lib.rs:55) ; 注释 :308 「不兼容 -> Failed，主链不崩」

## 三个可核点
1. 判据是 **!=**（严格相等），不是 >=。
   => 未来 Mod 声明 api_version=2 也会被拒（即便向后兼容）。与 B0623 核的严同向。
2. 拒绝路径有 **三件事**：`tracing::warn!` + `ModStatus::Failed{message}` + `last_error`，
   而 `message` 里带**两个数**（声明值 + 宿主值）=> 排障时不用猜是哪一版。
   => 与 B0500「一条禁令要带后果」同族：后果 = 可见的状态 + 可见的日志 + 带数字的文案。
3. `start_one` 是**逐个 Mod 启动**的入口，`return` 只跳过这一个 => 主链不崩。

## 未核
descriptor.rs 的 api_version 门禁判据（AGENTS 记「不兼容 -> Failed 不崩」已核）·
mod-system 本体其余文件 · 其余 9 个 mod crate 本体 · shared/ 其余 8 个 json
