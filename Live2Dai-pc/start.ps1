# Live2Dai — Windows 一键启动脚本（仅启动 + 日志）
# 默认人设: 白
$ProjectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$VtuberDir   = Join-Path $ProjectRoot "open-llm-vtuber"
$VenvPython  = Join-Path $VtuberDir ".venv\Scripts\python.exe"

if (-not (Test-Path $VenvPython)) {
    Write-Host "[Live2Dai] 未找到 .venv，请先运行完整安装或检查路径: $VenvPython" -ForegroundColor Red
    exit 1
}

Set-Location $VtuberDir
Write-Host "[Live2Dai] Starting server... http://localhost:12393" -ForegroundColor Cyan
& $VenvPython run_server.py
if ($LASTEXITCODE -ne 0) {
    Write-Host "[Live2Dai] Server exited with code $LASTEXITCODE" -ForegroundColor Red
}
