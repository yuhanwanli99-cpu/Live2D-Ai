# BATCH-0643 落盘 · 调和的形状：**遍历工厂**、逐个查 manifest、缺省 false

## 跑的命令（全部只读）
```
grep -n "pub fn new" -A 20 crates/live2d-ai-desktop/src/mod_registry.rs | grep -nE "enabled|for |manifest"
grep -n "fn parse_mod_config" -A 16 crates/live2d-ai-desktop/src/mod_registry.rs
```

（**注**：`mod_registry.rs` 在 `src/` 下、**不在 `src/web_api/`** —— B0440 第一次救了我）

## 逐行
```
:181        manifest: &serde_json::Value,
:188        for factory in factories {
:189            let desc = factory.descriptor();
:190            let (enabled, config) = parse_mod_config(manifest, desc.id);
:191            let mut reg = RegisteredMod::new(*factory);
:192            reg.enabled = enabled;
:688  fn parse_mod_config(manifest: &Value, id: &'static str) -> (bool, Value) {
:689     let Some(obj) = manifest.get("mods").and_then(as_object).and_then(|m| m.get(id)) else {
:690         return (false, Value::Object(Default::default()));
        };
:692     let enabled = obj.get("enabled").and_then(as_bool).unwrap_or(false);
:693     let config  = obj.get("config").cloned().unwrap_or_else(|| Value::Object(..));
:694     (enabled, config)
```

## 四个可核点
1. => **遍历的是「工厂」不是「manifest」** ⇒ **manifest 里写着一个工厂没有的 id，会被完全忽略**
   ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒
   ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
   ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
   ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
   ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
   ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
   ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
   ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
   ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
   ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
   ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
   ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
   ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
   ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
   ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
   ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
   ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒ Eqs.
