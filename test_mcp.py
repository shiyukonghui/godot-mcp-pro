# Godot MCP 端到端测试
import socket, json

HOST = "127.0.0.1"
PORT = 9876

def mcp_call(method, params=None):
    """发送 MCP 请求（newline-delimited JSON 到 TCP）"""
    s = socket.socket()
    s.settimeout(15)
    s.connect((HOST, PORT))
    req = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params or {}})
    s.sendall((req + "\n").encode())
    # 逐行读取响应
    buf = b""
    while True:
        c = s.recv(1)
        if not c or c == b"\n":
            break
        buf += c
    s.close()
    return json.loads(buf.decode()) if buf else None

print("===== Godot MCP 连接测试 =====")

# 1. 初始化
print("[1/4] initialize...")
resp = mcp_call("initialize", {"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "test", "version": "1.0"}})
print(f"  server: {resp.get('result', {}).get('serverInfo', {})}")

# 2. 工具列表
print("[2/4] tools/list...")
resp = mcp_call("tools/list")
if resp and "result" in resp:
    tools = resp["result"]["tools"]
    print(f"  工具总数: {len(tools)}")
    for t in tools[:10]:
        print(f"    - {t['name']}")
else:
    print(f"  失败: {resp}")

# 3. 调用工具: get_project_info
print("[3/4] tools/call: get_project_info...")
resp = mcp_call("tools/call", {"name": "get_project_info", "arguments": {}})
if resp and "result" in resp:
    content = resp["result"]["content"]
    print(f"  响应: {content}")
else:
    print(f"  失败: {resp}")

# 4. 调用工具: get_audio_info
print("[4/4] tools/call: get_audio_info...")
resp = mcp_call("tools/call", {"name": "get_audio_info", "arguments": {}})
if resp and "result" in resp:
    content = resp["result"]["content"]
    print(f"  响应: {content}")
else:
    print(f"  失败: {resp}")

print("===== 测试完成 =====")
