# BATCH-0645 · F-0644-01 落盘（未知 id 静默忽略 · P2）

## 结果
- FINDINGS.md 新增 **F-0644-01 (P2)** —— mod_registry.rs:188-192 只遍历工厂、
  parse_mod_config 缺 id 时回落 (false, {}) 且不记日志
  ⇒ mods.json 里写一个不存在的 Mod id 被静默丢掉，无任何提示
- 用户可察觉后果：改配置没反应（而 mods.json 是用户可手改的文件）
- **缺省 false ⇒ 不会「莫名启动」，故 P2 而非 P1**
- 反证条款已写：若那属「归档仓专属」则表述要收窄

## 未核
mods.json 写回路径是否保留未知键（若是 ⇒ 用户手写的东西会被写回时抹掉，
那就要升级）· docs/verification/ignition-core-loop.md 其余 20 个文件 · 其余 9 个 mod crate
