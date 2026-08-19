# TTS Config-Schema Audit Report

Scope files read fully (static, no imports):
- `src/open_llm_vtuber/config_manager/tts.py` (945 lines)
- `src/open_llm_vtuber/config_manager/tts_preprocessor.py` (118 lines)
Cross-read: `src/open_llm_vtuber/config_manager/i18n.py`, `conf.yaml`, `conf.yaml.template`, `src/open_llm_vtuber/routes.py`, `renderer/src/settings-ui.ts`, `src/open_llm_vtuber/service_context.py`, `src/open_llm_vtuber/config_manager/utils.py`.
A live Pydantic probe confirmed `ConfigDict(populate_by_name=True)` (with default `extra="ignore"`) silently drops `voices`/`models`.

---

## Section A — Findings

### A1. HIGH — `voices` / `models` sub-keys exist on disk + frontend but are NOT model fields; silently dropped by the Pydantic layer
- `tts.py:150-153` — EdgeTTSConfig has only one field:
  ```
  150: class EdgeTTSConfig(I18nMixin):
  153:     voice: str = Field(..., alias="voice")
  ```
- `tts.py:243-255` — CosyVoiceCloudTtsConfig fields are `api_key, voice, model, format, sample_rate, volume, rate, pitch, ws_url, timeout, max_retries` (no `voices`, no `models`).
- `tts.py:301-304` — QwenTTSConfig fields are `api_key, voice, model, language_type` (no `voices`, no `models`).
- `i18n.py:84` — the shared base config: `model_config = ConfigDict(populate_by_name=True)`. No class in tts.py/tts_preprocessor.py sets `extra=`, so Pydantic v2 default `extra="ignore"` applies.
- On disk `conf.yaml` carries these keys for all three providers:
  ```
  330:     cosyvoice_cloud:
  334:       voice: longxiaochun
  335:       voices:
  336:       - longxiaochun
  ...
  343:       models:
  344:       - cosyvoice-v1
  346:     qwen_tts:
  349:       model: 'qwen3-tts-flash'
  350:       voices:
  ...
  354:       models:
  355:       - qwen3-tts-flash
  357:     edge_tts:
  358:       voice: 'zh-CN-XiaoxiaoNeural'
  360:       voices:
  ```
- The frontend and backend actively use these keys as the per-user voice/model catalog:
  - `settings-ui.ts:29-37` — `TtsProviderCfg` declares `voices?: string[]` / `models?: string[]`.
  - `settings-ui.ts:871-876` — `collectPayload` sends `voice/model/ws_url/voices/models`.
  - `routes.py:529-531` — `update_config` persists fields `("voice", "model", "ws_url", "voices", "models")` into YAML.
  - `routes.py:374-375` — `_get_common` falls back: `cfg.get("voices") or TTS_VOICE_CATALOG.get(pid, [])`.
- Live probe result:
  ```
  dumped: {'voice': 'ok'}
  extra present? False
  ```
- **Why it's a bug / symptom:** the typed config cannot round-trip these keys. `config_manager/utils.py:107-116` (`save_config`) dumps via `model_dump(by_alias=True, exclude_unset=True, exclude_none=True)`, so any save path through the Pydantic object silently strips custom `voices`/`models`, and the settings UI then falls back to the hard-coded `TTS_VOICE_CATALOG`/`TTS_MODEL_CATALOG` (`routes.py:374-375`) — user-added voices/models are silently lost. (Latent today: the active `/api/config` write path `routes.py:445-571` writes raw YAML and only *validates* with `AppConfig(**candidate)`, so the drop currently happens only on any dump→save path such as `save_config` or `service_context.deep_merge`/`validate_config` round-trips.)

### A2. HIGH — `TTSConfig.check_tts_config` never enforces that the *selected* provider is configured; missing block passes validation then crashes at runtime
- `tts.py:897-945` — every validators branch is guarded `... and values.<provider> is not None`; none raises when the selected provider block is absent:
  ```
  897:     @model_validator(mode="after")
  898:     def check_tts_config(cls, values: "TTSConfig", info: ValidationInfo):
  899:         tts_model = values.tts_model
  902:         if tts_model == "azure_tts" and values.azure_tts is not None:
  903:             values.azure_tts.model_validate(values.azure_tts.model_dump())
  ...
  904:         elif tts_model == "edge_tts" and values.edge_tts is not None:
  ...
  912:         elif tts_model == "cosyvoice_cloud" and values.cosyvoice_cloud is not None:
  913:             values.cosyvoice_cloud.model_validate(values.cosyvoice_cloud.model_dump())
  ```
- Runtime assumes presence: `service_context.py:409-410`
  ```
  409:                 tts_config.tts_model,
  410:                 **getattr(tts_config, tts_config.tts_model.lower()).model_dump(),
  ```
- **Why it's a bug / symptom:** `tts_model: edge_tts` with no `edge_tts:` block (e.g. hand-edited conf/character YAML merged via `service_context.handle_config_switch` deep-merge) validates fine (field is `Optional` with `None`), then `init_tts` does `None.model_dump()` → `AttributeError: 'NoneType' object has no attribute 'model_dump'`. The validator was clearly intended to gate the required sub-config but only re-validates already-present (`not None`) blocks — a no-op round trip (`model_dump()` then `model_validate()`) that can never fail on coercion/type grounds.

### A3. MEDIUM — provider ids modeled as `Literal` (*_tts only); legacy ids (`gpt_sovits/piper/sherpa_onnx/pyttsx3`) are remapped only in the routes write path, not in the schema
- `tts.py:780-802` — `tts_model: Literal["azure_tts","bark_tts",...,"pyttsx3_tts"]` (21 ids, all `*_tts` suffix).
- Legacy bare ids are handled only when *saving via the API*: `routes.py:459-464` and `routes.py:535-540` and `routes.py:1053`:
  ```
  459:                 requested_tts = {
  460:                     "gpt_sovits": "gpt_sovits_tts",
  461:                     "piper": "piper_tts",
  462:                     "sherpa_onnx": "sherpa_onnx_tts",
  463:                     "pyttsx3": "pyttsx3_tts",
  464:                 }.get(requested_tts, requested_tts)
  ```
- The frontend still knows the legacy ids: `settings-ui.ts:114-118` (`// 兼容旧配置`).
- **Why it's a bug / symptom:** a YAML that already contains a legacy id (or a character YAML with one) fails `Config(**dict)` / `AppConfig(**candidate)` with `Input should be 'gpt_sovits_tts', 'piper_tts', ...` — app fails startup (`run_server.py:138 Config(**config_dict)`) and config saving is rejected. The remap lives in the UI write path only; the schema itself does not accept or normalize the legacy ids. Note: on-disk `conf.yaml:328` currently uses `qwen_tts`, so this is latent but reachable via character/alt configs.

### A4. MEDIUM — non-selected provider blocks are still fully validated; one bad/unused block bricks config load (all-or-nothing)
- Each provider is a normal `Optional` field of `TTSConfig` (`tts.py:812-838`), so any provider block *present* in YAML is validated even when not selected. Required-field providers (e.g. `GPTSoVITSConfig` requires `api_url`, `text_lang`, … — `tts.py:370-378`) can therefore fail validation from an unused block.
- `run_server.py:138` `Config(**config_dict)`; `routes.py:557-569` validates the whole candidate on every save.
- **Why it's a bug / symptom:** `gpt_sovits_tts: {}` (or a typo'd unused provider) prevents the entire app from starting / every settings save, even when `tts_model` is `edge_tts`. The "only validate the selected model" comment at `tts.py:901` is not what the code does — it only re-validates the selected block when non-None; the per-field validation still happens for all present blocks.

### A5. LOW — SiliconFlowTTSConfig `DESCRIPTIONS` keys don't match field names (i18n help silently missing)
- `tts.py:502-512` fields: `api_url`, `default_model`, `default_voice`, `sample_rate`, `response_format`, `stream`, `speed`, `gain`.
- `tts.py:514-536` descriptions are keyed by `"url"`, `"model"`, `"voice"`, `"sample_rate"`, `"stream"`, `"speed"`, `"gain"` (no `response_format`), which do not equal the field names used by `I18nMixin.get_field_description` (`i18n.py:98-107`).
- **Why it's a bug / symptom:** `get_field_description("api_url"/"default_model"/"default_voice"/"response_format")` returns `None`; the settings/UI has no translated help for these fields.

### A6. LOW — `conf.yaml.template` drifted from schema/`conf.yaml` (missing `qwen_tts` block and `viseme_mode`)
- `conf.yaml.template:76-99` defines `tts_config` with only `tts_model: 'edge_tts'`, `concurrency_limit`, `edge_tts.voice`, and `cosyvoice_cloud.{api_key,voice,model,format,sample_rate,volume,rate,pitch,ws_url,timeout,max_retries}` — **no `qwen_tts` block and no `viseme_mode`**.
- On-disk `conf.yaml:328-372` has `qwen_tts`, `cosyvoice_cloud`, `edge_tts` AND `viseme_mode: rms` (line 372).
- **Why it's a bug / symptom:** cosmetic/self-documentation drift. Schema defaults (`viseme_mode: "rms"` at `tts.py:808-810`, Qwen defaults at `tts.py:301-304`) mask it, so it's not fatal, but a fresh install won't discover the `qwen_tts` mode or the `viseme_mode` option from the template.

### A7. LOW — empty/"auto" model semantics live in routes, not schema; empty strings are accepted by the schema
- Schema: most `model`/`voice` fields are plain `str` with no `min_length`; e.g. `QwenTTSConfig.model: str = Field("qwen3-tts-flash", ...)` (`tts.py:303`) accepts `""`; `OpenAITTSConfig.model/voice/api_key/base_url` are `Optional[str] = None` (`tts.py:542-545`).
- The "if empty, fill first known" fallback exists only in `routes.py:376-381` and `routes.py:546-551`:
  ```
  376:             current_voice = cfg.get("voice", "")
  377:             if not current_voice and known_voices:
  378:                 current_voice = known_voices[0]
  ```
- **Why it's a bug / symptom:** the data model gives no protection against an empty/`None` (`""`) voice/model written directly to YAML; only the routes layer heals it for the settings UI. Not a runtime crash on the happy path, but a schema-level gap.

### A8. No issue found — preprocessor mapping is clean
`conf.yaml:384-392` and `conf.yaml.template:111-119` both define:
```
tts_preprocessor_config:
  remove_special_char: true
  ignore_brackets: true
  ignore_parentheses: true
  ignore_asterisks: true
  ignore_angle_brackets: true
  translator_config:
    translate_audio: false
    translate_provider: 'deeplx'
```
This exactly matches `TTSPreprocessorConfig` (`tts_preprocessor.py:103-108`) — all five `ignore_*` fields have matching aliases/defaults, `remove_special_char` and `translator_config` are present, and `translate_audio`/`translate_provider` match `TranslatorConfig` (`tts_preprocessor.py:59-64`). `deeplx`/`tencent` sub-blocks are `Optional` (`tts_preprocessor.py:63-64`) and correctly omitted from the on-disk YAML while `translate_audio: false`.

Minor (not a mapping bug): `TTSPreprocessorConfig.DESCRIPTIONS` (`tts_preprocessor.py:110-117`) documents only `remove_special_char` and `translator_config`, omitting the four `ignore_*` fields — cosmetic i18n gap only.

### A9. No issue found — other areas
- **No mutable defaults**: every field default in `tts.py`/`tts_preprocessor.py` is a primitive (`str/int/float/bool` or `None`); grep for `Field([])`/`Field({})` returned nothing.
- **Literal validation is strict, not silently forgiving**: invalid `tts_model` / `viseme_mode` / `latency` values raise `ValidationError` (no silent coercion).
- `concurrency_limit: int Field(2, alias="concurrency_limit", ge=1)` (`tts.py:805`) matches on-disk `conf.yaml:329 concurrency_limit: 2`.
- Aliases equal python field names everywhere in `tts.py` (e.g. `alias="voice"`, `alias="ws_url"`), and `I18nMixin` sets `populate_by_name=True` (`i18n.py:84`), so there is **no snake_case/camelCase alias mismatch** between schema, YAML, and the frontend JSON keys (`tts_model`, `viseme_mode`, `tts_configs[].{id,voice,model,ws_url,voices,models}`).

---

## Section B — Schema keys discovered

Base config for all rows: inherited from `I18nMixin` → `i18n.py:84` `model_config = ConfigDict(populate_by_name=True)`; **no `extra=` set anywhere** (default `extra="ignore"`). All aliases equal the python field name unless noted. Type shown as written in the source.

### tts.py

| class (line) | python field | alias | type | default | validators | line |
|---|---|---|---|---|---|---|
| AzureTTSConfig (114) | api_key | api_key | str | (required) | — | 117 |
| | region | region | str | (required) | — | 118 |
| | voice | voice | str | (required) | — | 119 |
| | pitch | pitch | str | (required) | — | 120 |
| | rate | rate | str | (required) | — | 121 |
| BarkTTSConfig (138) | voice | voice | str | (required) | — | 141 |
| EdgeTTSConfig (150) | voice | voice | str | (required) | — | 153 |
| CosyvoiceTTSConfig (163) | client_url | client_url | str | (required) | — | 166 |
| | mode_checkbox_group | mode_checkbox_group | str | (required) | — | 167 |
| | sft_dropdown | sft_dropdown | str | (required) | — | 168 |
| | prompt_text | prompt_text | str | (required) | — | 169 |
| | prompt_wav_upload_url | prompt_wav_upload_url | str | (required) | — | 170 |
| | prompt_wav_record_url | prompt_wav_record_url | str | (required) | — | 171 |
| | instruct_text | instruct_text | str | (required) | — | 172 |
| | seed | seed | int | (required) | — | 173 |
| | api_name | api_name | str | (required) | — | 174 |
| Cosyvoice2TTSConfig (197) | client_url | client_url | str | (required) | — | 200 |
| | mode_checkbox_group | mode_checkbox_group | str | (required) | — | 201 |
| | sft_dropdown | sft_dropdown | str | (required) | — | 202 |
| | prompt_text | prompt_text | str | (required) | — | 203 |
| | prompt_wav_upload_url | prompt_wav_upload_url | str | (required) | — | 204 |
| | prompt_wav_record_url | prompt_wav_record_url | str | (required) | — | 205 |
| | instruct_text | instruct_text | str | (required) | — | 206 |
| | stream | stream | bool | (required) | — | 207 |
| | seed | seed | int | (required) | — | 208 |
| | speed | speed | float | (required) | — | 209 |
| | api_name | api_name | str | (required) | — | 210 |
| CosyVoiceCloudTtsConfig (235) | api_key | api_key | str | "" | — | 243 |
| | voice | voice | str | "longxiaochun" | — | 244 |
| | model | model | str | "cosyvoice-v1" | — | 245 |
| | format | format | str | "mp3" | — | 246 |
| | sample_rate | sample_rate | int | 22050 | — | 247 |
| | volume | volume | int | 50 | — | 248 |
| | rate | rate | float | 1.0 | — | 249 |
| | pitch | pitch | float | 1.0 | — | 250 |
| | ws_url | ws_url | str | "wss://dashscope.aliyuncs.com/api-ws/v1/inference" | — | 251-253 |
| | timeout | timeout | float | 60.0 | — | 254 |
| | max_retries | max_retries | int | 2 | — | 255 |
| QwenTTSConfig (298) | api_key | api_key | str | "" | — | 301 |
| | voice | voice | str | "Cherry" | — | 302 |
| | model | model | str | "qwen3-tts-flash" | — | 303 |
| | language_type | language_type | str | "Auto" | — | 304 |
| MeloTTSConfig (323) | speaker | speaker | str | (required) | — | 326 |
| | language | language | str | (required) | — | 327 |
| | device | device | str | "auto" | — | 328 |
| | speed | speed | float | 1.0 | — | 329 |
| XTTSConfig (347) | api_url | api_url | str | (required) | — | 350 |
| | speaker_wav | speaker_wav | str | (required) | — | 351 |
| | language | language | str | (required) | — | 352 |
| GPTSoVITSConfig (367) | api_url | api_url | str | (required) | — | 370 |
| | text_lang | text_lang | str | (required) | — | 371 |
| | ref_audio_path | ref_audio_path | str | (required) | — | 372 |
| | prompt_lang | prompt_lang | str | (required) | — | 373 |
| | prompt_text | prompt_text | str | (required) | — | 374 |
| | text_split_method | text_split_method | str | (required) | — | 375 |
| | batch_size | batch_size | str | (required) | — | 376 |
| | media_type | media_type | str | (required) | — | 377 |
| | streaming_mode | streaming_mode | str | (required) | — | 378 |
| FishAPITTSConfig (399) | api_key | api_key | str | (required) | — | 402 |
| | reference_id | reference_id | str | (required) | — | 403 |
| | latency | latency | Literal["normal","balanced"] | (required) | — | 404 |
| | base_url | base_url | str | (required) | — | 405 |
| CoquiTTSConfig (424) | model_name | model_name | str | (required) | — | 427 |
| | speaker_wav | speaker_wav | str | "" | — | 428 |
| | language | language | str | (required) | — | 429 |
| | device | device | str | "" | — | 430 |
| SherpaOnnxTTSConfig (450) | vits_model | vits_model | str | (required) | — | 453 |
| | vits_lexicon | vits_lexicon | Optional[str] | None | — | 454 |
| | vits_tokens | vits_tokens | str | (required) | — | 455 |
| | vits_data_dir | vits_data_dir | Optional[str] | None | — | 456 |
| | vits_dict_dir | vits_dict_dir | Optional[str] | None | — | 457 |
| | tts_rule_fsts | tts_rule_fsts | Optional[str] | None | — | 458 |
| | max_num_sentences | max_num_sentences | int | 2 | — | 459 |
| | sid | sid | int | 1 | — | 460 |
| | provider | provider | Literal["cpu","cuda","coreml"] | "cpu" | — | 461 |
| | num_threads | num_threads | int | 1 | — | 462 |
| | speed | speed | float | 1.0 | — | 463 |
| | debug | debug | bool | False | — | 464 |
| SiliconFlowTTSConfig (499) | api_url | api_url | str | "https://api.siliconflow.cn/v1/audio/speech" | — | 502 |
| | api_key | api_key | str | (required) | — | 503 |
| | default_model | default_model | str | "FunAudioLLM/CosyVoice2-0.5B" | — | 504 |
| | default_voice | default_voice | str | "speech:Dreamflowers:5bdstvc39i:xkqldnpasqmoqbakubom" | — | 505-507 |
| | sample_rate | sample_rate | int | 32000 | — | 508 |
| | response_format | response_format | str | "mp3" | — | 509 |
| | stream | stream | bool | True | — | 510 |
| | speed | speed | float | 1 | — | 511 |
| | gain | gain | int | 0 | — | 512 |
| OpenAITTSConfig (539) | model | model | Optional[str] | None | — | 542 |
| | voice | voice | Optional[str] | None | — | 543 |
| | api_key | api_key | Optional[str] | None | — | 544 |
| | base_url | base_url | Optional[str] | None | — | 545 |
| | file_extension | file_extension | Literal["mp3","wav"] | "mp3" | — | 546 |
| SparkTTSConfig (572) | api_url | api_url | str | (required) | — | 575 |
| | prompt_wav_upload | prompt_wav_upload | str | (required) | — | 576 |
| | api_name | api_name | str | (required) | — | 577 |
| | gender | gender | str | (required) | — | 578 |
| | pitch | pitch | int | (required) | — | 579 |
| | speed | speed | int | (required) | — | 580 |
| MinimaxTTSConfig (609) | group_id | group_id | str | (required) | — | 612 |
| | api_key | api_key | str | (required) | — | 613 |
| | model | model | str | "speech-02-turbo" | — | 614 |
| | voice_id | voice_id | str | "male-qn-qingse" | — | 615 |
| | pronunciation_dict | pronunciation_dict | str | "" | — | 616 |
| PiperTTSConfig (629) | model_path | model_path | str | "models/piper/zh_CN-huayan-medium.onnx" | — | 632 |
| | speaker_id | speaker_id | int | 0 | — | 633 |
| | length_scale | length_scale | float | 1.0 | — | 634 |
| | noise_scale | noise_scale | float | 0.667 | — | 635 |
| | noise_w | noise_w | float | 0.8 | — | 636 |
| | volume | volume | float | 1.0 | — | 637 |
| | normalize_audio | normalize_audio | bool | True | — | 638 |
| | use_cuda | use_cuda | bool | False | — | 639 |
| Pyttsx3TTSConfig (676) | (no fields) | | | | | |
| ElevenLabsTTSConfig (682) | api_key | api_key | str | (required) | — | 685 |
| | voice_id | voice_id | str | (required) | — | 686 |
| | model_id | model_id | str | "eleven_multilingual_v2" | — | 687 |
| | output_format | output_format | str | "mp3_44100_128" | — | 688 |
| | stability | stability | float | 0.5 | — | 689 |
| | similarity_boost | similarity_boost | float | 0.5 | — | 690 |
| | style | style | float | 0.0 | — | 691 |
| | use_speaker_boost | use_speaker_boost | bool | True | — | 692 |
| CartesiaTTSConfig (727) | model_id | model_id | Literal["sonic-3","sonic-2","sonic-turbo","sonic-multilingual","sonic"] | "sonic-3" | — | 730-732 |
| | api_key | api_key | str | (required) | — | 734 |
| | voice_id | voice_id | str | (required) | — | 735 |
| | output_format | output_format | Literal["wav","mp3"] | "wav" | — | 736 |
| | language | language | CartesiaLanguages (Literal[49]) | "en" | — | 737 |
| | emotion | emotion | CartesiaEmotions (Literal~60) | "neutral" | — | 738 |
| | volume | volume | float | 1.0 | — | 739 |
| | speed | speed | float | 1.0 | — | 740 |
| TTSConfig (777) root | tts_model | tts_model | Literal[21 ids: azure_tts … pyttsx3_tts] | (required) | — | 780-802 |
| | concurrency_limit | concurrency_limit | int (ge=1) | 2 | — | 805 |
| | viseme_mode | viseme_mode | Literal["off","rms","pymouth"] | "rms" | — | 808-810 |
| | azure_tts | azure_tts | Optional[AzureTTSConfig] | None | — | 812 |
| | bark_tts | bark_tts | Optional[BarkTTSConfig] | None | — | 813 |
| | edge_tts | edge_tts | Optional[EdgeTTSConfig] | None | — | 814 |
| | cosyvoice_tts | cosyvoice_tts | Optional[CosyvoiceTTSConfig] | None | — | 815 |
| | cosyvoice2_tts | cosyvoice2_tts | Optional[Cosyvoice2TTSConfig] | None | — | 816 |
| | cosyvoice_cloud | cosyvoice_cloud | Optional[CosyVoiceCloudTtsConfig] | None | — | 817-819 |
| | qwen_tts | qwen_tts | Optional[QwenTTSConfig] | None | — | 820 |
| | melo_tts | melo_tts | Optional[MeloTTSConfig] | None | — | 821 |
| | coqui_tts | coqui_tts | Optional[CoquiTTSConfig] | None | — | 822 |
| | x_tts | x_tts | Optional[XTTSConfig] | None | — | 823 |
| | gpt_sovits_tts | gpt_sovits_tts | Optional[GPTSoVITSConfig] | None | — | 824 |
| | fish_api_tts | fish_api_tts | Optional[FishAPITTSConfig] | None | — | 825 |
| | sherpa_onnx_tts | sherpa_onnx_tts | Optional[SherpaOnnxTTSConfig] | None | — | 826-828 |
| | siliconflow_tts | siliconflow_tts | Optional[SiliconFlowTTSConfig] | None | — | 829-831 |
| | openai_tts | openai_tts | Optional[OpenAITTSConfig] | None | — | 832 |
| | spark_tts | spark_tts | Optional[SparkTTSConfig] | None | — | 833 |
| | minimax_tts | minimax_tts | Optional[MinimaxTTSConfig] | None | — | 834 |
| | elevenlabs_tts | elevenlabs_tts | ElevenLabsTTSConfig \| None | None | — | 835 |
| | cartesia_tts | cartesia_tts | CartesiaTTSConfig \| None | None | — | 836 |
| | piper_tts | piper_tts | Optional[PiperTTSConfig] | None | — | 837 |
| | pyttsx3_tts | pyttsx3_tts | Optional[Pyttsx3TTSConfig] | None | — | 838 |
| | (model_validator) | | | | TTSConfig.check_tts_config (after) | 897-945 |

Mapping to the mission's named classes: TTSConfig root = `TTSConfig` (777); Edge = `EdgeTTSConfig` (150); Cosyvoice cloud = `CosyVoiceCloudTtsConfig` (235, note the exact class name); Cosyvoice local v1/v2 = `CosyvoiceTTSConfig` (163) / `Cosyvoice2TTSConfig` (197); Qwen = `QwenTTSConfig` (298). **No separate v3/v4 config classes exist** in this file (CosyVoice2 + SiliconFlow's `CosyVoice2-0.5B` default model are the only "2" references; no v3/v4).

### tts_preprocessor.py

| class (line) | python field | alias | type | default | validators | line |
|---|---|---|---|---|---|---|
| DeepLXConfig (9) | deeplx_target_lang | deeplx_target_lang | str | (required) | — | 12 |
| | deeplx_api_endpoint | deeplx_api_endpoint | str | (required) | — | 13 |
| TencentConfig (26) | secret_id | (none; name used) | str | (required) | — | 29 |
| | secret_key | (none; name used) | str | (required) | — | 30 |
| | region | (none; name used) | str | (required) | — | 31 |
| | source_lang | (none; name used) | str | (required) | — | 32-34 |
| | target_lang | (none; name used) | str | (required) | — | 35-37 |
| TranslatorConfig (56) | translate_audio | translate_audio | bool | (required) | — | 59 |
| | translate_provider | translate_provider | Literal["deeplx","tencent"] | (required) | — | 60-62 |
| | deeplx | deeplx | Optional[DeepLXConfig] | None | — | 63 |
| | tencent | tencent | Optional[TencentConfig] | None | — | 64 |
| | (model_validator) | | | | TranslatorConfig.check_translator_config (after) | 82-97 |
| TTSPreprocessorConfig (100) | remove_special_char | remove_special_char | bool | (required) | — | 103 |
| | ignore_brackets | ignore_brackets | bool | True | — | 104 |
| | ignore_parentheses | ignore_parentheses | bool | True | — | 105 |
| | ignore_asterisks | ignore_asterisks | bool | True | — | 106 |
| | ignore_angle_brackets | ignore_angle_brackets | bool | True | — | 107 |
| | translator_config | translator_config | TranslatorConfig | (required) | — | 108 |

Notes:
- `TencentConfig` fields declare `description=` but no explicit `alias`; with the field name equal to the YAML key this resolves fine.
- The `give_brackets`/parentheses/etc. `ignore_*` defaults (`True`) match on-disk `conf.yaml` values (`true`).

---

## Question-by-question summary

1. All model classes/fields: Section B table above. `TTSConfig` root includes `tts_model`, `concurrency_limit`, `viseme_mode` and 21 provider slots — so `conf.yaml`'s `tts_model`/`viseme_mode`/`concurrency_limit` and provider sub-dict keys (`cosyvoice_cloud`, `qwen_tts`, `edge_tts`) all appear as schema fields.
2. Field-name mismatches:
   - Schema vs on-disk `conf.yaml`: no alias mismatch; **gap = `voices`/`models` under all three providers are on-disk/UI keys but not schema fields** (A1). Everything else (including `viseme_mode`) matches.
   - Schema vs `conf.yaml.template`: template has no `qwen_tts` and no `viseme_mode` (A6); schema defaults cover it.
   - Frontend/JSON vs routes: identical key names (`tts_model`, `viseme_mode`, `tts_configs[].{id,voice,model,ws_url,voices,models}`); routes stores exactly `("voice","model","ws_url","voices","models")` (`routes.py:529-531`). The frontend catalog entries flow through raw YAML, not Pydantic.
3. Provider ids: `tts_model` is a strict `Literal` of 21 `*_tts` ids (`tts.py:780-802`); provider sub-dict keys are fixed `Optional` fields (unknown extra sub-dict keys are silently dropped by `extra="ignore"`). A non-Literal `tts_model` (including legacy `gpt_sovits`/`piper`/`sherpa_onnx`/`pyttsx3`) fails validation; the legacy→`*_tts` remap exists only in the routes write paths (`routes.py:459-464`, `535-540`, `1053`), not in the schema (A3). "auto"/empty model handling is implemented in routes (`routes.py:376-381`, `546-551`), not in the schema, which accepts empty strings (A7). Display-name vs id: the 13 active catalogs in `TTS_DEFAULTS`/`TTS_PROVIDER_LABELS` are consistent; only the legacy bare ids exist in the labels + routes remap, not in the schema.
4. Round-trip issues: unknown keys dropped — yes, `voices`/`models` (A1); fields with defaults re-serialized on dump — yes via `save_config` (`utils.py:107-116`), but that helper has no in-repo caller today; None vs default — `Optional[...]=None` provider slots work for non-selected providers but permit the selected-but-missing crash (A2); mutable defaults — none found (A9); enums/Literals — strict, no silent coercion (A9); `voices`/`models` as lists vs strings — they exist only as YAML/JS lists, never as schema fields (A1).
5. `tts_preprocessor.py` vs `conf.yaml` `character_config.tts_preprocessor_config.*`: **clean** (A8 — all keys map 1:1; only cosmetic i18n `DESCRIPTIONS` omissions).

---

Generated statically; no files were modified. Evidence files: conf.yaml (tts_config 327-372, tts_preprocessor_config 384-392), conf.yaml.template (tts_config 76-99, tts_preprocessor_config 111-119), routes.py (152-199 catalogs, 364-414 _get_common, 457-551 update_config, 930-975 check, 1044-1135 test_tts), settings-ui.ts (28-37 interface, 100-118 labels, 838-895 collectPayload).
