use joydeck::automation::{Dispatcher, LIMIT, Request, VERSION};
use joydeck_shell as joydeck;
use serde_json::{Value, json};
use std::{
    io::{self, BufRead, Read, Write},
    path::PathBuf,
};
fn request(d: &mut Dispatcher, id: String, method: &str, params: Value) -> Value {
    d.dispatch(Request {
        protocol_version: VERSION.into(),
        request_id: id,
        method: method.into(),
        target: Some(d.session.id),
        params,
    })
}
fn run() -> Result<(), (u8, String)> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut remote = None;
    if args.first().is_some_and(|s| s == "--connect") {
        if args.len() < 2 {
            return Err((2, "缺少凭据路径".into()));
        }
        args.remove(0);
        remote = Some(PathBuf::from(args.remove(0)));
    }
    let mut reads = vec![];
    let mut writes = vec![];
    while args
        .first()
        .is_some_and(|s| s == "--read-root" || s == "--write-root")
    {
        if args.len() < 2 {
            return Err((2, "root参数缺少目录".into()));
        }
        let flag = args.remove(0);
        let root = PathBuf::from(args.remove(0));
        if flag == "--read-root" {
            reads.push(root)
        } else {
            writes.push(root)
        }
    }
    let mut d = Dispatcher::new(reads, writes).map_err(|e| (2, e))?;
    if let Some(path) = remote {
        d.connect(path).map_err(|e| (2, e))?;
    }
    let invocation = joydeck::automation::operation_id().map_err(|e| (2, e))?;
    let auto_id = |suffix: &str| format!("{invocation}-{suffix}");
    let command = args.first().map(String::as_str).unwrap_or("commands");
    if command == "serve" {
        let path = args.get(1).ok_or((2, "serve需要新建凭据文件路径".into()))?;
        let service =
            joydeck::control::Service::start(d, &PathBuf::from(path), false).map_err(|e| (2, e))?;
        eprintln!("control listening on {}", service.address);
        let mut line = String::new();
        io::stdin()
            .read_line(&mut line)
            .map_err(|e| (5, e.to_string()))?;
        return Ok(());
    }
    if command == "mcp" {
        return mcp(&mut d).map_err(|e| (5, e));
    }
    let result = match command {
        "diagnostics" => joydeck::diagnostics::report_for(env!("CARGO_PKG_VERSION"), false),
        "commands" | "describe" => joydeck::automation::describe(),
        "inspect" | "render" | "convert" | "compare" => {
            let input = args.get(1).ok_or((2, "缺少输入绝对路径".into()))?;
            let opened = request(
                &mut d,
                auto_id("cli-open"),
                "document.open",
                json!({"path":input}),
            );
            if opened.get("error").is_some() {
                return Err((3, opened.to_string()));
            }
            if command == "compare" {
                let other = args
                    .get(2)
                    .ok_or((2, "compare需要参考文稿绝对路径".into()))?;
                request(
                    &mut d,
                    auto_id("cli-compare"),
                    "document.compare",
                    json!({"path":other}),
                )
            } else if command == "inspect" {
                request(
                    &mut d,
                    auto_id("cli-inspect"),
                    "document.inspect",
                    json!({}),
                )
            } else {
                let output = args.get(2).ok_or((2, "缺少输出绝对路径".into()))?;
                let format = args
                    .get(3)
                    .map(String::as_str)
                    .unwrap_or(if command == "render" { "png" } else { "pdf" });
                request(
                    &mut d,
                    auto_id("cli-export"),
                    "document.export",
                    json!({"path":output,"format":format,"range":if command=="render"{"1"}else{""},"acceptLosses":args.iter().any(|a|a=="--accept-losses")}),
                )
            }
        }
        "run" => {
            let text = args.get(1).ok_or((2, "run需要JSON请求数组".into()))?;
            if text.len() > LIMIT {
                return Err((2, "输入超限".into()));
            }
            let commands: Vec<Value> =
                serde_json::from_str(text).map_err(|e| (2, e.to_string()))?;
            if commands.len() > 128 {
                return Err((2, "最多128个请求".into()));
            }
            let mut out = vec![];
            for (i, c) in commands.into_iter().enumerate() {
                let method = c["method"].as_str().ok_or((2, "缺少method".into()))?;
                let result = request(
                    &mut d,
                    c["operationId"]
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| auto_id(&format!("cli-{i}"))),
                    method,
                    c["params"].clone(),
                );
                let error = result.get("error").is_some();
                out.push(result);
                if error {
                    break;
                }
            }
            json!(out)
        }
        _ => {
            return Err((
                2,
                "命令: commands/describe/inspect/run/render/convert/compare/mcp；roots需显式配置"
                    .into(),
            ));
        }
    };
    let failure = result.get("error").is_some()
        || result
            .as_array()
            .is_some_and(|a| a.iter().any(|v| v.get("error").is_some()));
    println!("{result}");
    if failure {
        Err((4, "operation failed".into()))
    } else {
        Ok(())
    }
}
fn mcp(d: &mut Dispatcher) -> Result<(), String> {
    let mut initialized = false;
    let mut ready = false;
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let mut line = vec![];
    loop {
        line.clear();
        let n = reader
            .by_ref()
            .take((LIMIT + 1) as u64)
            .read_until(b'\n', &mut line)
            .map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        if line.len() > LIMIT {
            return Err("MCP消息超过1MiB".into());
        }
        let v: Value = match serde_json::from_slice(&line) {
            Ok(v) => v,
            Err(_) => {
                println!(
                    "{}",
                    json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Parse error"}})
                );
                continue;
            }
        };
        let id = v.get("id").cloned();
        let method = v["method"].as_str().unwrap_or("");
        if id.is_none() {
            if method == "notifications/initialized" && initialized {
                ready = true;
            }
            continue;
        }
        let Some(id) = id else {
            continue;
        };
        let result: Result<Value, (i32, &str)> = if v["jsonrpc"] != "2.0" {
            Err((-32600, "Invalid Request"))
        } else {
            match method {
                "initialize" if !initialized => {
                    if v["params"]["protocolVersion"] != "2025-11-25" {
                        Err((-32602, "Only MCP 2025-11-25 is supported"))
                    } else {
                        initialized = true;
                        Ok(
                            json!({"protocolVersion":"2025-11-25","capabilities":{"tools":{},"resources":{}},"serverInfo":{"name":"joydeck-headless","version":env!("CARGO_PKG_VERSION")}}),
                        )
                    }
                }
                "ping" => Ok(json!({})),
                _ if !ready => Err((-32002, "Server not initialized")),
                "tools/list" => Ok(
                    json!({"tools":[{"name":"joydeck_dispatch","description":"Serial JoyDeck headless operation. Mutations require stable operationId. Explicit document target and configured read/write roots are enforced.","inputSchema":{"type":"object","properties":{"operationId":{"type":"string"},"method":{"type":"string"},"target":{"type":"integer"},"params":{"type":"object"}},"required":["operationId","method","params"],"additionalProperties":false}}]}),
                ),
                "tools/call" => {
                    if v["params"]["name"] != "joydeck_dispatch" {
                        Err((-32602, "Unknown tool"))
                    } else {
                        let a = &v["params"]["arguments"];
                        if let (Some(op), Some(method)) =
                            (a["operationId"].as_str(), a["method"].as_str())
                        {
                            let response = d.dispatch(Request {
                                protocol_version: VERSION.into(),
                                request_id: op.into(),
                                method: method.into(),
                                target: a["target"].as_u64(),
                                params: a["params"].clone(),
                            });
                            Ok(
                                json!({"content":[{"type":"text","text":response.to_string()}],"isError":response.get("error").is_some()}),
                            )
                        } else {
                            Err((-32602, "Missing operationId or method"))
                        }
                    }
                }
                "resources/list" => Ok(
                    json!({"resources":[{"uri":"joydeck://current/summary","name":"Current document summary","mimeType":"application/json"}]}),
                ),
                "resources/read" => {
                    if v["params"]["uri"] == "joydeck://current/summary" {
                        let response = d.dispatch(Request {
                            protocol_version: VERSION.into(),
                            request_id: joydeck::automation::operation_id()?,
                            method: "document.inspect".into(),
                            target: Some(d.session.id),
                            params: json!({}),
                        });
                        if let Some(result) = response.get("result") {
                            Ok(
                                json!({"contents":[{"uri":"joydeck://current/summary","mimeType":"application/json","text":json!({"documentId":result["documentId"],"revision":result["revision"],"pages":result["deck"]["slides"].as_array().map(Vec::len),"dirty":result["dirty"]}).to_string()}]}),
                            )
                        } else {
                            Err((-32603, "Backend inspect failed"))
                        }
                    } else {
                        Err((-32602, "Unknown resource"))
                    }
                }
                _ => Err((-32601, "Method not found")),
            }
        };
        let response = match result {
            Ok(result) => json!({"jsonrpc":"2.0","id":id,"result":result}),
            Err((code, message)) => {
                json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
            }
        };
        println!("{response}");
        io::stdout().flush().map_err(|e| e.to_string())?;
    }
    Ok(())
}
fn main() {
    if let Err((code, message)) = run() {
        eprintln!("{}", json!({"error":message}));
        std::process::exit(code as i32);
    }
}
