# Live2Dai — Windows 一键启动脚本（精简版）
# 默认人设: 白（实际值从 conf.yaml 读取）
# 用法: .\start.ps1 [-SkipEnvCheck] [-SkipFrontendCheck] [-Quiet] [-Config <path>]

param(
    [switch]$Help,
    [string]$Config = "conf.yaml",
    [switch]$SkipEnvCheck,
    [switch]$SkipFrontendCheck,
    [switch]$Quiet
)

$ProjectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot    = Split-Path -Parent $ProjectRoot
$VtuberDir   = Join-Path $ProjectRoot "open-llm-vtuber"
$ConfigPath  = Join-Path $VtuberDir $Config

function Write-Ok   { param([string]$Text) Write-Host "  ✅ $Text" -ForegroundColor Green }
function Write-Warn { param([string]$Text) Write-Host "  ⚠ $Text" -ForegroundColor Yellow }
function Write-Info { param([string]$Text) Write-Host "  ℹ $Text" -ForegroundColor Cyan }
function Write-Err  { param([string]$Text) Write-Host "  ❌ $Text" -ForegroundColor Red }

function Show-Help {
    Write-Host @"
Live2Dai Windows 一键启动脚本（精简版）
用法: .\start.ps1 [选项]

选项:
  -Config <path>        指定 conf.yaml 路径（相对 open-llm-vtuber/）
  -SkipEnvCheck         跳过环境自动安装/创建（仍显示检测结果）
  -SkipFrontendCheck    跳过 Electron 前端检测
  -Quiet                不打印环境摘要，直接启动
  -Help                 显示帮助

示例:
  .\start.ps1                       # 一键启动
  .\start.ps1 -SkipEnvCheck         # 快速启动（已有环境）
  .\start.ps1 -Quiet                # 静默启动
"@
    exit
}

if ($Help) { Show-Help }

if (-not (Test-Path $VtuberDir)) {
    Write-Err "未找到 Open-LLM-VTuber 目录: $VtuberDir"
    Write-Warn "请确认 start.ps1 与 open-llm-vtuber/ 同级"
    exit 1
}

Set-Location $VtuberDir

# ── 读取配置摘要（用于顶部横幅） ──────────────────────────────
$confText = ""
if (Test-Path $ConfigPath) {
    $confText = Get-Content $ConfigPath -Raw -Encoding UTF8
}
$cfgLlm = "unknown"; $cfgTts = "unknown"; $cfgAsr = "disabled"
$cfgPort = "12393";   $cfgFps = "120";    $cfgName = "白"
if ($confText -match "llm_provider:\s*'([^']+)'")        { $cfgLlm  = $Matches[1] }
if ($confText -match "tts_model:\s*'([^']+)'")           { $cfgTts  = $Matches[1] }
if ($confText -match "asr_model:\s*'([^']+)'")           { $cfgAsr  = $Matches[1] }
if ($confText -match "port:\s*(\d+)")                    { $cfgPort = $Matches[1] }
if ($confText -match "live2d_fps:\s*(\d+)")              { $cfgFps  = $Matches[1] }
if ($confText -match "character_name:\s*'([^']+)'")      { $cfgName = $Matches[1] }

Write-Host ""
Write-Host "  Live2Dai · 人设: $cfgName · http://localhost:$cfgPort" -ForegroundColor Cyan
Write-Host "  LLM: $cfgLlm | TTS: $cfgTts | ASR: $cfgAsr | FPS: $cfgFps" -ForegroundColor DarkGray
Write-Host ""

# ── 读取仓库根 .env（与 run_server.py 一致） ──────────────────
$envMap = @{}
$envFile = Join-Path $RepoRoot ".env"
if (Test-Path $envFile) {
    Get-Content $envFile -Encoding UTF8 | ForEach-Object {
        if ($_ -match '^\s*([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.*?)\s*$') {
            $key = $Matches[1]
            $val = $Matches[2].Trim()
            if ($val.Length -ge 2) {
                if ($val[0] -eq '"' -and $val[-1] -eq '"') { $val = $val.Substring(1, $val.Length - 2) }
                elseif ($val[0] -eq "'" -and $val[-1] -eq "'") { $val = $val.Substring(1, $val.Length - 2) }
            }
            $envMap[$key] = $val
        }
    }
}

function Resolve-Key {
    param([string]$Raw)
    $Raw = $Raw.Trim()
    if ($Raw -match '^\$\{([^}]+)\}$') {
        $name = $Matches[1]
        if ($envMap.ContainsKey($name) -and $envMap[$name] -ne "") { return $envMap[$name] }
        $proc = [Environment]::GetEnvironmentVariable($name)
        if ($proc) { return $proc }
        return ""
    }
    return $Raw
}

function Test-KeyUsable {
    param([string]$Key)
    $Key = $Key.Trim()
    if ($Key -eq "" -or $Key -eq "YOUR API KEY HERE" -or $Key -eq "your-api-key") { return $false }
    if ($Key -match '^\$\{.*\}$') { return $false }
    return $true
}

# ── 环境检测（不打印过程，统一在最后摘要） ────────────────────
$pyOk = $false; $pyVer = ""
try {
    $verOut = & python --version 2>&1
    if ($LASTEXITCODE -eq 0 -and $verOut -match "Python\s+(3\.(1[0-9]|[2-9][0-9])\.\d+)") {
        $pyOk = $true; $pyVer = $Matches[1]
    }
} catch { }
if (-not $pyOk) {
    try {
        $verOut = & python3 --version 2>&1
        if ($LASTEXITCODE -eq 0 -and $verOut -match "Python\s+(3\.(1[0-9]|[2-9][0-9])\.\d+)") {
            $pyOk = $true; $pyVer = $Matches[1]
        }
    } catch { }
}

$venvPy = Join-Path $VtuberDir ".venv\Scripts\python.exe"
$venvOk = Test-Path $venvPy

if (-not $SkipEnvCheck -and -not $venvOk -and $pyOk) {
    if (Get-Command "uv" -ErrorAction SilentlyContinue) {
        $null = uv sync --directory $VtuberDir 2>&1
    } else {
        $null = & python -m venv (Join-Path $VtuberDir ".venv") 2>&1
    }
    $venvOk = Test-Path $venvPy
}

$depsOk = $false
if ($venvOk) {
    $null = & $venvPy -c "import open_llm_vtuber" 2>&1
    if ($LASTEXITCODE -eq 0) { $depsOk = $true }
}
if (-not $SkipEnvCheck -and -not $depsOk -and $venvOk) {
    $null = & $venvPy -m pip install -e $VtuberDir 2>&1
    $null = & $venvPy -c "import open_llm_vtuber" 2>&1
    if ($LASTEXITCODE -eq 0) { $depsOk = $true }
}

$ffmpegOk = $false; $ffmpegDetail = ""
$bundledFfmpeg = Join-Path $VtuberDir "bin\ffmpeg.exe"
if (Test-Path $bundledFfmpeg) {
    try {
        $ffOut = & $bundledFfmpeg -version 2>&1
        if ($LASTEXITCODE -eq 0 -and $ffOut -match "ffmpeg version\s+(\S+)") {
            $ffmpegOk = $true; $ffmpegDetail = "$($Matches[1]) (bundled)"
            $env:PATH = "$(Split-Path $bundledFfmpeg);$env:PATH"
        }
    } catch { }
}
if (-not $ffmpegOk) {
    try {
        $ffOut = ffmpeg -version 2>&1
        if ($LASTEXITCODE -eq 0 -and $ffOut -match "ffmpeg version\s+(\S+)") {
            $ffmpegOk = $true; $ffmpegDetail = $Matches[1]
        }
    } catch { }
}

$frontendOk = $false; $frontendPath = ""
if (-not $SkipFrontendCheck) {
    $searchPaths = @(
        "$env:LOCALAPPDATA\Programs\open-llm-vtuber",
        "${env:ProgramFiles(x86)}\open-llm-vtuber",
        "$env:ProgramFiles\open-llm-vtuber"
    )
    foreach ($p in $searchPaths) {
        if (Test-Path $p) { $frontendOk = $true; $frontendPath = $p; break }
    }
}

# API Key 状态（支持 .env 中的真实 Key）
$apiFreeGLM = $confText -match "zhipu_free_llm:"
$apiDeepSeek = $false; $apiZhipu = $false; $apiQwen = $false
if ($confText -match "deepseek_llm:\s*\n\s+llm_api_key:\s*'([^']*)'") {
    $apiDeepSeek = Test-KeyUsable (Resolve-Key $Matches[1])
}
if ($confText -match "zhipu_llm:\s*\n\s+llm_api_key:\s*'([^']*)'") {
    $apiZhipu = Test-KeyUsable (Resolve-Key $Matches[1])
}
if ($confText -match "qwen_llm:\s*\n\s+llm_api_key:\s*'([^']*)'") {
    $apiQwen = Test-KeyUsable (Resolve-Key $Matches[1])
}

# ── 启动摘要 ──────────────────────────────────────────────────
if (-not $Quiet) {
    Write-Host "  环境检查" -ForegroundColor Cyan
    if ($pyOk -and $venvOk -and $depsOk) {
        Write-Ok "Python $pyVer + .venv + 依赖已就绪"
    } elseif ($pyOk -and $venvOk) {
        Write-Warn "Python $pyVer + .venv 就绪，依赖未安装（将尝试自动安装）"
    } elseif ($pyOk) {
        Write-Warn "Python $pyVer 就绪，但 .venv 未创建"
    } else {
        Write-Err "未找到 Python 3.10+（安装: winget install Python.Python.3.12）"
    }

    if ($ffmpegOk) {
        Write-Ok "ffmpeg $ffmpegDetail"
    } else {
        Write-Info "ffmpeg 未安装（仅后端 ASR 需要；浏览器语音输入不需要）"
    }

    if ($frontendOk) {
        Write-Ok "Electron 前端已找到: $frontendPath"
    } else {
        Write-Info "Electron 未安装（浏览器可直接使用，不影响启动）"
    }

    if ($apiFreeGLM) { Write-Ok "免费 GLM 兜底可用（零配置）" }
    if ($apiDeepSeek) { Write-Ok "DeepSeek 已配置" } else { Write-Info "DeepSeek 未配置（可选）" }
    if ($apiZhipu)    { Write-Ok "智谱 GLM 已配置" } else { Write-Info "智谱 GLM 未配置（可选）" }
    if ($apiQwen)     { Write-Ok "Qwen 已配置" } else { Write-Info "Qwen 未配置（可选）" }

    Write-Host ""
    Write-Host "  启动: http://localhost:$cfgPort  (Ctrl+C 停止)" -ForegroundColor Green
    Write-Host ""
}

# ── 启动后端 ──────────────────────────────────────────────────
$launchPython = if ($venvOk -and (Test-Path $venvPy)) { $venvPy } elseif ($pyOk) { "python" } else { "python" }

& $launchPython run_server.py

if ($LASTEXITCODE -ne 0) {
    Write-Host ""
    Write-Err "后端异常退出 (退出代码: $LASTEXITCODE)"
    Write-Warn "按任意键退出..."
    $null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")
}
