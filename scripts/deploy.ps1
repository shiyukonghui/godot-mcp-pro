# Godot MCP RS 部署脚本
# 构建 GDExtension DLL 并复制到 Godot 插件目录

$BuildProfile = if ($args -contains "-release") { "release" } else { "debug" }
$GodotProject = "F:\UE5\ly2"
$SkipBuild = $args -contains "-SkipBuild"

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot
$AddonDir = Join-Path $ProjectRoot "addons\godot_mcp_rs"

Write-Host "=== Godot MCP RS 构建部署 ===" -ForegroundColor Cyan

# 1. 构建 GDExtension
if (-not $SkipBuild) {
    Write-Host "[1/4] 构建 godot_mcp_gdext ($BuildProfile)..." -ForegroundColor Yellow
    if ($BuildProfile -eq "release") {
        cargo build --release -p godot_mcp_gdext
    } else {
        cargo build -p godot_mcp_gdext
    }
    if ($LASTEXITCODE -ne 0) {
        throw "GDExtension 构建失败，退出码: $LASTEXITCODE"
    }
} else {
    Write-Host "[1/4] 跳过构建，使用现有产物" -ForegroundColor DarkYellow
}
$BuildDir = if ($BuildProfile -eq "release") { "release" } else { "debug" }

# 2. 复制 DLL 到 addons 目录
Write-Host "[2/4] 同步仓库插件 DLL 到 $AddonDir ..." -ForegroundColor Yellow
$SourceDll = Join-Path $ProjectRoot "target\$BuildDir\godot_mcp_gdext.dll"
$TargetDll = Join-Path $AddonDir "godot_mcp_gdext.dll"

if (Test-Path $SourceDll) {
    Copy-Item -Force $SourceDll $TargetDll
    Write-Host "  ✓ DLL 已复制: $TargetDll" -ForegroundColor Green
} else {
    Write-Host "  ✗ DLL 未找到: $SourceDll" -ForegroundColor Red
    exit 1
}

# 3. 关闭 Godot，释放已加载的 DLL
Write-Host "[3/4] 关闭 Godot 进程..." -ForegroundColor Yellow
$GodotProcesses = Get-Process -Name "Godot*" -ErrorAction SilentlyContinue
if ($GodotProcesses) {
    $GodotProcesses | Stop-Process -Force
    $GodotProcesses | Wait-Process -ErrorAction SilentlyContinue
    Write-Host "  ✓ Godot 进程已关闭" -ForegroundColor Green
} else {
    Write-Host "  - 未发现运行中的 Godot 进程" -ForegroundColor DarkGray
}

# 4. 同步到实际 Godot 项目并校验 DLL 哈希
if ($GodotProject) {
    Write-Host "[4/4] 同步到 Godot 项目并校验哈希..." -ForegroundColor Yellow
    $TargetAddon = Join-Path $GodotProject "addons\godot_mcp_rs"
    if (-not (Test-Path $TargetAddon)) {
        New-Item -ItemType Directory -Force $TargetAddon | Out-Null
    }
    Copy-Item -Force $TargetDll (Join-Path $TargetAddon "godot_mcp_gdext.dll")
    Copy-Item -Force (Join-Path $AddonDir "godot_mcp_rs.gdextension") $TargetAddon
    Copy-Item -Force (Join-Path $AddonDir "plugin.cfg") $TargetAddon
    # 同步 GDScript 文件
    Copy-Item -Force (Join-Path $AddonDir "plugin.gd") $TargetAddon
    Copy-Item -Force (Join-Path $AddonDir "mcp_runtime_agent.gd") $TargetAddon
    $ProjectDll = Join-Path $TargetAddon "godot_mcp_gdext.dll"
    $SourceHash = (Get-FileHash $SourceDll -Algorithm SHA256).Hash
    $TargetHash = (Get-FileHash $ProjectDll -Algorithm SHA256).Hash
    if ($SourceHash -ne $TargetHash) {
        throw "DLL 哈希校验失败：源文件 $SourceHash，目标文件 $TargetHash"
    }
    Write-Host "  ✓ 已同步到 Godot 项目: $GodotProject" -ForegroundColor Green
    Write-Host "  ✓ SHA-256: $TargetHash" -ForegroundColor Green
}

Write-Host "=== 部署完成 ===" -ForegroundColor Cyan
