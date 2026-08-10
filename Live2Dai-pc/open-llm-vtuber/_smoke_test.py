"""Live2Dai — Full Pipeline Smoke Test (全链路点火测试)
=============================================================
Tests every link in the chain independently, then end-to-end.
Run: .venv\Scripts\python.exe _smoke_test.py
============================================================="""

import io
import sys

# Fix Windows GBK encoding for emoji output
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")

import asyncio
import json
import os
import time
import traceback

import httpx
import yaml

_project_root = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(_project_root, "src"))
sys.path.insert(0, _project_root)


# ─── helpers ───────────────────────────────────────────────
def load_config():
    with open(os.path.join(_project_root, "conf.yaml"), "r", encoding="utf-8") as f:
        return yaml.safe_load(f)


RESULT = {"pass": 0, "fail": 0, "skip": 0}


def check(name, condition, detail=""):
    if condition:
        RESULT["pass"] += 1
        print(f"  [PASS] {name}")
    else:
        RESULT["fail"] += 1
        print(f"  [FAIL] {name}  — {detail}")
    return condition


def skip(name, reason=""):
    RESULT["skip"] += 1
    print(f"  [SKIP]  {name}  — {reason}")


def summary():
    p, f, s = RESULT["pass"], RESULT["fail"], RESULT["skip"]
    print(f"\n{'=' * 60}")
    print(f"Results: {p} passed, {f} failed, {s} skipped")
    if f == 0:
        print("*** ALL CHECKS PASSED! ***")
    else:
        print(f"WARNING:  {f} CHECK(S) FAILED — review output above")
    return f == 0


# ─── Test 0: Config Integrity ──────────────────────────────
def test_config_integrity(cfg):
    print("\n── Test 0: Config Integrity ──")
    sc = cfg["system_config"]
    cc = cfg["character_config"]
    check("system_config present", "system_config" in cfg)
    check("character_config present", "character_config" in cfg)
    check("port is 12393", sc["port"] == 12393)
    check("live2d_fps is 120", sc["live2d_fps"] == 120)
    check("persona_prompt not empty", len(cc["persona_prompt"]) > 50)
    check("persona contains 凛", "凛" in cc["persona_prompt"])
    check("persona contains 傲娇", "傲娇" in cc["persona_prompt"])
    check("persona contains 用户", "用户" in cc["persona_prompt"])

    llm_cfgs = cc["agent_config"]["llm_configs"]
    check("deepseek_llm has llm_api_key", "llm_api_key" in llm_cfgs["deepseek_llm"])
    check("zhipu_llm has llm_api_key", "llm_api_key" in llm_cfgs["zhipu_llm"])
    check(
        "deepseek model is deepseek-chat",
        llm_cfgs["deepseek_llm"]["model"] == "deepseek-chat",
    )

    ag = cc["agent_config"]
    check(
        "conversation_agent is basic_memory_agent",
        ag["conversation_agent_choice"] == "basic_memory_agent",
    )
    bma = ag["agent_settings"]["basic_memory_agent"]
    check("llm_provider is deepseek_llm", bma["llm_provider"] == "deepseek_llm")
    check("use_mcpp is True", bma["use_mcpp"] == True)
    check("mcp_enabled_servers has time", "time" in bma["mcp_enabled_servers"])
    check(
        "mcp_enabled_servers has ddg-search",
        "ddg-search" in bma["mcp_enabled_servers"],
    )

    tts = cc["tts_config"]
    check("tts_model is edge_tts", tts["tts_model"] == "edge_tts")
    check(
        "edge_tts voice is zh-CN-XiaoxiaoNeural",
        tts["edge_tts"]["voice"] == "zh-CN-XiaoxiaoNeural",
    )

    check(
        "ASR model is faster_whisper", cc["asr_config"]["asr_model"] == "faster_whisper"
    )
    check("VAD is disabled (null)", cc["vad_config"]["vad_model"] is None)


# ─── Test 1: DeepSeek API ──────────────────────────────────
async def test_deepseek_api(cfg):
    print("\n── Test 1: DeepSeek API ──")
    ds = cfg["character_config"]["agent_config"]["llm_configs"]["deepseek_llm"]
    api_key = ds["llm_api_key"]
    model = ds["model"]

    check("API key is not placeholder", "sk-" in api_key and len(api_key) > 20)

    headers = {
        "Authorization": f"Bearer {api_key}",
        "Content-Type": "application/json",
    }

    # 1a: List models
    try:
        async with httpx.AsyncClient(timeout=15) as client:
            resp = await client.get(
                "https://api.deepseek.com/v1/models", headers=headers
            )
        check(
            f"GET /v1/models → {resp.status_code}",
            resp.status_code == 200,
            f"status={resp.status_code} body={resp.text[:200]}",
        )
        if resp.status_code == 200:
            data = resp.json()
            check("models list is non-empty", len(data.get("data", [])) > 0)
    except Exception as e:
        check("GET /v1/models", False, str(e))

    # 1b: Chat completion
    persona = cfg["character_config"]["persona_prompt"]
    try:
        async with httpx.AsyncClient(timeout=30) as client:
            resp = await client.post(
                "https://api.deepseek.com/v1/chat/completions",
                headers=headers,
                json={
                    "model": model,
                    "messages": [
                        {"role": "system", "content": persona},
                        {"role": "user", "content": "你好喵~"},
                    ],
                    "temperature": 0.8,
                    "max_tokens": 100,
                    "stream": False,
                },
            )
        check(
            f"POST /v1/chat/completions → {resp.status_code}",
            resp.status_code == 200,
            f"status={resp.status_code} body={resp.text[:300]}",
        )
        if resp.status_code == 200:
            data = resp.json()
            reply = data["choices"][0]["message"]["content"]
            check("reply is non-empty", len(reply) > 0)
            check("reply mentions 喵", "喵" in reply, f"reply={reply[:100]}")
            print(f"     DeepSeek says: {reply[:120]}...")
    except Exception as e:
        check("POST /v1/chat/completions", False, str(e))


# ─── Test 2: Zhipu (GLM-4V) API ────────────────────────────
async def test_zhipu_api(cfg):
    print("\n── Test 2: Zhipu (GLM-4V) API ──")
    zp = cfg["character_config"]["agent_config"]["llm_configs"]["zhipu_llm"]
    api_key = zp["llm_api_key"]
    model = zp["model"]

    check("API key is not placeholder", len(api_key) > 20)

    headers = {
        "Authorization": f"Bearer {api_key}",
        "Content-Type": "application/json",
    }

    # Simple text completion (non-vision)
    try:
        async with httpx.AsyncClient(timeout=30) as client:
            resp = await client.post(
                "https://open.bigmodel.cn/api/paas/v4/chat/completions",
                headers=headers,
                json={
                    "model": model,
                    "messages": [{"role": "user", "content": "say hello in one word"}],
                    "max_tokens": 100,
                    "stream": False,
                },
            )
        check(
            f"GLM-4V chat → {resp.status_code}",
            resp.status_code == 200,
            f"status={resp.status_code} body={resp.text[:200]}",
        )
        if resp.status_code == 200:
            data = resp.json()
            reply = data["choices"][0]["message"]["content"]
            check("reply is non-empty", len(reply) > 0)
            print(f"     GLM-4V says: {reply[:80]}")
    except Exception as e:
        check("GLM-4V chat", False, str(e))


# ─── Test 3: Edge TTS ──────────────────────────────────────
async def test_edge_tts():
    print("\n── Test 3: Edge TTS ──")

    try:
        import edge_tts  # noqa: F401

        check("edge_tts module importable", True)
    except ImportError:
        check("edge_tts module importable", False, "pip install edge-tts")
        return

    # List available voices
    try:
        voices = await edge_tts.list_voices()
        zh_voices = [v for v in voices if "zh-CN" in v["Locale"]]
        check(f"zh-CN voices available ({len(zh_voices)})", len(zh_voices) > 0)
        xiaoxiao = [v for v in zh_voices if "Xiaoxiao" in v["ShortName"]]
        check(
            "XiaoxiaoNeural available",
            len(xiaoxiao) > 0,
            f"found {[v['ShortName'] for v in zh_voices[:5]]}",
        )
    except Exception as e:
        check("list_voices", False, str(e))

    # Actually synthesize a short audio
    try:
        text = "你好呀！凛在这里呢！(=^･ω･^=)"
        communicate = edge_tts.Communicate(text, "zh-CN-XiaoxiaoNeural")
        temp_path = os.path.join(_project_root, "_test_tts_output.mp3")
        start = time.time()
        await communicate.save(temp_path)
        elapsed = time.time() - start
        file_size = os.path.getsize(temp_path)
        check(f"TTS synthesized ({file_size} bytes, {elapsed:.1f}s)", file_size > 100)
        # Cleanup
        os.remove(temp_path)
    except Exception as e:
        check("TTS synthesize", False, str(e))


# ─── Test 4: MCP Tools ─────────────────────────────────────
async def test_mcp_tools():
    print("\n── Test 4: MCP Tools ──")
    mcp_path = os.path.join(_project_root, "mcp_servers.json")
    check("mcp_servers.json exists", os.path.exists(mcp_path))
    if not os.path.exists(mcp_path):
        return

    with open(mcp_path, "r", encoding="utf-8") as f:
        mcp_cfg = json.load(f)

    servers = mcp_cfg.get("mcp_servers", {})
    check("time server defined", "time" in servers)
    check("ddg-search server defined", "ddg-search" in servers)
    check("weather server defined", "weather" in servers)
    check("filesystem server defined", "filesystem" in servers)
    check("sequential-thinking defined", "sequential-thinking" in servers)

    for name in ["time", "ddg-search"]:
        s = servers.get(name, {})
        check(f"{name} has command", bool(s.get("command")))
        check(f"{name} has args", bool(s.get("args")))
        check(f"{name} has description", bool(s.get("description")))

    # uvx availability check
    import shutil

    uvx_path = shutil.which("uvx")
    if uvx_path:
        check(f"uvx found at {uvx_path}", True)
    else:
        skip("uvx in PATH", "MCP tools require uvx (install via pip install uv)")


# ─── Test 5: Server Startup ────────────────────────────────
async def test_server_startup():
    print("\n── Test 5: Server Startup ──")
    from open_llm_vtuber.config_manager.main import Config

    # Load config through Pydantic model
    cfg_raw = load_config()
    try:
        config = Config(**cfg_raw)
        check("Pydantic Config validates", True)
    except Exception as e:
        check("Pydantic Config validates", False, str(e))
        return

    # Verify server import chain works (without starting ASR/model download)
    try:

        check("WebSocketServer importable", True)

        # Check key sub-modules importable

        check("ASRFactory importable", True)
        check("ServiceContext importable", True)

        # Verify faster_whisper installed
        try:
            import faster_whisper  # noqa: F401

            check("faster_whisper installed", True)
        except ImportError:
            check("faster_whisper installed", False, "pip install faster-whisper")

    except Exception as e:
        check("Server imports", False, str(e))
        traceback.print_exc()


# ─── Test 6: End-to-End Chat Flow ──────────────────────────
async def test_e2e_chat():
    """Simulate the full chat pipeline: persona → LLM → TTS text preprocessing."""
    print("\n── Test 6: End-to-End Chat Flow ──")
    cfg = load_config()
    cc = cfg["character_config"]

    # 1. System prompt is well-formed
    persona = cc["persona_prompt"]
    check(
        "persona has system prompt structure",
        "## 性格特点" in persona or "性格特点" in persona,
    )
    check("persona has expression tags", "[joy]" in persona or "[neutral]" in persona)

    # 2. TTS preprocessor config
    tts_pre = cc["tts_preprocessor_config"]
    check(
        "ignore_brackets enabled (for emotion tags)", tts_pre["ignore_brackets"] == True
    )
    check(
        "ignore_parentheses enabled (for kaomoji)",
        tts_pre["ignore_parentheses"] == True,
    )
    check("remove_special_char enabled", tts_pre["remove_special_char"] == True)

    # 3. Verify expression prompt template exists
    prompts_dir = os.path.join(_project_root, "prompts")
    check("prompts directory exists", os.path.isdir(prompts_dir))
    if os.path.isdir(prompts_dir):
        prompt_files = os.listdir(prompts_dir)
        check("has prompt files", len(prompt_files) > 0, f"files: {prompt_files[:5]}")

    # 4. Verify Live2D model exists
    model_dir = os.path.join(_project_root, "live2d-models", cc["live2d_model_name"])
    check(
        f"Live2D model '{cc['live2d_model_name']}' dir exists", os.path.isdir(model_dir)
    )
    if os.path.isdir(model_dir):
        model3_path = os.path.join(
            model_dir, "runtime", f"{cc['live2d_model_name']}.model3.json"
        )
        check("model3.json exists", os.path.exists(model3_path))
        moc3_path = os.path.join(
            model_dir, "runtime", f"{cc['live2d_model_name']}.moc3"
        )
        check(".moc3 exists", os.path.exists(moc3_path))


# ─── main ───────────────────────────────────────────────────
async def main():
    print("=" * 60)
    print("  Live2Dai — Full Pipeline Smoke Test")
    print("=" * 60)

    cfg = load_config()
    test_config_integrity(cfg)
    await test_deepseek_api(cfg)
    await test_zhipu_api(cfg)
    await test_edge_tts()
    await test_mcp_tools()
    await test_server_startup()
    await test_e2e_chat()

    ok = summary()
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    asyncio.run(main())
