# Godot MCP HTTP 端点测试（完整输出）
import urllib.request, json

HOST = "127.0.0.1"
HTTP_PORT = 9877

url = f"http://{HOST}:{HTTP_PORT}/mcp"
body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "get_project_info", "arguments": {}}})
req = urllib.request.Request(url, data=body.encode(), headers={"Content-Type": "application/json"})
with urllib.request.urlopen(req, timeout=15) as resp:
    raw = resp.read().decode()
    print("完整响应:")
    print(raw)
    print()
    parsed = json.loads(raw)
    print("解析后:")
    import pprint
    pprint.pprint(parsed)
