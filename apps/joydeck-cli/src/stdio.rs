//! MCP 2025-11-25 and JoyDeck control-1 stdio transport; stdout contains protocol only.
use joydeck_shell::automation::{Dispatcher, LIMIT, Request, VERSION};
use serde_json::{Value, json};
use std::{
    io::{BufRead, Read, Write},
    path::Path,
};

pub fn run(root: &Path, mcp: bool) -> Result<(), String> {
    let mut dispatcher = Dispatcher::new(root)?;
    let mut initialized = false;
    let mut ready = false;
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    loop {
        let mut line = vec![];
        let count = input
            .by_ref()
            .take((LIMIT + 1) as u64)
            .read_until(b'\n', &mut line)
            .map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        if line.len() > LIMIT {
            let response = json!({"jsonrpc":"2.0","id":null,"error":{"code":-32600,"message":"request exceeds 1 MiB; closing stdio"}});
            writeln!(output, "{response}").map_err(|e| e.to_string())?;
            output.flush().map_err(|e| e.to_string())?;
            return Err("protocol framing limit".into());
        }
        let parsed = serde_json::from_slice::<Value>(&line);
        let response = if !mcp {
            match parsed.and_then(serde_json::from_value::<Request>) {
                Ok(r) => dispatcher.dispatch(r),
                Err(e) => json!({"error":{"code":"invalid-request","message":e.to_string()}}),
            }
        } else {
            let request = match parsed {
                Ok(v) => v,
                Err(_) => {
                    writeln!(output, "{}", rpc_error(Value::Null, -32700, "Parse error"))
                        .map_err(|e| e.to_string())?;
                    output.flush().map_err(|e| e.to_string())?;
                    continue;
                }
            };
            if request["jsonrpc"] != "2.0"
                || !request.is_object()
                || request.get("id").is_some_and(|id| {
                    !id.is_string() && id.as_i64().is_none() && id.as_u64().is_none()
                })
            {
                rpc_error(request["id"].clone(), -32600, "Invalid Request")
            } else if request.get("id").is_none() {
                if request["method"] == "notifications/initialized" && initialized {
                    ready = true;
                }
                continue;
            } else {
                let id = request["id"].clone();
                let method = request["method"].as_str().unwrap_or("");
                let result = match method {
                    "initialize" if !initialized => {
                        initialized = true;
                        Ok(
                            json!({"protocolVersion":"2025-11-25","capabilities":{"tools":{"listChanged":false}},"serverInfo":{"name":"joydeck","version":env!("CARGO_PKG_VERSION")},"instructions":"Local headless session. New output paths only, authorized root fixed at launch. Mutations require document target/revision. Poll explicit export jobs; this server does not attach to desktop windows."}),
                        )
                    }
                    "ping" => Ok(json!({})),
                    _ if !ready => Err((
                        -32002,
                        "Initialize and send notifications/initialized first",
                    )),
                    "tools/list" => Ok(
                        json!({"tools":[{"name":"joydeck.control","description":"Control a headless JoyDeck document, inspect/edit/save, export jobs and deterministic slideshow frames. Paths must be absolute and within the startup root. Protocol joydeck-control-1.","inputSchema":{"type":"object","properties":{"protocolVersion":{"const":VERSION,"type":"string"},"requestId":{"type":"string","minLength":1,"maxLength":128},"method":{"type":"string"},"target":{"type":["integer","null"]},"expectedRevision":{"type":["integer","null"]},"params":{"type":"object"}},"required":["protocolVersion","requestId","method"],"additionalProperties":false},"annotations":{"readOnlyHint":false,"destructiveHint":false,"idempotentHint":true,"openWorldHint":false}}]}),
                    ),
                    "tools/call" => {
                        if request["params"]["name"] != "joydeck.control" {
                            Err((-32602, "Unknown tool"))
                        } else {
                            match serde_json::from_value::<Request>(
                                request["params"]["arguments"].clone(),
                            ) {
                                Ok(r) => {
                                    let response = dispatcher.dispatch(r);
                                    Ok(
                                        json!({"content":[{"type":"text","text":response.to_string()}],"structuredContent":response,"isError":response.get("error").is_some()}),
                                    )
                                }
                                Err(_) => Err((-32602, "Invalid tool arguments")),
                            }
                        }
                    }
                    _ => Err((-32601, "Method not found")),
                };
                match result {
                    Ok(result) => json!({"jsonrpc":"2.0","id":id,"result":result}),
                    Err((code, message)) => rpc_error(id, code, message),
                }
            }
        };
        writeln!(output, "{response}").map_err(|e| e.to_string())?;
        output.flush().map_err(|e| e.to_string())?;
    }
    Ok(())
}
fn rpc_error(id: Value, code: i32, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}
