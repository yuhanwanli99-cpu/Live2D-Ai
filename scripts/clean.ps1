# Live2D-Ai — 清理生成物/缓存（不删除 .venv / node_modules / models）
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $Root

Write-Host "== Removing build/cache/log/agent-work clutter =="
$targets = @(
    "build", "cache", "spoq", ".pytest_cache", ".ruff_cache",
    "s.log", "s2.log", "s_dl.log",
    "Live2D-Ai-Android\.gradle", "Live2D-Ai-Android\.kotlin",
    "Live2D-Ai-Android\app\build", "Live2D-Ai-Android\app\.cxx"
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
