# Godot MCP 连接测试脚本
# MCP 使用 newline-delimited JSON，每次读取一行即可
param([string]$Port = "9876")

function Send-Mcp {
    param([string]$Method, $Params)

    $req = @{jsonrpc = "2.0"; id = 1; method = $Method}
    if ($Params) { $req.params = $Params }

    $msg = ($req | ConvertTo-Json -Compress) + "`n"
    $tcp = New-Object System.Net.Sockets.TcpClient('127.0.0.1', [int]$Port)
    $stream = $tcp.GetStream()
    $writer = New-Object System.IO.StreamWriter($stream)
    $writer.Write($msg)
    $writer.Flush()

    # 逐行读取响应（MCP 协议：一行一个 JSON）
    $reader = New-Object System.IO.StreamReader($stream)
    $line = $reader.ReadLine()
    $tcp.Close()
    return $line
}

Write-Host "===== Godot MCP 连接测试 =====" -ForegroundColor Cyan

# 1. list_tools
Write-Host "[1/3] list_tools..." -ForegroundColor Yellow
$resp = Send-Mcp "list_tools" $null
$data = $resp | ConvertFrom-Json
$count = $data.result.tools.Count
Write-Host "  ✅ 工具总数: $count" -ForegroundColor Green
$data.result.tools[0..9] | ForEach-Object { Write-Host "    - $($_.name)" }

# 2. get_project_info
Write-Host "[2/3] call_tool: get_project_info..." -ForegroundColor Yellow
$resp = Send-Mcp "call_tool" @{name = "get_project_info"; arguments = @{}}
Write-Host "  ✅ 响应: $resp" -ForegroundColor Green

# 3. get_audio_info
Write-Host "[3/3] call_tool: get_audio_info..." -ForegroundColor Yellow
$resp = Send-Mcp "call_tool" @{name = "get_audio_info"; arguments = @{}}
Write-Host "  ✅ 响应: $resp" -ForegroundColor Green

Write-Host "===== 测试完成 =====" -ForegroundColor Cyan
