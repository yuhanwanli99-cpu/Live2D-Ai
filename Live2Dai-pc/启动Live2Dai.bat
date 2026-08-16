@echo off
chcp 65001 >nul
cd /d "%~dp0"

echo ===================================
echo   Live2Dai - 启动中...
echo ===================================
echo.
echo Live2Dai ^| 白 ^| http://localhost:12393
echo.

powershell -ExecutionPolicy Bypass -File ".\start.ps1"
pause
