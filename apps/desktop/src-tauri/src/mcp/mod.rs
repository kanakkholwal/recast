//! `recast mcp`: an MCP stdio server proxying to the running app.
//! Stateless by design, so a client restart loses nothing and the control socket stays the single writer.

mod protocol;
mod tools;

use std::io::{BufRead, Write};

use serde_json::Value;

/// Forwards each tool call to the running app over the control socket.
struct ControlHost {
    auto_launch: bool,
    timeout_ms: u64,
}

impl protocol::ToolHost for ControlHost {
    fn call(&self, verb: &str, params: Value) -> Result<Value, String> {
        crate::control::send(verb, params, self.auto_launch, self.timeout_ms)
    }
}

/// Serve MCP on stdin/stdout until the client closes the stream.
/// # Errors Only a stdout write failure, which means the client is gone.
pub fn serve(auto_launch: bool, timeout_ms: u64) -> Result<(), String> {
    let host = ControlHost {
        auto_launch,
        timeout_ms,
    };
    serve_on(
        std::io::stdin().lock(),
        std::io::stdout().lock(),
        &host,
        &mut protocol::Server::new(),
    )
}

/// The loop, over any reader and writer, so it can be driven by a test.
fn serve_on(
    input: impl BufRead,
    mut output: impl Write,
    host: &impl protocol::ToolHost,
    server: &mut protocol::Server,
) -> Result<(), String> {
    for line in input.lines() {
        // A read error is a client that vanished mid-line; nothing to report.
        let Ok(line) = line else { break };
        let Some(response) = server.handle_line(&line, host) else {
            continue;
        };
        writeln!(output, "{response}").map_err(|e| format!("mcp stdout: {e}"))?;
        output.flush().map_err(|e| format!("mcp stdout: {e}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct StubHost;

    impl protocol::ToolHost for StubHost {
        fn call(&self, _verb: &str, _params: Value) -> Result<Value, String> {
            Ok(json!({ "ok": true }))
        }
    }

    fn drive(input: &str) -> Vec<Value> {
        let mut output = Vec::new();
        serve_on(
            input.as_bytes(),
            &mut output,
            &StubHost,
            &mut protocol::Server::new(),
        )
        .expect("serve");
        String::from_utf8(output)
            .expect("utf8")
            .lines()
            .map(|line| serde_json::from_str(line).expect("valid JSON"))
            .collect()
    }

    #[test]
    fn answers_one_request_per_line() {
        let input = format!(
            "{}\n{}\n",
            json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" }),
            json!({ "jsonrpc": "2.0", "id": 2, "method": "ping" })
        );

        assert_eq!(drive(&input).len(), 2);
    }

    #[test]
    fn keeps_responses_in_request_order() {
        let input = format!(
            "{}\n{}\n",
            json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" }),
            json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })
        );

        let ids: Vec<Value> = drive(&input)
            .into_iter()
            .map(|value| value["id"].clone())
            .collect();

        assert_eq!(ids, vec![json!(1), json!(2)]);
    }

    #[test]
    fn writes_nothing_for_a_notification() {
        let input = format!(
            "{}\n",
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })
        );

        assert!(drive(&input).is_empty());
    }

    #[test]
    fn a_malformed_line_does_not_stop_the_session() {
        let input = format!(
            "not json\n{}\n",
            json!({ "jsonrpc": "2.0", "id": 2, "method": "ping" })
        );

        assert_eq!(drive(&input).len(), 2);
    }

    #[test]
    fn ends_cleanly_when_the_client_closes_stdin() {
        assert!(drive("").is_empty());
    }
}
