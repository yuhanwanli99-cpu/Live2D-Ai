@echo off
setlocal
chcp 65001 >nul

rem Portable launcher - unlike start.ps1 this does NO python/venv/ffmpeg
rem detection: .venv (managed Python 3.12 + all deps) and bin\ (portable
rem ffmpeg) ship inside the zip.
rem NOTE: this script cd's into its own directory; the venv's pyvenv.cfg
rem resolves 'home' relative to the current working directory, so the
rem server must always be started from here.

cd /d "%~dp0"

rem 1) Put the bundled portable ffmpeg on PATH for THIS process only
set "PATH=%~dp0bin;%PATH%"

rem 2) Lightweight onboarding (first run only)
if not exist ".onboarding_done" (
    echo ==================================================
    echo   Welcome to Live2Dai!  ^(=^o.o^=^)
    echo ==================================================
    echo   Zero-config chat ^| browser opens http://localhost:12393
    echo   LLM: DeepSeek / GLM ^| TTS: Edge TTS ^| ASR: faster-whisper
    echo ==================================================
    echo.
    type nul > ".onboarding_done"
)

rem 3) First run: generate conf.yaml from the template if missing
if not exist "conf.yaml" (
    if exist "conf.yaml.template" (
        copy /y "conf.yaml.template" "conf.yaml" >nul
    ) else (
        echo [Live2Dai] ERROR: conf.yaml.template is missing - cannot create conf.yaml.
        pause
        exit /b 1
    )
)

rem 4) Background poller: open the browser once the backend is ready
rem    (polls http://127.0.0.1:12393 for up to ~3 minutes)
start "" /b powershell -NoProfile -Command "$u='http://localhost:12393'; for($i=0;$i -lt 90;$i++){ try { $r=Invoke-WebRequest -UseBasicParsing -Uri 'http://127.0.0.1:12393' -TimeoutSec 2; if($r.StatusCode -eq 200){ Start-Process $u; break } } catch {}; Start-Sleep -Seconds 2 }" >nul 2>&1

rem 5) Start the backend with the bundled managed Python
rem    (first launch auto-downloads the speech recognition model - please wait)
echo [Live2Dai] Starting backend, please wait...
echo [Live2Dai] Frontend: http://localhost:12393
".venv\Scripts\python.exe" "run_server.py"
set "EXIT_CODE=%ERRORLEVEL%"

if not "%EXIT_CODE%"=="0" (
    echo.
    echo [Live2Dai] Backend exited abnormally (exit code: %EXIT_CODE%^)
    pause
)
exit /b %EXIT_CODE%
