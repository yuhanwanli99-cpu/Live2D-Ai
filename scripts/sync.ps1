# Live2Dai — Sync Shared Config → All Consumers
# ================================================================
# 将 shared/ 目录的配置文件同步到所有消费端:
#   - Android assets/
#   - (Desktop 端自动从 shared/ 加载，无需手动同步)
#
# 使用方法:
#   .\sync.ps1
# ================================================================

param(
    [switch]$Verify  # 只检查是否同步，不做实际拷贝
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path

Write-Host "╔════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║   Live2Dai — 配置同步工具              ║" -ForegroundColor Cyan
Write-Host "╚════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""

$SharedDir = Join-Path $ProjectRoot "shared"
$AndroidAssetsDir = Join-Path $ProjectRoot "android-native\app\src\main\assets"
$DesktopConf = Join-Path $ProjectRoot "desktop\open-llm-vtuber\conf.yaml"

# ── Check shared/ exists ──
if (-not (Test-Path $SharedDir)) {
    Write-Host "[ERROR] shared/ 目录不存在: $SharedDir" -ForegroundColor Red
    exit 1
}

# ── Files to sync ──
$SyncFiles = @(
    @{Name="persona.yaml";   Desc="猫娘人设配置"},
    @{Name="mcp_tools.json"; Desc="MCP 工具配置"},
    @{Name="model_dict.json";Desc="模型路由表"}
)

# ── Sync to Android assets ──
Write-Host "[1/2] 同步到 Android assets/" -ForegroundColor Green

if (-not (Test-Path $AndroidAssetsDir)) {
    Write-Host "  [WARN] Android assets 目录不存在，创建: $AndroidAssetsDir" -ForegroundColor Yellow
    New-Item -ItemType Directory -Path $AndroidAssetsDir -Force | Out-Null
}

$syncedCount = 0
foreach ($file in $SyncFiles) {
    $src = Join-Path $SharedDir $file.Name
    $dst = Join-Path $AndroidAssetsDir $file.Name

    if (-not (Test-Path $src)) {
        Write-Host "  [SKIP] 源文件不存在: $($file.Name)" -ForegroundColor DarkGray
        continue
    }

    $needSync = $true
    if (Test-Path $dst) {
        $srcHash = (Get-FileHash $src -Algorithm SHA256).Hash
        $dstHash = (Get-FileHash $dst -Algorithm SHA256).Hash
        if ($srcHash -eq $dstHash) {
            $needSync = $false
            Write-Host "  [OK] $($file.Name) — 已同步" -ForegroundColor DarkGray
        } else {
            Write-Host "  [DIFF] $($file.Name) — 内容不同，需要同步" -ForegroundColor Yellow
        }
    } else {
        Write-Host "  [NEW] $($file.Name) — 目标不存在，创建" -ForegroundColor Yellow
    }

    if ($needSync -and -not $Verify) {
        Copy-Item $src $dst -Force
        Write-Host "  [✓] $($file.Name) 已同步" -ForegroundColor Green
        $syncedCount++
    }
}

# ── Check Desktop conf.yaml references ──
Write-Host ""
Write-Host "[2/2] 检查桌面端 conf.yaml" -ForegroundColor Green

if (Test-Path $DesktopConf) {
    $confContent = Get-Content $DesktopConf -Raw
    if ($confContent -match 'load_persona_from_file') {
        Write-Host "  [OK] conf.yaml 已配置 load_persona_from_file" -ForegroundColor DarkGray
    } else {
        Write-Host "  [WARN] conf.yaml 未使用 load_persona_from_file" -ForegroundColor Yellow
    }
} else {
    Write-Host "  [WARN] conf.yaml 不存在: $DesktopConf" -ForegroundColor Yellow
}

# ── Summary ──
Write-Host ""
if ($Verify) {
    if ($syncedCount -eq 0) {
        Write-Host "✓ 所有配置文件已同步" -ForegroundColor Green
    } else {
        Write-Host "⚠ $syncedCount 个文件需要同步。运行不带 -Verify 的 sync.ps1 来执行同步。" -ForegroundColor Yellow
    }
} else {
    if ($syncedCount -gt 0) {
        Write-Host "✓ 已同步 $syncedCount 个文件" -ForegroundColor Green
    } else {
        Write-Host "✓ 所有文件无需更新" -ForegroundColor Green
    }
}
