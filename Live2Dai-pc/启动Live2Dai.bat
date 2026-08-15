@echo off
chcp 65001 >nul
cd /d "%~dp0"

echo ===================================
echo   Live2Dai - 启动中...
echo ===================================
echo.
echo DeepSeek V4 Flash ^| 120fps ^| Edge TTS
echo.

powershell -ExecutionPolicy Bypass -File ".\start.ps1"
pause
