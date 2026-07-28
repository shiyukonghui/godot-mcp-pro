# Godot MCP HTTP 端到端测试
import http.client
import json

conn = http.client.HTTPConnection("127.0.0.1", 9877, timeout=10)

# 1. ping
body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": "ping"})
conn.request("POST", "/mcp", body, {"Content-Type": "application/json"})
resp = conn.getresponse()
print("ping:", resp.status, resp.read().decode())

# 2. tools/list
body = json.dumps({"jsonrpc": "2.0", "id": 2, "method": "tools/list"})
conn.request("POST", "/mcp", body, {"Content-Type": "application/json"})
resp = conn.getresponse()
data = json.loads(resp.read())
tools = data.get("result", {}).get("tools", [])
print(f"tools/list: {len(tools)} 个工具")
if tools:
    t0 = tools[0]
    print(f"  首个: name={t0['name']}, inputSchema type={type(t0.get('inputSchema')).__name__}")
    t_last = tools[-1]
    print(f"  末个: name={t_last['name']}, inputSchema type={type(t_last.get('inputSchema')).__name__}")

# 3. 测试一个具体工具
body = json.dumps({
    "jsonrpc": "2.0", "id": 3, "method": "tools/call",
    "params": {"name": "get_project_info", "arguments": {}}
})
conn.request("POST", "/mcp", body, {"Content-Type": "application/json"})
resp = conn.getresponse()
result = json.loads(resp.read())
print(f"tools/call get_project_info:", json.dumps(result, ensure_ascii=False)[:200])

conn.close()
print("\n所有测试通过！")
