# Live2Dai — Sync Shared Config → Android Assets
# ================================================================
# 将 shared/ 中的跨端共享配置同步到 Android assets/。
# PC 端通过 conf.yaml 的 load_persona_from_file 直接读取 shared/，无需复制。
#
# 用法:
#   .\sync.ps1            # 执行同步
#   .\sync.ps1 -Verify    # 只检查差异，不写入
# ================================================================

param(
    [switch]$Verify
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$SharedDir = Join-Path $ProjectRoot "shared"
$AndroidAssetsDir = Join-Path $ProjectRoot "Live2Dai-Android\app\src\main\assets"

Write-Host "Live2Dai — 配置同步工具 (shared -> Android assets)" -ForegroundColor Cyan
Write-Host ""

if (-not (Test-Path $SharedDir)) {
    Write-Host "[ERROR] shared/ 目录不存在: $SharedDir" -ForegroundColor Red
    exit 1
}

# 需要同步到 Android assets 的文件（model_dict.json 在 Android 端已无引用，不同步）
$SyncFiles = @(
    @{Name="persona.yaml";   Desc="人设配置"},
    @{Name="mcp_tools.json"; Desc="MCP 工具配置"},
    @{Name="model_registry.json"; Desc="模型注册表"}
)

if (-not (Test-Path $AndroidAssetsDir)) {
    Write-Host "[WARN] 创建 Android assets 目录: $AndroidAssetsDir" -ForegroundColor Yellow
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

# 检查 PC 端 conf.yaml 是否引用 shared/persona.yaml（仅提示，不修改）
$DesktopConf = Join-Path $ProjectRoot "Live2Dai-pc\open-llm-vtuber\conf.yaml"
if (Test-Path $DesktopConf) {
    $confContent = Get-Content $DesktopConf -Raw
    if ($confContent -match 'load_persona_from_file') {
        Write-Host "  [OK] PC conf.yaml 已配置 load_persona_from_file" -ForegroundColor DarkGray
    } else {
        Write-Host "  [WARN] PC conf.yaml 未使用 load_persona_from_file" -ForegroundColor Yellow
    }
} else {
    Write-Host "  [WARN] PC conf.yaml 不存在: $DesktopConf" -ForegroundColor Yellow
}

Write-Host ""
if ($Verify) {
    if ($syncedCount -eq 0) {
        Write-Host "✓ 所有配置文件已同步" -ForegroundColor Green
    } else {
        Write-Host "⚠ $syncedCount 个文件需要同步。运行不带 -Verify 的 sync.ps1 执行同步。" -ForegroundColor Yellow
    }
} else {
    if ($syncedCount -gt 0) {
        Write-Host "✓ 已同步 $syncedCount 个文件" -ForegroundColor Green
    } else {
        Write-Host "✓ 所有文件无需更新" -ForegroundColor Green
    }
}
