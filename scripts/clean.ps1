# Live2Dai — 清理生成物/缓存（不删除 .venv / node_modules / models）
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $Root

Write-Host "== Removing build/cache/log/agent-work clutter =="
$targets = @(
    "build", "cache", "spoq", ".pytest_cache", ".ruff_cache",
    "s.log", "s2.log", "s_dl.log",
    "Live2Dai-Android\.gradle", "Live2Dai-Android\.kotlin",
    "Live2Dai-Android\app\build", "Live2Dai-Android\app\.cxx"
)
foreach ($t in $targets) {
    if (Test-Path $t) {
        Remove-Item -Recurse -Force $t
        Write-Host "  removed $t"
    }
}
Get-ChildItem -Path . -Recurse -Directory -Force |
    Where-Object { $_.Name -in @("__pycache__", ".pytest_cache", ".ruff_cache") -or $_.Name -like "*.egg-info" } |
    ForEach-Object { Remove-Item -Recurse -Force $_.FullName }

Write-Host "== Done. Local runtime (.venv/node_modules/models) kept. =="
