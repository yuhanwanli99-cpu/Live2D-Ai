# Live2Dai — Windows 桌面端一键启动脚本（增强版）
# ===============================================================
# 自动检测 Python、创建虚拟环境、安装依赖、启动后端
# 使用: .\start.ps1 [-SkipEnvCheck] [-SkipFrontendCheck] [-Config <path>]
# ===============================================================

param(
    [switch]$Help,
    [string]$Config = "conf.yaml",
    [switch]$SkipEnvCheck,
    [switch]$SkipFrontendCheck
)

# ═══════════════════════════════════════════════════════
# 辅助函数
# ═══════════════════════════════════════════════════════

function Write-Status {
    param([string]$Label, [bool]$Ok, [string]$Detail = "")
    $icon = if ($Ok) { "✅" } else { "❌" }
    $color = if ($Ok) { "Green" } else { "Red" }
    if ($Detail) {
        Write-Host "  $icon $Label $Detail" -ForegroundColor $color
    } else {
        Write-Host "  $icon $Label" -ForegroundColor $color
    }
}

function Show-Help {
    Write-Host @"
Live2Dai Windows 桌面端启动脚本（增强版）
========================================
用法: .\start.ps1 [选项]

选项:
  -Config <path>       指定 conf.yaml 路径（相对于 open-llm-vtuber 目录）
  -SkipEnvCheck        跳过环境检测（Python/venv/ffmpeg/API Key）
  -SkipFrontendCheck   跳过前端检测
  -Help                显示此帮助信息

启动完整流程:
  1. 检测 Python 3.10+  -> 未安装则提示安装命令
  2. 检测/创建 .venv    -> uv sync 或 python -m venv
  3. 安装依赖           -> pip install -e . (使用 venv)
  4. 检测 ffmpeg        -> 可选，仅提示
  5. 检测 Electron 前端 -> 检查安装路径
  6. 检测 API Key       -> 检查 conf.yaml 中密钥状态
  7. 显示环境摘要面板
  8. 启动后端服务器

示例:
  .\start.ps1                          # 一键启动（完整检测）
  .\start.ps1 -Config conf.yaml        # 指定配置
  .\start.ps1 -SkipEnvCheck            # 跳过环境检测，快速启动
  .\start.ps1 -SkipFrontendCheck       # 跳过前端检测
"@
    exit
}

# ═══════════════════════════════════════════════════════
# 路径初始化
# ═══════════════════════════════════════════════════════

$ProjectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$VtuberDir   = Join-Path $ProjectRoot "open-llm-vtuber"
$ConfigPath  = Join-Path $VtuberDir $Config

if (-not (Test-Path $VtuberDir)) {
    Write-Host "[ERROR] 未找到 Open-LLM-VTuber 目录: $VtuberDir" -ForegroundColor Red
    Write-Host "请确认项目结构完整: start.ps1 与 open-llm-vtuber/ 同级" -ForegroundColor Yellow
    exit 1
}

Set-Location $VtuberDir

# ═══════════════════════════════════════════════════════
# Step 0: 显示 Banner
# ═══════════════════════════════════════════════════════

Write-Host ""
Write-Host "╔════════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║         Live2Dai -- 桌宠后端启动器            ║" -ForegroundColor Cyan
Write-Host "╠════════════════════════════════════════════════╣" -ForegroundColor Cyan
Write-Host "║ LLM: DeepSeek V4 Flash | GLM-4.6V (视觉)    ║" -ForegroundColor Cyan
Write-Host "║ TTS: Edge TTS (免费)    ASR: faster-whisper  ║" -ForegroundColor Cyan
Write-Host "║ FPS: 120  端口: 12393  人设: 猫娘女仆       ║" -ForegroundColor Cyan
Write-Host "╚════════════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""

# ═══════════════════════════════════════════════════════
# 状态变量（全局，供摘要面板读取）
# ═══════════════════════════════════════════════════════

$Script:PythonVer   = $null      # 检测到的 Python 版本字符串
$Script:PythonOk    = $false     # Python 检测通过
$Script:VenvOk      = $false     # 虚拟环境就绪
$Script:DepsOk      = $false     # 依赖已安装
$Script:FfmpegOk    = $false     # ffmpeg 可用
$Script:FrontendOk  = $false     # Electron 前端已安装
$Script:ApiDeepSeek = $false     # DeepSeek API Key 已配置
$Script:ApiZhipu    = $false     # 智谱 API Key 已配置
$Script:ApiFreeGLM  = $false     # 零配置免费 GLM 兜底已就绪（zhipu_free_llm 存在即视为开箱即用）
$Script:VenvPython  = $null      # venv 中 python.exe 路径
$Script:LaunchMsg   = ""         # 额外的启动前提示

# ═══════════════════════════════════════════════════════
# Step 1: 检测 Python 环境
# ═══════════════════════════════════════════════════════

function Test-PythonInstall {
    Write-Host "[1/6] 检测 Python 环境..." -ForegroundColor Cyan
    try {
        $verOut = & python --version 2>&1
        if ($LASTEXITCODE -eq 0 -and $verOut -match "Python\s+(3\.(1[0-9]|[2-9][0-9])\.\d+)") {
            $Script:PythonVer = $Matches[1]
            $Script:PythonOk  = $true
            Write-Status "Python" $true "$($Script:PythonVer) (系统 PATH)"
            return $true
        }
    } catch {
        # python not found
    }

    # 尝试 python3 别名 (Windows Store Python 有时用 python3)
    try {
        $verOut = & python3 --version 2>&1
        if ($LASTEXITCODE -eq 0 -and $verOut -match "Python\s+(3\.(1[0-9]|[2-9][0-9])\.\d+)") {
            $Script:PythonVer = $Matches[1]
            $Script:PythonOk  = $true
            Write-Status "Python" $true "$($Script:PythonVer) (python3)"
            return $true
        }
    } catch {
        # python3 not found
    }

    Write-Status "Python 3.10+" $false "未找到"
    Write-Host ""
    Write-Host "  => 请安装 Python 3.10+:" -ForegroundColor Yellow
    Write-Host "     winget install Python.Python.3.12" -ForegroundColor Yellow
    Write-Host "     或下载: https://www.python.org/downloads/" -ForegroundColor Yellow
    return $false
}

# ═══════════════════════════════════════════════════════
# Step 2: 检测/创建虚拟环境
# ═══════════════════════════════════════════════════════

function Ensure-Venv {
    param([string]$TargetDir)

    Write-Host "[2/6] 检测虚拟环境..." -ForegroundColor Cyan

    $venvPath   = Join-Path $TargetDir ".venv"
    $venvPython = Join-Path $venvPath "Scripts\python.exe"

    if (Test-Path $venvPython) {
        Write-Status "虚拟环境 (.venv)" $true "已存在"
        $Script:VenvOk = $true
        return $venvPython
    }

    Write-Host "  -> 未检测到 .venv，正在创建..." -ForegroundColor Yellow

    # 优先尝试 uv sync
    $uvAvailable = $null -ne (Get-Command "uv" -ErrorAction SilentlyContinue)
    if ($uvAvailable) {
        Write-Host "  -> 使用 uv sync (创建 venv + 安装依赖)..." -ForegroundColor Yellow
        try {
            $null = uv sync --directory $TargetDir 2>&1
            if (Test-Path $venvPython) {
                Write-Status "虚拟环境 (.venv)" $true "已创建 (uv)"
                $Script:VenvOk    = $true
                $Script:DepsOk    = $true
                return $venvPython
            }
        } catch {
            Write-Host "  -> uv sync 失败，降级到 python -m venv" -ForegroundColor Yellow
        }
    } else {
        Write-Host "  -> uv 未安装，使用 python -m venv" -ForegroundColor Yellow
    }

    # 降级: python -m venv
    try {
        $null = & python -m venv $venvPath 2>&1
        if (Test-Path $venvPython) {
            Write-Status "虚拟环境 (.venv)" $true "已创建 (venv)"
            $Script:VenvOk = $true
            return $venvPython
        }
    } catch {
        # failed
    }

    Write-Status "虚拟环境 (.venv)" $false "创建失败"
    Write-Host "  => 请手动创建: python -m venv .venv" -ForegroundColor Yellow
    return $null
}

# ═══════════════════════════════════════════════════════
# Step 3: 安装依赖
# ═══════════════════════════════════════════════════════

function Ensure-Dependencies {
    param([string]$VenvPython)

    Write-Host "[3/6] 检查 Python 依赖..." -ForegroundColor Cyan

    if (-not $VenvPython -or -not (Test-Path $VenvPython)) {
        Write-Status "依赖" $false "无可用 Python"
        return $false
    }

    # 快速检测是否已安装
    try {
        $result = & $VenvPython -c "import open_llm_vtuber; print('ok')" 2>&1
        if ($LASTEXITCODE -eq 0 -and $result.Trim() -eq "ok") {
            Write-Status "依赖" $true "已安装 (open_llm_vtuber)"
            $Script:DepsOk = $true
            return $true
        }
    } catch {
        # need install
    }

    Write-Host "  -> 依赖未安装，运行 pip install -e . ..." -ForegroundColor Yellow
    try {
        $null = & $VenvPython -m pip install -e $VtuberDir 2>&1
        # 验证安装
        $result = & $VenvPython -c "import open_llm_vtuber; print('ok')" 2>&1
        if ($LASTEXITCODE -eq 0 -and $result.Trim() -eq "ok") {
            Write-Status "依赖" $true "已安装 (open_llm_vtuber)"
            $Script:DepsOk = $true
            return $true
        }
    } catch {
        Write-Host "  -> pip install 失败: $_" -ForegroundColor Red
    }

    Write-Status "依赖" $false "安装失败"
    Write-Host "  => 请手动执行: cd open-llm-vtuber && pip install -e ." -ForegroundColor Yellow
    return $false
}

# ═══════════════════════════════════════════════════════
# Step 4: 检测 ffmpeg
# ═══════════════════════════════════════════════════════

function Test-FFmpeg {
    Write-Host "[4/6] 检测系统依赖 (ffmpeg)..." -ForegroundColor Cyan
    try {
        $ffOut = ffmpeg -version 2>&1
        if ($LASTEXITCODE -eq 0 -and $ffOut -match "ffmpeg version\s+(\S+)") {
            $ver = $Matches[1]
            $Script:FfmpegOk = $true
            Write-Status "ffmpeg" $true $ver
            return $true
        }
    } catch {
        # not found
    }
    Write-Status "ffmpeg" $false "未安装"
    Write-Host "  -> ASR (faster-whisper) 需要 ffmpeg，安装命令:" -ForegroundColor Yellow
    Write-Host "     winget install Gyan.FFmpeg" -ForegroundColor Yellow
    Write-Host "     或: https://ffmpeg.org/download.html" -ForegroundColor Yellow
    return $false
}

# ═══════════════════════════════════════════════════════
# Step 5: 检测 Electron 前端
# ═══════════════════════════════════════════════════════

function Test-ElectronFrontend {
    Write-Host "[5/6] 检测 Electron 前端..." -ForegroundColor Cyan
    $searchPaths = @(
        "$env:LOCALAPPDATA\Programs\open-llm-vtuber",
        "${env:ProgramFiles(x86)}\open-llm-vtuber",
        "$env:ProgramFiles\open-llm-vtuber"
    )
    foreach ($p in $searchPaths) {
        if (Test-Path $p) {
            $Script:FrontendOk = $true
            Write-Status "Electron 前端" $true $p
            return $true
        }
    }
    Write-Status "Electron 前端" $false "未找到"
    Write-Host "  => 请从以下地址下载安装:" -ForegroundColor Yellow
    Write-Host "     https://github.com/Open-LLM-VTuber/Open-LLM-VTuber/releases" -ForegroundColor Yellow
    return $false
}

# ═══════════════════════════════════════════════════════
# Step 6: 检测 API Key 状态
# ═══════════════════════════════════════════════════════

function Get-ApiKeyStatus {
    Write-Host "[6/6] 检测 API Key 状态..." -ForegroundColor Cyan

    if (-not (Test-Path $ConfigPath)) {
        Write-Status "API Key" $false "配置文件不存在: $ConfigPath"
        return
    }

    try {
        $yaml = Get-Content $ConfigPath -Raw -Encoding UTF8

        # 零配置默认：zhipu_free_llm 存在（免费 GLM-4.7-Flash 兜底，代码层内置共享 Key）
        # 即视为开箱即用，无需用户先申请/配置任何 Key。
        if ($yaml -match "zhipu_free_llm:") {
            $Script:ApiFreeGLM = $true
        }

        # 占位符（${VAR}）视为未配置，避免把未解析的环境变量误报为“已配置”
        $placeholderPattern = '^\$\{.*\}$'

        # DeepSeek（可选升级路径）
        if ($yaml -match "deepseek_llm:\s*\n\s+llm_api_key:\s*'([^']+)'") {
            $key = $Matches[1].Trim()
            if ($key -ne "" -and $key -ne "YOUR API KEY HERE" -and $key -ne "your-api-key" -and $key -notmatch $placeholderPattern) {
                $Script:ApiDeepSeek = $true
            }
        }
        # 智谱
        if ($yaml -match "zhipu_llm:\s*\n\s+llm_api_key:\s*'([^']+)'") {
            $key = $Matches[1].Trim()
            if ($key -ne "" -and $key -ne "YOUR API KEY HERE" -and $key -ne "your-api-key" -and $key -notmatch $placeholderPattern) {
                $Script:ApiZhipu = $true
            }
        }
    } catch {
        Write-Status "API Key" $false "解析失败: $_"
        return
    }

    if ($Script:ApiFreeGLM) { Write-Status "免费 GLM 兜底 (零配置)" $true "已就绪，无需配置 Key" }

    if ($Script:ApiDeepSeek) { Write-Status "API Key (DeepSeek)" $true "已配置" }
    else                     { Write-Status "API Key (DeepSeek)" $false "未配置（可选，升级用）" }

    if ($Script:ApiZhipu)    { Write-Status "API Key (智谱 GLM-4V)" $true "已配置" }
    else                     { Write-Status "API Key (智谱 GLM-4V)" $false "未配置（可选，视觉用）" }

    if ($Script:ApiFreeGLM) {
        $Script:LaunchMsg = "ℹ 对话使用零配置免费 GLM 兜底（共享额度，限 1 并发）；如需更高并发可在 conf.yaml 配置自己的 DeepSeek/Zhipu Key"
    } elseif (-not $Script:ApiDeepSeek -and -not $Script:ApiZhipu) {
        $Script:LaunchMsg = "⚠ 未检测到零配置免费兜底与任何 API Key，请检查 conf.yaml"
    } elseif (-not $Script:ApiDeepSeek) {
        $Script:LaunchMsg = "⚠ DeepSeek API Key 未配置，对话功能可能异常"
    } elseif (-not $Script:ApiZhipu) {
        $Script:LaunchMsg = "⚠ 智谱 GLM-4V API Key 未配置，视觉功能可能异常"
    }
}

# ═══════════════════════════════════════════════════════
# Step 7: 显示环境摘要面板
# ═══════════════════════════════════════════════════════

function Show-SummaryPanel {
    Write-Host ""
    Write-Host "╔══════════════════════════════════════════════════╗" -ForegroundColor Cyan
    Write-Host "║           Live2Dai -- 环境摘要                   ║" -ForegroundColor Cyan
    Write-Host "╠══════════════════════════════════════════════════╣" -ForegroundColor Cyan

    # Python
    if ($Script:PythonOk) {
        $pyVer = if ($Script:PythonVer) { $Script:PythonVer } else { "3.10+" }
        $venvTag = if ($Script:VenvOk) { "(venv: .venv)" } else { "" }
        Write-Host ("║  $(if ($Script:PythonOk) { '✅' } else { '❌' }) Python     $pyVer $venvTag") -ForegroundColor Green
    } else {
        Write-Host ("║  ❌ Python     未安装") -ForegroundColor Red
    }

    # ffmpeg
    $ffIcon = if ($Script:FfmpegOk) { "✅" } else { "❌" }
    $ffColor = if ($Script:FfmpegOk) { "Green" } else { "Red" }
    if ($Script:FfmpegOk) {
        Write-Host "║  $ffIcon ffmpeg    已安装                        " -ForegroundColor $ffColor
    } else {
        Write-Host "║  $ffIcon ffmpeg    未安装 (ASR 可能需要)        " -ForegroundColor $ffColor
    }

    # Electron
    $feIcon = if ($Script:FrontendOk) { "✅" } else { "❌" }
    $feColor = if ($Script:FrontendOk) { "Green" } else { "Red" }
    if ($Script:FrontendOk) {
        Write-Host "║  $feIcon Electron  已安装 (Open-LLM-VTuber)      " -ForegroundColor $feColor
    } else {
        Write-Host "║  $feIcon Electron  未安装 (前端不可用)          " -ForegroundColor $feColor
    }

    # API Keys
    $dsIcon  = if ($Script:ApiDeepSeek) { "✅" } else { "❌" }
    $dsColor = if ($Script:ApiDeepSeek) { "Green" } else { "Red" }
    Write-Host "║  $dsIcon DeepSeek  $("已配置" -f $(if ($Script:ApiDeepSeek) { "已配置" } else { "未配置" }))" -ForegroundColor $dsColor

    $zpIcon  = if ($Script:ApiZhipu) { "✅" } else { "❌" }
    $zpColor = if ($Script:ApiZhipu) { "Green" } else { "Red" }
    Write-Host "║  $zpIcon 智谱 GLM  $("已配置" -f $(if ($Script:ApiZhipu) { "已配置" } else { "未配置" }))" -ForegroundColor $zpColor

    # Dependencies
    if ($Script:DepsOk) {
        Write-Host ("║  $(if ($Script:DepsOk) { '✅' } else { '❌' }) 依赖      已安装 (open_llm_vtuber)      ") -ForegroundColor Green
    } else {
        Write-Host ("║  ❌ 依赖      未安装                          ") -ForegroundColor Red
    }

    Write-Host "╠══════════════════════════════════════════════════╣" -ForegroundColor Cyan
    Write-Host "║  模式: 独立直连 API | 端口: 12393 | TTS: Edge  ║" -ForegroundColor Cyan
    Write-Host "╚══════════════════════════════════════════════════╝" -ForegroundColor Cyan

    if ($Script:LaunchMsg) {
        Write-Host ""
        Write-Host $Script:LaunchMsg -ForegroundColor Yellow
    }
    Write-Host ""
}

# ═══════════════════════════════════════════════════════
# 主入口
# ═══════════════════════════════════════════════════════

# 帮助
if ($Help) { Show-Help }

# ── 环境检测 ──
if (-not $SkipEnvCheck) {
    # Step 1
    $null = Test-PythonInstall

    # Step 2 (依赖 Python)
    $venvPy = $null
    if ($Script:PythonOk) {
        $venvPy = Ensure-Venv $VtuberDir
        $Script:VenvPython = $venvPy
    }

    # Step 3 (依赖 venv)
    if ($Script:VenvOk -and $Script:VenvPython) {
        $null = Ensure-Dependencies $Script:VenvPython
    } elseif ($Script:PythonOk) {
        # venv 创建失败，尝试直接用系统 python 安装
        Write-Host "[3/6] 虚拟环境不可用，跳过依赖自动安装" -ForegroundColor Yellow
    }

    # Step 4
    $null = Test-FFmpeg

    # Step 6 (独立，仅需配置文件)
    $null = Get-ApiKeyStatus
} else {
    Write-Host "[跳过] -SkipEnvCheck 已指定，跳过环境检测" -ForegroundColor Yellow
    # 仍尝试找 venv python 用于启动
    $fallbackVenv = Join-Path $VtuberDir ".venv\Scripts\python.exe"
    if (Test-Path $fallbackVenv) {
        $Script:VenvPython = $fallbackVenv
        $Script:VenvOk = $true
    }
    # 检测 ffmpeg (轻量)
    $Script:FfmpegOk = $null -ne (Get-Command "ffmpeg" -ErrorAction SilentlyContinue)
}

# ── 前端检测 ──
if (-not $SkipFrontendCheck) {
    $null = Test-ElectronFrontend
} else {
    Write-Host "[跳过] -SkipFrontendCheck 已指定，跳过前端检测" -ForegroundColor Yellow
}

# ── 显示环境摘要 ──
Show-SummaryPanel

# ── 确定启动用 Python 路径 ──
$launchPython = if ($Script:VenvPython -and (Test-Path $Script:VenvPython)) {
    $Script:VenvPython
} elseif ($Script:PythonOk) {
    "python"
} else {
    # 最后尝试：直接找 .venv
    $fallback = Join-Path $VtuberDir ".venv\Scripts\python.exe"
    if (Test-Path $fallback) { $fallback } else { "python" }
}

# ── 启动后端服务器 ──
Write-Host "╔════════════════════════════════════════════════╗" -ForegroundColor Green
Write-Host "║           启动后端服务器...                    ║" -ForegroundColor Green
Write-Host "╠════════════════════════════════════════════════╣" -ForegroundColor Green
Write-Host "║ Python:  $launchPython" -ForegroundColor Green
Write-Host "║ 配置:    $ConfigPath" -ForegroundColor Green
Write-Host "║ 目录:    $(Get-Location)" -ForegroundColor Green
Write-Host "╚════════════════════════════════════════════════╝" -ForegroundColor Green
Write-Host ""

& $launchPython run_server.py

# ── 异常退出处理 ──
if ($LASTEXITCODE -ne 0) {
    Write-Host ""
    Write-Host "[Live2Dai] 后端异常退出 (退出代码: $LASTEXITCODE)" -ForegroundColor Red
    Write-Host "按任意键退出..." -ForegroundColor Yellow
    $null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")
}
