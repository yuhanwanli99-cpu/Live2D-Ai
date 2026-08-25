"""Architecture guards that prevent HTTP route god-files from returning."""

from pathlib import Path

PACKAGE = Path(__file__).parents[1] / "src" / "open_llm_vtuber"


def _line_count(path: Path) -> int:
    return len(path.read_text(encoding="utf-8").splitlines())


def test_legacy_routes_module_stays_below_600_lines():
    path = PACKAGE / "routes.py"
    assert _line_count(path) < 50, "routes.py is a compatibility assembly module only"


def test_domain_route_modules_stay_below_500_lines():
    offenders = {
        path.relative_to(PACKAGE).as_posix(): _line_count(path)
        for path in (PACKAGE / "api").glob("*.py")
        if path.name != "config_update.py" and _line_count(path) >= 500
    }
    assert offenders == {}, f"split oversized domain route modules: {offenders}"


def test_routes_does_not_reabsorb_legacy_audio_endpoints():
    source = (PACKAGE / "routes.py").read_text(encoding="utf-8")
    for route in ('@router.post("/asr")', '@router.websocket("/tts-ws")', '@router.websocket("/proxy-ws")'):
        assert route not in source
