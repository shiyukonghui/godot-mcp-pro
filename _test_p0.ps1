$body = '{"jsonrpc":"2.0","id":220,"method":"tools/call","params":{"name":"execute_editor_script","arguments":{"code":"42"}}}'
$r = Invoke-WebRequest -UseBasicParsing -Uri http://127.0.0.1:9877/mcp -Method POST -ContentType application/json -Body $body -TimeoutSec 10
Write-Output $r.Content
