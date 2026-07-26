use std::collections::HashMap;
use std::io::Write;
use std::sync::Arc;

use anyhow::{Context, Result};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::Mutex;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use super::explore;

const MCP_PROTOCOL_VERSION: &str = "2025-06-18";

pub(crate) async fn run() -> Result<()> {
    let stdin = tokio::io::stdin();
    let mut lines = BufReader::new(stdin).lines();
    let active = Arc::new(Mutex::new(HashMap::<String, CancellationToken>::new()));
    let mut tasks = JoinSet::new();

    loop {
        tokio::select! {
            line = lines.next_line() => {
                let Some(line) = line.context("failed to read MCP stdin")? else {
                    cancel_all(&active).await;
                    break;
                };
                let line = line.trim_start_matches('\u{feff}');
                if line.trim().is_empty() {
                    continue;
                }
                let request = match serde_json::from_str::<Value>(line) {
                    Ok(request) => request,
                    Err(error) => {
                        write_message(&rpc_error(Value::Null, -32700, format!("parse error: {error}")))?;
                        continue;
                    }
                };
                dispatch(request, Arc::clone(&active), &mut tasks).await?;
            }
            joined = tasks.join_next(), if !tasks.is_empty() => {
                if let Some(Err(error)) = joined {
                    eprintln!("Spark MCP request task failed: {error}");
                }
            }
        }
    }

    while let Some(joined) = tasks.join_next().await {
        if let Err(error) = joined {
            eprintln!("Spark MCP request task failed during shutdown: {error}");
        }
    }
    Ok(())
}

async fn dispatch(
    request: Value,
    active: Arc<Mutex<HashMap<String, CancellationToken>>>,
    tasks: &mut JoinSet<()>,
) -> Result<()> {
    let id = request.get("id").cloned();
    let method = request
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let params = request.get("params").cloned().unwrap_or_else(|| json!({}));

    match method {
        "initialize" => {
            if let Some(id) = id {
                write_message(&rpc_result(id, initialize_result(&params)))?;
            }
        }
        "notifications/initialized" => {}
        "ping" => {
            if let Some(id) = id {
                write_message(&rpc_result(id, json!({})))?;
            }
        }
        "tools/list" => {
            if let Some(id) = id {
                write_message(&rpc_result(
                    id,
                    json!({"tools": [explore::tool_definition()]}),
                ))?;
            }
        }
        "tools/call" => {
            let Some(id) = id else {
                return Ok(());
            };
            let key = id_key(&id);
            let cancellation = CancellationToken::new();
            active
                .lock()
                .await
                .insert(key.clone(), cancellation.clone());
            let active_for_task = Arc::clone(&active);
            tasks.spawn(async move {
                let response = handle_tool_call(id, params, cancellation).await;
                if let Err(error) = write_message(&response) {
                    eprintln!("failed to write Spark MCP tool response: {error:#}");
                }
                active_for_task.lock().await.remove(&key);
            });
        }
        "notifications/cancelled" => {
            if let Some(request_id) = params.get("requestId") {
                let key = id_key(request_id);
                if let Some(cancellation) = active.lock().await.get(&key) {
                    cancellation.cancel();
                }
            }
        }
        _ => {
            if let Some(id) = id {
                write_message(&rpc_error(
                    id,
                    -32601,
                    format!("method not found: {method}"),
                ))?;
            }
        }
    }
    Ok(())
}

async fn handle_tool_call(id: Value, params: Value, cancellation: CancellationToken) -> Value {
    let name = params.get("name").and_then(Value::as_str);
    if name != Some("explore_repo") {
        return rpc_error(
            id,
            -32602,
            format!("unknown tool: {}", name.unwrap_or("<missing>")),
        );
    }
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    match explore::run(arguments, cancellation).await {
        Ok(summary) => tool_success(id, summary),
        Err(error) => rpc_result(
            id,
            json!({
                "content": [{
                    "type": "text",
                    "text": format!("Spark exploration failed: {error:#}")
                }],
                "isError": true
            }),
        ),
    }
}

fn tool_success(id: Value, summary: String) -> Value {
    rpc_result(
        id,
        json!({
            "content": [{"type": "text", "text": summary}],
            "isError": false
        }),
    )
}

fn initialize_result(params: &Value) -> Value {
    let requested = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or(MCP_PROTOCOL_VERSION);
    json!({
        "protocolVersion": requested,
        "capabilities": {
            "tools": {"listChanged": false}
        },
        "serverInfo": {
            "name": "codex-spark-agent",
            "version": env!("CARGO_PKG_VERSION")
        },
        "instructions": "Use explore_repo for non-trivial read-only repository reconnaissance. Native Codex remains responsible for decisions, edits, and validation."
    })
}

fn rpc_result(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

fn rpc_error(id: Value, code: i64, message: String) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
}

fn id_key(id: &Value) -> String {
    serde_json::to_string(id).unwrap_or_else(|_| "null".to_string())
}

fn write_message(message: &Value) -> Result<()> {
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();
    serde_json::to_writer(&mut stdout, message)?;
    writeln!(stdout)?;
    stdout.flush()?;
    Ok(())
}

async fn cancel_all(active: &Mutex<HashMap<String, CancellationToken>>) {
    for cancellation in active.lock().await.values() {
        cancellation.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::{initialize_result, rpc_error, rpc_result, tool_success};
    use serde_json::json;

    #[test]
    fn initialize_advertises_tools_capability() {
        let result = initialize_result(&json!({"protocolVersion": "2025-06-18"}));
        assert_eq!(result["protocolVersion"], "2025-06-18");
        assert_eq!(result["capabilities"]["tools"]["listChanged"], false);
        assert_eq!(result["serverInfo"]["name"], "codex-spark-agent");
    }

    #[test]
    fn json_rpc_helpers_preserve_string_ids() {
        assert_eq!(rpc_result(json!("abc"), json!({}))["id"], "abc");
        assert_eq!(
            rpc_error(json!("abc"), -32601, "missing".to_string())["id"],
            "abc"
        );
    }

    #[test]
    fn successful_tool_results_emit_only_compact_text() {
        let response = tool_success(json!(7), "Evidence brief".to_string());
        let result = &response["result"];

        assert_eq!(result["content"][0]["text"], "Evidence brief");
        assert_eq!(result["isError"], false);
        assert!(result.get("structuredContent").is_none());
    }
}
