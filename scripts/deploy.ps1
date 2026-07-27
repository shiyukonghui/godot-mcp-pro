# Godot MCP RS 部署脚本
# 构建 GDExtension DLL 并复制到 Godot 插件目录

param(
    [string]$Profile = "debug",
    [string]$GodotProject = ""  # 可选: 目标 Godot 项目路径
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot
$AddonDir = Join-Path $ProjectRoot "addons\godot_mcp_rs"

Write-Host "=== Godot MCP RS 构建部署 ===" -ForegroundColor Cyan

# 1. 构建 GDExtension
Write-Host "[1/3] 构建 godot_mcp_gdext ($Profile)..." -ForegroundColor Yellow
if ($Profile -eq "release") {
    cargo build --release -p godot_mcp_gdext
    $BuildDir = "release"
} else {
    cargo build -p godot_mcp_gdext
    $BuildDir = "debug"
}

# 2. 复制 DLL 到 addons 目录
Write-Host "[2/3] 复制 DLL 到 $AddonDir ..." -ForegroundColor Yellow
$SourceDll = Join-Path $ProjectRoot "target\$BuildDir\godot_mcp_gdext.dll"
$TargetDll = Join-Path $AddonDir "godot_mcp_gdext.dll"

if (Test-Path $SourceDll) {
    Copy-Item -Force $SourceDll $TargetDll
    Write-Host "  ✓ DLL 已复制: $TargetDll" -ForegroundColor Green
} else {
    Write-Host "  ✗ DLL 未找到: $SourceDll" -ForegroundColor Red
    exit 1
}

# 3. 构建 mcp_bridge
Write-Host "[3/3] 构建 mcp_bridge ($Profile)..." -ForegroundColor Yellow
if ($Profile -eq "release") {
    cargo build --release -p mcp_bridge
    $BridgePath = Join-Path $ProjectRoot "target\$BuildDir\mcp_bridge.exe"
} else {
    cargo build -p mcp_bridge
    $BridgePath = Join-Path $ProjectRoot "target\$BuildDir\mcp_bridge.exe"
}

if (Test-Path $BridgePath) {
    Write-Host "  ✓ 桥接器: $BridgePath" -ForegroundColor Green
}

# 4. 如果指定了 Godot 项目路径，也复制到那里
if ($GodotProject) {
    $TargetAddon = Join-Path $GodotProject "addons\godot_mcp_rs"
    if (-not (Test-Path $TargetAddon)) {
        New-Item -ItemType Directory -Force $TargetAddon | Out-Null
    }
    Copy-Item -Force $TargetDll $TargetAddon\
    Copy-Item -Force (Join-Path $AddonDir "godot_mcp_rs.gdextension") $TargetAddon\
    Copy-Item -Force (Join-Path $AddonDir "plugin.cfg") $TargetAddon\
    Write-Host "  ✓ 已同步到 Godot 项目: $GodotProject" -ForegroundColor Green
}

Write-Host "=== 部署完成 ===" -ForegroundColor Cyan
