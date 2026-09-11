# Live2D-Ai PC — Settings UI & Backend Call-Chain Audit (findings + fix plan)

Scope: `renderer/src/settings-ui.ts`, `src/open_llm_vtuber/routes.py`, `config_manager/*`, `server.py`, `proxy_handler.py`, `websocket_handler.py`, `message_handler.py`, `run_server.py`, `_start_server.py`, `service_context.py`, plus `tests/` and renderer tests.
Status: analysis only — **no code was changed.**

---

## 0. The traced call chain (as actually built)

1. `run_server.py:113` `read_yaml("conf.yaml")` (env-var substitution) → persona injected → `Config(**config_dict)` → `WebSocketServer(config)` (`server.py:145-150`, `default_context_cache = ServiceContext()`).
2. `server.py:89-98` mounts `init_config_routes(config_path="conf.yaml", default_context_cache=self.default_context_cache)` **before** the static catch-all mount, so `/api/*` wins over static.
3. Settings UI (`settings-ui.ts:819` `open()`) does `GET /api/config` (`routes.py:436`, response built by `_get_common` at `routes.py:328`) then `GET /api/config/tree` (`routes.py:718`).
4. Save: `settings-ui.ts:910` `save(apply)` → `POST /api/config` (`routes.py:445`): mutates the ruamel raw dict → `AppConfig(**candidate)` validation (`routes.py:562-588`) → `_save_yaml(data)` (`routes.py:590`; **non-atomic**) → write `.env` (`routes.py:592-629`) → save `shared/persona.yaml` (`routes.py:631-649`) → if `apply`, `await default_context_cache.load_from_config(new_config)` (`routes.py:651-657`).
5. Runtime reads config at engine-construction time only (`OpenAICompatibleLLM.__init__`, `EdgeTTSEngine(voice)`, `TTS/ASR/VADFactory(kwargs)`, `ServiceContext.load_from_config`). Engine objects are constructed once and cached; per-session contexts deep-copy config but **share the engine object references** (`websocket_handler.py:183-202`).

Key conclusion: **`GET /api/config` returns arrays and `POST /api/config` consumes maps keyed by provider id; both sides match (by design), but the two shapes are asymmetric and nothing guards it.** The real defects are in atomicity, the apply/hot-reload path, empty-value handling, port dead-config, and log-level round-trip.

---

## A. Settings UI logic findings (`renderer/src/settings-ui.ts`)

### A1. [medium] `log_level` is never loaded, but is always sent back → every "保存全部" silently resets the log level to INFO
- `settings-ui.ts:779-783` (`renderSystem` sets character_name/port/fps only), `:785-817` (`populate` never touches `.settings-log-level`), `:906` (`collectPayload` always reads the select), `:910-927` (`save` always POSTs it).
```ts
const renderSystem = (): void => {
  overlay.querySelector<HTMLInputElement>('.settings-character-name')!.value = currentConfig.character_name || '';
  overlay.querySelector<HTMLInputElement>('.settings-port')!.value = String(currentConfig.port ?? 12393);
  overlay.querySelector<HTMLInputElement>('.settings-fps')!.value = String(currentConfig.live2d_fps ?? 120);
};
...
log_level: overlay.querySelector<HTMLSelectElement>('.settings-log-level')!.value,
```
- The option list is hardcoded (`settings-ui.ts:256-260`) with `INFO` the first (HTML default). `GET /api/config` returns no `log_level` field (`routes.py:414-433`), so the dropdown can never reflect the real `.env` value (`LIVE2DAI_LOG_LEVEL`).
- Backend writes it unconditionally: `routes.py:594-595` `if payload.get("log_level"): api_keys["LIVE2DAI_LOG_LEVEL"] = str(payload["log_level"])`.
- Symptom: user with `DEBUG` in `.env` opens settings and hits "保存全部" (or "保存并应用") → level silently reset to `INFO`; a "no-change" save still rewrites it.

### A2. [medium] Failed `/api/config` load leaves the form saveable → a transient failure can wipe the config
- `settings-ui.ts:819-836` (on catch only sets status text; never disables the footer buttons), `:910-927` (buttons always live), `:892-893` (`port: Number(...?.value)` → `Number('')===0`).
```ts
} catch (err) {
  statusEl.textContent = `加载失败: ${(err as Error).message}`;
}
```
- Symptom: if the startup `GET /api/config` fails, selects are empty; pressing "保存全部" POSTs `port: 0`, `llm_provider: ''`, empty configs (see also B2/B3 for how the backend then skips/keeps values).

### A3. [medium] `/api/config/tree` load failure is silent and the YAML box keeps a stale/empty value → "保存设定树" can clobber the real tree
- `settings-ui.ts:827-832`:
```ts
if (treeRes.ok && treeData.yaml) {
  configTreeEl.value = treeData.yaml;
}
statusEl.textContent = '';
```
- `saveTree()` (`settings-ui.ts:1059-1071`) POSTs whatever is in `configTreeEl.value`; backend `update_config_tree` (`routes.py:729-740`) does a `safe_load` syntax check then **overwrites conf.yaml** with no `AppConfig` validation. A stale/empty box → valid-but-broken or empty YAML → next startup `Config(**config_dict)` fails.

### A4. [medium] Provider select "change" only marks `touched` — no model discovery, no auto-apply; top-level and per-card state desync
- `settings-ui.ts:1075-1080`:
```ts
overlay.querySelector<HTMLSelectElement>('.settings-llm')!.addEventListener('change', (e) => {
  markLlmTouched((e.target as HTMLSelectElement).value);
});
overlay.querySelector<HTMLSelectElement>('.settings-tts')!.addEventListener('change', (e) => {
  markTtsTouched((e.target as HTMLSelectElement).value);
});
```
- The ASR select has **no** change listener at all. There is no code path from provider `<select>` to `discoverModels()` (`settings-ui.ts:686`). The model dropdown only refreshes on explicit "获取可用模型", or via `autoDiscoverConfiguredModels()` (`:724`) at open (which only covers `p.configured`, i.e., providers already in `llm_configs`).
- Symptom: switching to an unconfigured provider shows no models until the user manually discovers; "auto-apply" of a provider choice does not exist (design gap vs. the product intent of the task's "provider selection auto-apply").

### A5. [medium] "全部检查 / 列出可用模型" never lists models
- `settings-ui.ts:654-683` (`checkAll`) declares `models?: string[]` in the response type but never writes them into any `.llm-model` select; only an aggregate `✅ LLM x/y 可用` is shown.
- Symptom: the button promises to "列出可用模型" but only prints counts.

### A6. [medium/low] Double-submit & last-writer-wins on a single shared status element, no busy/disable handling
- Buttons are never disabled during async work (`settings-ui.ts:1083-1102`), so a user can fire two concurrent `POST /api/config` (save + apply), `POST /api/test-llm`, etc. All async paths write one shared `statusEl`/`modelStatusEl`: `checkAll:654`, `discoverModels:686`, `autoDiscover:724`, `save:910`, `testLlm:929`, `testTts:961`, `scanModels:1027`, `selectModel:1043`, `saveTree:1059`.
- Symptom: the final message can describe a *different* request than the user last clicked (e.g. "已保存并热重载" for an apply=false save, or an old "测试失败" overwriting a save success).

### A7. [low] `fillSelect()` has no "no selection" placeholder → invalid/empty current selects the first provider silently
- `settings-ui.ts:302-316` (only appends `values`, `opt.selected = v === current`, never appends an empty option) + `populate:785-807`.
- Symptom: if `GET /api/config` returns `llm_provider: ''` (e.g. missing key) while `llm_providers` is non-empty, the browser selects the first id and the next save persists that wrong provider.

### A8. [low] `switchEdgeTts()` reports success even when `edge_tts` is not in the select/editor
- `settings-ui.ts:1008-1024`: sets `select.value = 'edge_tts'` (a no-op if no such option exists), `markTtsTouched('edge_tts')` (no-op if no editor), then unconditionally prints `'⚡ 已切换低延迟 Edge TTS…'`.

### A9. [low] `selectModel()` never refreshes the model list → "默认" badge stays on the old model until reopen
- `settings-ui.ts:1043-1055` (sets `modelStatusEl` only; no `renderModels()` call, unlike `scanModels` at `:1036`).

### A10. [design note — confirmed NOT a bug] POST `llm_configs`/`tts_configs` are objects keyed by provider id while GET returns arrays
- `settings-ui.ts:838-856` builds `Record<pid, {...}>`; `routes.py:480-482` iterates them as `llm_overrides.items()`; `routes.py:349-414` returns `llm_provider_list` as an array with `id`. Both directions intentionally use different shapes and they match. This is fragile (a typo in one key silently drops a provider's config) but not currently a bug.
- Same for omitting `llm_providers/tts_providers/asr_providers` from the POST payload — `GET` reconstructs them (`routes.py:368-414`) so nothing is lost on save.

### A11. [low] `Audio.play()` return is unhandled + preview URL construction is Windows-hacky
- `settings-ui.ts:997-1002` `new Audio(audioUrl); void audio.play();`. Autoplay-denial rejection is unhandled, and `audioPath.replace(/\\/g,'/')` then `/${path}` assumes a relative/`/`-prefixed path returned by the TTS engine; an absolute Windows path would 404.

---

## B. Backend / config call-chain and runtime findings

### B1. [high] Save is not atomic and the write order allows partial write on failure
- `routes.py:207-212`:
```python
def _save_yaml(data):
    yaml = YAML(typ="rt")
    yaml.preserve_quotes = True
    yaml.allow_unicode = True
    with open(config_abs, "w", encoding="utf-8") as f:
        yaml.dump(data, f)
```
- `conf.yaml` is written at `routes.py:590` **before** `.env` (`:592-629`) and `persona.yaml` (`:631-649`). If the `.env` or persona write then raises (disk full, permission), the endpoint returns a 500 "保存失败" but `conf.yaml` was already modified — the user's config is silently half-applied. No temp-file+`os.replace` and no file lock anywhere (`grep` confirms; the tests subagent verified `os.replace`/`tempfile` are absent from all save paths, including `config_manager/utils.py:107-124`).
- Also concurrent `POST /api/config` requests can interleave read-modify-write on `_load_yaml()`/`_save_yaml()` with no lock.

### B2. [high] Empty-string and zero values are silently dropped → settings can never be cleared, and `port: 0` is impossible to set intentionally but easy to send by accident
- `routes.py:455-477`:
```python
if "llm_provider" in payload and payload["llm_provider"]:
    basic["llm_provider"] = payload["llm_provider"]
...
if "character_name" in payload and payload["character_name"]:
    cc["character_name"] = payload["character_name"]
if "port" in payload and payload["port"]:
    data.setdefault("system_config", {})["port"] = int(payload["port"])
if "live2d_fps" in payload and payload["live2d_fps"]:
    data.setdefault("system_config", {})["live2d_fps"] = int(payload["live2d_fps"])
```
- `routes.py:494-496` and `:529-531`:
```python
for field in ("model", "base_url", "temperature", "max_tokens", "models"):
    if field in override and override[field] not in (None, ""):
        target[field] = override[field]
```
and for persona `:644-648` the same `not in (None, "")` filter.
- Symptoms:
  - Clearing "system prompt"/"character name"/"role"/"model" in the UI is ignored → old value stays forever.
  - Frontend sends `port: Number('') === 0` when the field is cleared → falsy → backend keeps the old port while UI showed 0 (A2 makes this plausible during a failed load).
  - `max_tokens: Number(...) || undefined` (`settings-ui.ts:853,902`) means an empty box is omitted from JSON → backend interprets "keep old" → a max_token can never be removed.

### B3. [high] "保存并应用 / hot reload" only reloads the shared `default_context_cache`; already-connected sessions keep old engines AND the reload snapshot is internally inconsistent
- `websocket_handler.py:183-202` each session deep-copies config but receives the **same** engine object references:
```python
await session_service_context.load_cache(
    config=self.default_context_cache.config.model_copy(deep=True),
    system_config=self.default_context_cache.system_config.model_copy(deep=True),
    character_config=self.default_context_cache.character_config.model_copy(deep=True),
    live2d_model=self.default_context_cache.live2d_model,
    asr_engine=self.default_context_cache.asr_engine,
    tts_engine=self.default_context_cache.tts_engine,
    ...
    agent_engine=self.default_context_cache.agent_engine,
)
```
- `routes.py:651-657`:
```python
if payload.get("apply") and default_context_cache is not None:
    try:
        new_config = AppConfig(**_config_dict_for_reload())
        await default_context_cache.load_from_config(new_config)
        return JSONResponse({"ok": True, "requires_restart": False, "applied": True})
```
- `load_from_config` (`service_context.py:265-308`) replaces the shared context's config/engines, but **nothing re-syncs `WebSocketHandler.client_contexts`**, so an existing client keeps the old `agent_engine`/`tts_engine`/`asr_engine` objects it was handed at connect time. UI still reports `applied: true`.
- Ordering bug inside the reload: `load_from_config` runs `init_agent(...)` at `service_context.py:301-304` **before** reassigning `self.system_config`/`self.character_config` at `:306-308`, so the re-created agent is built with the *previous* `system_config` (tool_prompts/host/port/proxy) and previous `character_config.avatar` (`init_agent` uses `self.system_config.model_dump()` and `self.character_config.avatar` at `service_context.py:500-501`), while `init_live2d`/`init_tts`/`init_asr` already ran with the new config.
- Symptom: PC client (one long-lived WS) changes voice/persona/model + "保存并应用" → nothing changes live until reconnect; and a reconnecting client may get a new Live2D model with old system-prompt plumbing.

### B4. [medium] `.env` keys saved by the API are never visible to the running process → UI checks (fresh file) can say "OK" while the running LLM uses the stale key
- `.env` write: `routes.py:628-629` (via `_env_value` which re-reads the file: `routes.py:311-323`). But `run_server.py:21` `load_dotenv(...)` runs **once** at startup, and `config_manager/utils.py:44` substitutes `${VAR}` via `os.getenv` (stale process env). The apply path builds `new_config = AppConfig(**_config_dict_for_reload())` (`routes.py:654`) → `read_yaml` → `os.getenv`.
- Symptom: change an API key + "保存并应用" → `providers/check`/`test-llm` (which read the fresh file, `routes.py:1037-1040`) succeed, but the running engine keeps the old key until restart.

### B5. [medium] `system_config.port` is dead config — both launchers hardcode 12393
- `run_server.py:145` `port=12393,` and `_start_server.py:27` `port=12393,`. The settings route saves/validates/reloads `system_config.port` (`routes.py:472-473`), but the uvicorn listener never reads it. `server.py:121-123` builds the proxy URL from the (config) port only — so raising the port in settings can make `/proxy-ws` point at a dead port while the API keeps listening on 12393.

### B6. [medium] `_start_server.py` is a divergent launcher: no ${VAR} substitution, no persona injection, no MCP path resolution
- `_start_server.py:17-19`:
```python
with open("conf.yaml", "r", encoding="utf-8") as f:
    config_dict = yaml.safe_load(f)
config = Config(**config_dict)
```
- vs `run_server.py:113-136` (`read_yaml` substitution + persona load + model overrides + MCP path resolve). If a user starts with `_start_server.py`, `${DEEPSEEK_API_KEY}` placeholders reach the client literally, persona is missing, and MCP paths are wrong.

### B7. [medium] `handle_disconnect` pops the context before looking it up → `ServiceContext.close()` is dead code, resource leak on every disconnect
- `websocket_handler.py:302-314`:
```python
self.client_connections.pop(client_uid, None)
self.client_contexts.pop(client_uid, None)
self.received_data_buffers.pop(client_uid, None)
...
context = self.client_contexts.get(client_uid)   # always None
if context:
    await context.close()
```
- `ServiceContext.close()` (which closes MCPClient/agent/vision, `service_context.py:206-218`) never runs; the failed-connection path `_cleanup_failed_connection` similarly never calls `close()`.

### B8. [medium] Engine replace-on-reload never disposes the old engine
- `service_context.py:395-414` (`init_asr`/`init_tts` overwrite `self.asr_engine`/`self.tts_engine` without closing), `:477-514` (`init_agent` reassigns `self.agent_engine` without closing). TTS/ASR clients may hold websockets/HTTP sessions; repeated apply/switch leaks handles. `init_vision` early-returns if a handler exists (`service_context.py:434-436`) so vision model/interval can never be hot-changed.

### B9. [medium] Router/endpoint gaps: invalid/missing data handling
- `GET /api/config`: returns `llm_provider: basic.get('llm_provider','')` (`routes.py:360`) — if conf omits it (or a custom provider is current and not in `LLM_DEFAULTS`/`llm_configs`), the UI select gets an empty/unknown current value (A7).
- `POST /api/config` validation rejects the save when `shared/persona.yaml` is missing/empty: `_apply_persona_to_data` only sets `cc["persona_prompt"]` if `persona.get("system_prompt")` truthy (`routes.py:265-267`), and `CharacterConfig.persona_prompt` is a required field (`config_manager/character.py:22`) — so `AppConfig(**candidate)` raises `Field required` and the endpoint returns 400 "配置校验失败" for a legitimately-empty persona file.
- `test-llm`/`discover-models`/`check_providers` hardcode OpenAI-shaped `GET /models` + `POST /chat/completions` with `Bearer` auth for **every** provider (`routes.py:846-861`, `:915-961`, `:1016-1040`), while the runtime engine layer has provider-specific protocols (Anthropic `/v1/messages`, Ollama `/api/chat`, etc.). "检查通过" can diverge from what the actual agent does. (Frontend impact: `testLlm`/`checkAll` may report success for a provider the live agent cannot use.)

### B10. [medium] `update_config_tree` overwrites conf.yaml after only a YAML-syntax check — no `AppConfig` validation
- `routes.py:733-736`:
```python
_yaml.safe_load(raw)  # 语法校验，失败会抛异常
with open(config_abs, "w", encoding="utf-8") as f:
    f.write(raw)
```
- A "valid YAML, broken config" tree (removed `character_config`, wrong `tts_model`, `port: "abc"`, missing `persona_prompt`) is written; the server only fails on next startup (and A3 shows the UI can submit an empty/stale tree).

### B11. [low/design] `models/scan` scans `os.getcwd()/live2d-models` while `models/select` writes `character_config.live2d_model_name`/`conf_name` to conf.yaml
- `routes.py:756-757` (`live2d_dir = os.path.join(os.getcwd(), "live2d-models")`), `:806-810` (`cc["live2d_model_name"] = model_id; cc["conf_name"] = model_id`). Model switch persists to conf but is only honored at next start (UI says "重启后生效"); no live `set-model-and-conf` push exists from this route (compare `service_context.handle_config_switch`, which does push `set-model-and-conf` over WS at `service_context.py:594-603`).

### B12. [medium/design] Proxy can stall for the rest of the session when a playback-ack times out
- `conversations/conversation_utils.py:179-195`: after `tts_manager.task_list` completes it waits up to 5 s for `frontend-playback-complete`; on timeout it does:
```python
if not response:
    logger.warning(f"No playback completion response from {client_uid}")
    return
```
which returns **before** `send_conversation_end_signal(...)` (`conversation_utils.py:202, 208-219` emits `{"type": "control", "text": "conversation-chain-end"}`). `proxy_handler.py:224-229` only resets `ProxyMessageQueue.conversation_active` on that control message (or an interrupt at `:129-136`), so the queue stays gated and queued `text-input` (`:125-127`) is never forwarded. Note `message_handler.py:35-54` registers the wait key with `request_id=None`, so a frontend that sends a `request_id` on `frontend-playback-complete` would never match.

---

## Schema ↔ YAML ↔ frontend cross-reference (config_manager; firsthand read)

| Key layer | YAML (`conf.yaml`) | Pydantic (config_manager) | UI (settings-ui.ts) | Verdict |
|---|---|---|---|---|
| `system_config.port` | `port: 12393` | `SystemConfig.port` (system.py:12) | `.settings-port` (line 253) | **match but dead at runtime** (B5) |
| `system_config.live2d_fps` | `live2d_fps: 120` | `SystemConfig.live2d_fps` (system.py:16) | `.settings-fps` (line 254) | match |
| `character_config.character_name` | `character_name: '白'` | `CharacterConfig.character_name` (character.py:19) | `.settings-character-name` (line 251) | match (can't clear, B2) |
| `agent_config.agent_settings.basic_memory_agent.llm_provider` | `llm_provider: 'qwen_llm'` | `BasicMemoryAgentConfig.llm_provider` (agent.py:19) | `.settings-llm` → backend writes here | match |
| `agent_config.llm_configs.*` (provider dicts) | `qwen_llm/deepseek_llm/...` | `StatelessLLMConfigs` has fields for all ids UI+routes use (stateless_llm.py:232-270: `qwen_llm, openrouter, openai, moonshot, together, fireworks, siliconflow, opencode, opencode_go, groq, mistral, anthropic, openai_compatible_llm, openai_compatible`, …) | POST object keyed by pid (matched) | match (GET/POST shape asymmetric — A10) |
| provider model fields | `model/base_url/temperature/max_tokens/models` | `OpenAICompatibleConfig.model/base_url/temperature` … (stateless_llm.py:58-66) | `.llm-model/.llm-base-url/.llm-temperature/.llm-max-tokens` | match |
| `character_config.tts_config.tts_model` | `tts_model: 'qwen_tts'` | `TTSConfig.tts_model` Literal incl. all UI ids (tts.py:780-802) | `.settings-tts` | match |
| TTS per-provider | `qwen_tts/cosyvoice_cloud/edge_tts` with `voice/model/ws_url/voices/models` | `TTSConfig.qwen_tts/cosyvoice_cloud/edge_tts` (tts.py) | POST `tts_configs` keyed by pid | match |
| `asr_config.asr_model` | `asr_model: 'faster_whisper'` | `ASRConfig.asr_model` Literal (asr.py:313-322) | `.settings-asr` | match |
| `tts_config.viseme_mode` | `viseme_mode: rms` | `TTSConfig.viseme_mode` Literal `off/rms/pymouth` (tts.py:792-796) | `.settings-viseme-mode` options `rms/off/pymouth` | match |
| `vision_config.model` (from persona.vision_model) | written by routes `:554-558` | `VisionConfig.model` | `.settings-persona-vision-model` | match |
| `log_level` | lives only in root `.env` (`LIVE2DAI_LOG_LEVEL`) | — (not a Pydantic field) | `.settings-log-level`, but never loaded | **mismatch** (A1/B2) |
| `api_keys` | root `.env` | — | `.settings-key-input` → POST `api_keys` | by design; GET never returns (fine, placeholder "留空不修改") |
| `persona_prompt` | injected at startup from `shared/persona.yaml` (not in conf.yaml) | `CharacterConfig.persona_prompt` required (character.py:22) | persona tab `.settings-persona-prompt` (saved via persona.yaml) | save depends on persona.yaml existing (B9) |

Verbatim schema evidence:
- `config_manager/main.py:12-35` `class Config(I18nMixin, BaseModel): ... character_config: CharacterConfig = Field(..., alias="character_config")` (no `extra=` config → Pydantic-default ignore on validate).
- `config_manager/stateless_llm.py:259-270` the provider aliases.
- `config_manager/tts.py:780-802` `TTSConfig.tts_model` Literal.
- `config_manager/asr.py:313-322` `ASRConfig.asr_model` Literal.
- `config_manager/character.py:16-28` required fields incl. `persona_prompt`, `agent_config`, `asr_config`, `tts_config`, `vad_config`, `tts_preprocessor_config`.

Round-trip note: the settings save path keeps the raw ruamel dict (`_load_yaml`/`_save_yaml`), so **unknown YAML keys are preserved** on this path; `config_manager/utils.py:save_config` (`:116-124`, `model_dump(by_alias=True, exclude_unset=True, exclude_none=True)`) would drop extras, but routes do not use it — flag for any future consumer of `save_config`.

---

## Test coverage (summary of `tests/` + renderer tests)

- Exists & green-leaning: `tests/test_mood_engine.py`, `test_sentence_divider_priority.py`, `test_stream_interceptor.py`, `test_tts_bargein.py`, `test_tts_instruction.py`, `test_tts_parallel.py`, `test_ws_protocol.py`, `test_integration.py`, `test_shared.py` (mood/segmenter/interceptor/TTS-bargein/WS payload + static shared files). Top-level `/tests/` covers persona-zero-config, model registry, validate-registry, consistency-parse. Renderer `renderer/tests/*.test.ts` (Vitest) covers interaction/arbiter/emotion/lipsync/idle/wsbridge/texture — **none import `settings-ui.ts`**.
- **Not covered (major gaps)**:
  - Every HTTP route (`GET/POST /api/config`, `/api/config/tree`, `/api/persona`, `/api/models{/scan,/select}`, `/api/llm/discover-models`, `/api/providers/check`, `/api/test-llm`, `/api/test-tts`).
  - Hot reload / `apply=true` path (`routes.py:651-657` → `service_context.load_from_config`).
  - conf.yaml round-trip (YAML→Pydantic→YAML), unknown-key/null/empty handling, alias mismatches.
  - `run_server` vs `_start_server` divergence; port behavior.
  - `settings-ui.ts` collectPayload/save/provider-dropdown logic (all logic is private inside `initSettingsPanel`, `settings-ui.ts:159` — not unit-testable as written).
  - `websocket_handler`/`proxy_handler`/`message_handler`/`service_context` coupling.
- Harness gap: `tests/conftest.py:13-17` only injects `src` into `sys.path`. No FastAPI `TestClient`, no fixtures, no server construction under test.

---

## C. Root-cause summary (bug → symptom)

1. **Hot reload is not a full runtime apply** (B3/B1, B4): `apply` re-configures only `default_context_cache` (and even then out-of-order); per-session engines, `.env`-derived keys, and the running process are stale → "已保存并热重载" is misleading; most settings actually require reconnect/restart.
2. **No single mutable source of truth** (B3/B4/B8): config is snapshotted into engine constructors; there is no canonical "current settings" store that live components read — so save ≠ applied.
3. **Empty/zero values are indistinguishable from "don't change"** (B2) at the API and char-mapping level → can't clear fields; accidental `port: 0` from a failed UI load (A2/A1) silently keeps the old value.
4. **Non-atomic, ordered persistence** (B1) plus UI's unconditional `log_level` echo (A1) → every save rewrites `.env`'s log level; a mid-save failure leaves conf.yaml written but `.env`/persona untouched.
5. **Port is dead config** (B5) + **divergent launcher** (B6) → settings shows values that don't match how the process actually listens/starts.
6. **Resource leaks** (B7/B8, plus B12 proxy stall) → long-running desktop app degrades after repeated apply/disconnect.
7. **Zero automated coverage of the settings stack** → all of the above shipped untested.

---

## D. Fix plan (ordered; verification for each)

### D1 [P0] Make "保存并应用" actually apply to running sessions
- Files: `src/open_llm_vtuber/routes.py`, `src/open_llm_vtuber/websocket_handler.py`, `src/open_llm_vtuber/service_context.py`.
- What: on `apply=true`, after `load_from_config(new_config)`, iterate `WebSocketHandler.client_contexts` and re-run `load_cache`/`load_from_config` for each session; reorder `load_from_config` to assign `self.system_config`/`self.character_config` **before** `init_agent` so the agent is built from the new snapshot; dispose old engines (D5). If true live-apply for connected clients is out of scope, change the response to `{"applied": false, "requires_restart": true}` and the UI text from "已保存并热重载" to "已保存，重启/重连后生效".
- Why: B3/B4 show the current `applied:true` claim is only partially true.
- Verify: TestClient/WS harness — connect a WS client, `POST /api/config {apply:true}` with a different voice/model, assert the connected session now synthesizes with the new voice (mock engines).

### D2 [P0] Atomic, ordered persistence with rollback
- File: `src/open_llm_vtuber/routes.py` (`_save_yaml` `:207-212`, save block `:590-649`).
- What: write each artifact (conf.yaml, `.env`, persona.yaml) via temp file + `os.replace` (single-writer lock like `threading.Lock`/`filelock`); finish all writers, and if any step fails report it with no partial state (or write conf last so a failed save never mutates conf).
- Why: B1 partial-write; also enables the UI to trust the error message.
- Verify: inject a failing `.env` write and assert conf.yaml is unchanged; run concurrent saves and assert no lost updates.

### D3 [P1] Fix dead `system_config.port` and the divergent launcher
- Files: `run_server.py:145`, `_start_server.py:17-27`, `src/open_llm_vtuber/server.py:121-123`.
- What: bind `uvicorn.Config(..., port=config.system_config.port)` in both launchers (use `run_server.py` pathway in `_start_server.py` too — share a single bootstrap function). Keep proxy URL derived from the same value.
- Why: B5/B6; settings shows a port that the server never uses.
- Verify: set `system_config.port: 12399` in a temp conf, start server, assert it listens on 12399; assert `/proxy-ws` points there.

### D4 [P1] Distinguish "cleared" from "unchanged"; round-trip log_level in `/api/config`
- Files: `src/open_llm_vtuber/routes.py:455-477, 494-496, 529-531, 644-648`; `renderer/src/settings-ui.ts` (`renderSystem` `:779-783`, `collectPayload` `:886-907`, `AppConfig` type `:51-65`, `EMPTY_CONFIG` `:149-157`).
- What: API takes an explicit sentinel (e.g. `null` clears, missing-key keeps); apply the same for persona fields so the UI can clear system prompt/role/name; have `_get_common` return `log_level` (read from `.env`) and have `renderSystem` set the select; only send `log_level` when the user changed it, or treat an unchanged select as "no-op".
- Why: A1/B2 (can't clear; accidental log-level reset; `port:0`).
- Verify: save empty persona fields → assert they're removed; set `.env` `LIVE2DAI_LOG_LEVEL=DEBUG`, open UI → dropdown shows DEBUG; save without touching → `.env` unchanged.

### D5 [P1] Avoid engine leaks on reload/disconnect
- Files: `src/open_llm_vtuber/service_context.py:395-514`, `websocket_handler.py:302-314`.
- What: give each engine a `close()`, call the old engine's close before replacement; fix `handle_disconnect` to look up the context **before** popping and to close it (and make per-session close ref-counted so it doesn't kill shared engines — or stop sharing engines across sessions by constructing per-session engines).
- Why: B7/B8.
- Verify: repeat apply/disconnect under `psutil`/resource counters; assert no growing open handles/subprocesses.

### D6 [P2] Guard the settings UI against load failure and double-submit
- File: `renderer/src/settings-ui.ts` (`open:819`, `save:910`, button listeners `:1083-1102`, `checkAll`, `test*`, `saveTree`).
- What: disable Save/Apply until a successful load; disable the in-flight button per action; add an AbortController/token so the last-clicked request wins the status text; add an explicit "未选择" option and call `checkValidity()` before POST.
- Why: A2/A6/A7; prevents a transient failure from wiping config.
- Verify: Vitest with a mocked `fetch` — assert save disabled before load resolves; assert double-click fires one POST; assert invalid port is blocked.

### D7 [P2] Validate the config tree before overwrite
- File: `src/open_llm_vtuber/routes.py:729-740`.
- What: after `safe_load`, run `AppConfig(**tree)` (after injecting persona) and return 400 on failure instead of writing.
- Why: B10.
- Verify: POST a valid-YAML-but-broken tree → 400, file unchanged.

### D8 [P2] Add a real HTTP/apply test harness
- Files: `tests/conftest.py`, new `tests/test_settings_api.py`, new `renderer/tests/settings-ui.test.ts`.
- What: add a FastAPI `TestClient`/`ASGITransport` fixture with a temp conf.yaml + temp `.env` + temp persona; test GET/POST `/api/config`, `/config/tree`, `/persona`, `/models/*`, `/providers/check` (with respx-mocked outbound), and the apply path (mock engines). Expose `settings-ui.ts` logic for unit testing (or test through `initSettingsPanel` with mocked `fetch`).
- Why: closes the coverage gaps that let B1-B10 ship.
- Verify: new tests red before, green after each fix above.

---

## Explicitly checked / no issue found

- GET/POST `/api/config` payload shapes (arrays on GET, provider-keyed dicts on POST) are internally consistent — asymmetric but functional (A10).
- Provider-id ↔ display labels (`LLM_PROVIDER_LABELS`/`TTS_PROVIDER_LABELS`, `routes.py` `LLM_DEFAULTS.name`) match for all shipped providers.
- `StatelessLLMConfigs`/`TTSConfig`/`ASRConfig` schema field names match the YAML keys and the UI POST keys for every provider currently in conf.yaml (`qwen_llm`, `deepseek_llm`, `zhipu_free_llm`, `cosyvoice_cloud`, `edge_tts`, `faster_whisper`, …) — no field-name typo found in the settings save path.
- `run_server.py` primary startup order (env substitution → persona → model overrides → MCP path resolve → Config → initialize) is correct and consistent.
- `server.py` route/mount ordering (config routes before the static catch-all) is correct.
- `message_handler.wait_for_response`/`handle_message`/`cleanup_client` logic itself is sound (its use by the proxy end-of-turn is the issue, B12).
