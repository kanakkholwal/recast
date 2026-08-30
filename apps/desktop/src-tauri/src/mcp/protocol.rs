//! MCP over stdio: newline-delimited JSON-RPC 2.0.
//!
//! Hand-rolled rather than pulled from `rmcp`, whose current release needs a
//! newer rustc than this crate's `rust-version`. The surface we need is small
//! (initialize, tools/list, tools/call, ping), and keeping it here means the
//! request handling is a pure function that tests without a socket or a process.

use serde_json::{json, Value};

use super::tools;

/// Protocol revision we implement. A client asking for a different one is not
/// refused: MCP expects the server to answer with what it actually speaks.
pub const PROTOCOL_VERSION: &str = "2025-06-18";

pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const PARSE_ERROR: i64 = -32700;
/// Sent before the client has completed the initialize handshake.
pub const NOT_INITIALIZED: i64 = -32002;

/// Runs the verb behind a tool. The stdio server proxies to the control socket;
/// tests substitute a fake.
pub trait ToolHost {
    /// # Errors
    /// The message is surfaced to the model as tool output, not as a transport
    /// failure, so it should read as something the model can act on.
    fn call(&self, verb: &str, params: Value) -> Result<Value, String>;
}

#[derive(Debug, Default)]
pub struct Server {
    initialized: bool,
}

impl Server {
    pub fn new() -> Self {
        Self::default()
    }

    /// Handle one line from the client.
    ///
    /// Returns the line to write back, or `None` for a notification, which
    /// JSON-RPC says must not be answered.
    pub fn handle_line(&mut self, line: &str, host: &impl ToolHost) -> Option<String> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }
        let request: Value = match serde_json::from_str(trimmed) {
            Ok(value) => value,
            Err(err) => {
                return Some(render(error_response(
                    Value::Null,
                    PARSE_ERROR,
                    format!("invalid JSON: {err}"),
                )))
            }
        };
        self.handle(&request, host).map(render)
    }

    fn handle(&mut self, request: &Value, host: &impl ToolHost) -> Option<Value> {
        let method = request.get("method").and_then(Value::as_str).unwrap_or("");
        let id = request.get("id").cloned();
        let params = request.get("params").cloned().unwrap_or(Value::Null);

        // No id means a notification: act on it, answer nothing.
        let Some(id) = id else {
            if method == "notifications/initialized" {
                self.initialized = true;
            }
            return None;
        };

        // `ping` stays open so a client can probe liveness before handshaking.
        if !self.initialized && !matches!(method, "initialize" | "ping") {
            return Some(error_response(
                id,
                NOT_INITIALIZED,
                format!("'{method}' before initialize"),
            ));
        }

        Some(match method {
            "initialize" => {
                self.initialized = true;
                success(id, initialize_result())
            }
            "ping" => success(id, json!({})),
            "tools/list" => success(id, json!({ "tools": tool_descriptors() })),
            "tools/call" => match call_tool(&params, host) {
                Ok(result) => success(id, result),
                Err(err) => error_response(id, err.code, err.message),
            },
            "resources/list" => match list_resources(host) {
                Ok(result) => success(id, result),
                Err(err) => error_response(id, err.code, err.message),
            },
            "resources/read" => match read_resource(&params, host) {
                Ok(result) => success(id, result),
                Err(err) => error_response(id, err.code, err.message),
            },
            other => error_response(id, METHOD_NOT_FOUND, format!("unknown method '{other}'")),
        })
    }
}

struct CallError {
    code: i64,
    message: String,
}

fn call_tool(params: &Value, host: &impl ToolHost) -> Result<Value, CallError> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| CallError {
            code: INVALID_PARAMS,
            message: "tools/call requires a name".into(),
        })?;
    let tool = tools::find(name).ok_or_else(|| CallError {
        code: INVALID_PARAMS,
        message: format!("unknown tool '{name}'"),
    })?;
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    // A failed verb is a tool result the model can read and retry, which is exactly what MCP reserves `isError` for.
    Ok(match host.call(tool.verb, arguments) {
        Ok(value) => tool_result(&value, false),
        Err(message) => tool_result(&Value::String(message), true),
    })
}

fn tool_result(value: &Value, is_error: bool) -> Value {
    let text = match value {
        Value::String(text) => text.clone(),
        other => serde_json::to_string_pretty(other).unwrap_or_else(|_| other.to_string()),
    };
    json!({
        "content": [{ "type": "text", "text": text }],
        "isError": is_error,
    })
}

/// Projects are resources as well as tool arguments, so a client can attach one
/// to the conversation without spending a tool call on it.
///
/// The path is percent-encoded into the URI rather than appended raw: a Windows
/// path carries `:` and `\`, and a recording title routinely carries a space.
const RESOURCE_PREFIX: &str = "recast://project/";

fn resource_uri(path: &str) -> String {
    format!("{RESOURCE_PREFIX}{}", urlencoding::encode(path))
}

fn resource_path(uri: &str) -> Option<String> {
    let encoded = uri.strip_prefix(RESOURCE_PREFIX)?;
    urlencoding::decode(encoded)
        .ok()
        .map(|decoded| decoded.into_owned())
}

/// A failing host is an empty library rather than a protocol error: a client
/// lists resources on connect, and refusing there reads as a broken server.
fn list_resources(host: &impl ToolHost) -> Result<Value, CallError> {
    let entries = host.call("project.list", json!({})).unwrap_or(json!([]));
    let resources: Vec<Value> = entries
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter_map(|entry| {
            let path = entry.get("path")?.as_str()?;
            let name = entry
                .get("filename")
                .and_then(Value::as_str)
                .unwrap_or(path);
            Some(json!({
                "uri": resource_uri(path),
                "name": name,
                "description": "Saved edits for this recording, as a RenderState.",
                "mimeType": "application/json",
            }))
        })
        .collect();
    Ok(json!({ "resources": resources }))
}

fn read_resource(params: &Value, host: &impl ToolHost) -> Result<Value, CallError> {
    let uri = params
        .get("uri")
        .and_then(Value::as_str)
        .ok_or_else(|| CallError {
            code: INVALID_PARAMS,
            message: "resources/read requires a uri".into(),
        })?;
    let path = resource_path(uri).ok_or_else(|| CallError {
        code: INVALID_PARAMS,
        message: format!("'{uri}' is not a {RESOURCE_PREFIX} uri"),
    })?;
    let state = host
        .call("editor.show", json!({ "path": path }))
        .map_err(|message| CallError {
            code: INVALID_PARAMS,
            message,
        })?;
    Ok(json!({
        "contents": [{
            "uri": uri,
            "mimeType": "application/json",
            "text": serde_json::to_string_pretty(&state).unwrap_or_else(|_| state.to_string()),
        }],
    }))
}

fn tool_descriptors() -> Vec<Value> {
    tools::TOOLS.iter().map(tools::Tool::descriptor).collect()
}

fn initialize_result() -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": {
            "tools": { "listChanged": false },
            "resources": { "subscribe": false, "listChanged": false },
        },
        "serverInfo": {
            "name": "recast",
            "version": env!("CARGO_PKG_VERSION"),
        },
        "instructions": concat!(
            "Start with recast_project_list to find a project path; nothing else here ",
            "discovers one. Recast edits are then proposed on a branch, never written ",
            "directly: create a branch, append ops to it, then tell the user to review ",
            "and apply it in the editor. Applying is deliberately not available here."
        ),
    })
}

fn success(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn error_response(id: Value, code: i64, message: impl Into<String>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message.into() },
    })
}

/// One line, no embedded newlines: the stdio framing depends on it.
fn render(value: Value) -> String {
    value.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[derive(Default)]
    struct FakeHost {
        calls: RefCell<Vec<(String, Value)>>,
        reply: Option<Result<Value, String>>,
    }

    impl FakeHost {
        fn returning(value: Value) -> Self {
            Self {
                reply: Some(Ok(value)),
                ..Self::default()
            }
        }

        fn failing(message: &str) -> Self {
            Self {
                reply: Some(Err(message.to_string())),
                ..Self::default()
            }
        }

        fn last_call(&self) -> (String, Value) {
            self.calls.borrow().last().cloned().expect("a call")
        }
    }

    impl ToolHost for FakeHost {
        fn call(&self, verb: &str, params: Value) -> Result<Value, String> {
            self.calls
                .borrow_mut()
                .push((verb.to_string(), params.clone()));
            self.reply.clone().unwrap_or_else(|| Ok(json!({})))
        }
    }

    /// Most tests speak to an already-handshaken server, which is the only
    /// state a real client reaches.
    fn ready_server(host: &FakeHost) -> Server {
        let mut server = Server::new();
        server.handle_line(&request(0, "initialize", json!({})), host);
        server
    }

    fn respond(line: &str, host: &FakeHost) -> Value {
        let raw = ready_server(host)
            .handle_line(line, host)
            .expect("a response");
        serde_json::from_str(&raw).expect("valid JSON")
    }

    fn request(id: u64, method: &str, params: Value) -> String {
        json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string()
    }

    mod framing {
        use super::*;

        #[test]
        fn a_blank_line_is_ignored() {
            assert!(Server::new()
                .handle_line("   ", &FakeHost::default())
                .is_none());
        }

        #[test]
        fn malformed_json_reports_a_parse_error() {
            let response = respond("{ not json", &FakeHost::default());

            assert_eq!(response["error"]["code"], json!(PARSE_ERROR));
        }

        #[test]
        fn a_response_never_contains_a_newline() {
            let raw = Server::new()
                .handle_line(&request(1, "tools/list", json!({})), &FakeHost::default())
                .expect("a response");

            assert!(!raw.contains('\n'));
        }

        #[test]
        fn every_response_carries_the_request_id() {
            assert_eq!(
                respond(&request(7, "ping", json!({})), &FakeHost::default())["id"],
                json!(7)
            );
        }
    }

    mod handshake {
        use super::*;

        #[test]
        fn a_tool_call_before_initialize_is_refused() {
            let raw = Server::new()
                .handle_line(&request(1, "tools/list", json!({})), &FakeHost::default())
                .expect("a response");
            let response: Value = serde_json::from_str(&raw).expect("valid JSON");

            assert_eq!(response["error"]["code"], json!(NOT_INITIALIZED));
        }

        #[test]
        fn ping_is_answered_before_initialize() {
            let raw = Server::new()
                .handle_line(&request(1, "ping", json!({})), &FakeHost::default())
                .expect("a response");
            let response: Value = serde_json::from_str(&raw).expect("valid JSON");

            assert!(response["result"].is_object(), "got: {response}");
        }

        #[test]
        fn the_host_is_never_reached_before_initialize() {
            let host = FakeHost::default();

            Server::new().handle_line(
                &request(1, "tools/call", json!({ "name": "recast_status" })),
                &host,
            );

            assert!(host.calls.borrow().is_empty());
        }
    }

    mod initialize {
        use super::*;

        #[test]
        fn reports_the_protocol_version() {
            let response = respond(&request(1, "initialize", json!({})), &FakeHost::default());

            assert_eq!(
                response["result"]["protocolVersion"],
                json!(PROTOCOL_VERSION)
            );
        }

        #[test]
        fn advertises_the_tools_capability() {
            let response = respond(&request(1, "initialize", json!({})), &FakeHost::default());

            assert!(response["result"]["capabilities"]["tools"].is_object());
        }

        #[test]
        fn advertises_the_resources_capability() {
            let response = respond(&request(1, "initialize", json!({})), &FakeHost::default());

            assert!(response["result"]["capabilities"]["resources"].is_object());
        }

        #[test]
        fn names_the_server() {
            let response = respond(&request(1, "initialize", json!({})), &FakeHost::default());

            assert_eq!(response["result"]["serverInfo"]["name"], json!("recast"));
        }

        #[test]
        fn the_initialized_notification_gets_no_reply() {
            let notification = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });

            assert!(Server::new()
                .handle_line(&notification.to_string(), &FakeHost::default())
                .is_none());
        }

        #[test]
        fn the_initialized_notification_alone_opens_the_session() {
            let mut server = Server::new();
            let notification = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
            server.handle_line(&notification.to_string(), &FakeHost::default());

            let raw = server
                .handle_line(&request(1, "tools/list", json!({})), &FakeHost::default())
                .expect("a response");
            let response: Value = serde_json::from_str(&raw).expect("valid JSON");

            assert!(response["result"].is_object(), "got: {response}");
        }
    }

    mod tools_list {
        use super::*;

        #[test]
        fn lists_every_tool() {
            let response = respond(&request(1, "tools/list", json!({})), &FakeHost::default());

            assert_eq!(
                response["result"]["tools"].as_array().expect("array").len(),
                tools::TOOLS.len()
            );
        }

        #[test]
        fn each_entry_carries_an_input_schema() {
            let response = respond(&request(1, "tools/list", json!({})), &FakeHost::default());

            for tool in response["result"]["tools"].as_array().expect("array") {
                assert!(tool["inputSchema"].is_object(), "{tool}");
            }
        }
    }

    mod tools_call {
        use super::*;

        fn call(name: &str, arguments: Value, host: &FakeHost) -> Value {
            respond(
                &request(
                    1,
                    "tools/call",
                    json!({ "name": name, "arguments": arguments }),
                ),
                host,
            )
        }

        #[test]
        fn proxies_to_the_verb_behind_the_tool() {
            let host = FakeHost::returning(json!([]));

            call("recast_branch_list", json!({ "path": "p.recast" }), &host);

            assert_eq!(host.last_call().0, "branch.list");
        }

        #[test]
        fn forwards_the_arguments_untouched() {
            let host = FakeHost::returning(json!([]));

            call("recast_branch_list", json!({ "path": "p.recast" }), &host);

            assert_eq!(host.last_call().1, json!({ "path": "p.recast" }));
        }

        #[test]
        fn missing_arguments_become_an_empty_object() {
            let host = FakeHost::returning(json!({}));

            respond(
                &request(1, "tools/call", json!({ "name": "recast_status" })),
                &host,
            );

            assert_eq!(host.last_call().1, json!({}));
        }

        #[test]
        fn renders_a_result_as_text_content() {
            let host = FakeHost::returning(json!({ "recording": false }));

            let response = call("recast_status", json!({}), &host);

            assert!(response["result"]["content"][0]["text"]
                .as_str()
                .expect("text")
                .contains("recording"));
        }

        #[test]
        fn a_successful_call_is_not_flagged_as_an_error() {
            let host = FakeHost::returning(json!({}));

            let response = call("recast_status", json!({}), &host);

            assert_eq!(response["result"]["isError"], json!(false));
        }

        #[test]
        fn a_failing_verb_is_a_tool_error_not_a_protocol_error() {
            let host = FakeHost::failing("editor_locked: held by 'ui:me'");

            let response = call("recast_status", json!({}), &host);

            assert_eq!(response["result"]["isError"], json!(true));
        }

        #[test]
        fn a_failing_verb_surfaces_its_message_to_the_model() {
            let host = FakeHost::failing("editor_locked: held by 'ui:me'");

            let response = call("recast_status", json!({}), &host);

            assert_eq!(
                response["result"]["content"][0]["text"],
                json!("editor_locked: held by 'ui:me'")
            );
        }

        #[test]
        fn an_unknown_tool_is_rejected_before_the_host_is_touched() {
            let host = FakeHost::default();

            call("recast_nope", json!({}), &host);

            assert!(host.calls.borrow().is_empty());
        }

        #[test]
        fn an_unknown_tool_reports_invalid_params() {
            let response = call("recast_nope", json!({}), &FakeHost::default());

            assert_eq!(response["error"]["code"], json!(INVALID_PARAMS));
        }

        #[test]
        fn a_call_without_a_name_is_rejected() {
            let response = respond(&request(1, "tools/call", json!({})), &FakeHost::default());

            assert_eq!(response["error"]["code"], json!(INVALID_PARAMS));
        }
    }

    mod resources {
        use super::*;

        const WINDOWS_PATH: &str = r"C:\Users\kanak\Videos\demo take 2.recast";

        fn library() -> FakeHost {
            FakeHost::returning(json!([
                { "path": WINDOWS_PATH, "filename": "demo take 2.recast" }
            ]))
        }

        #[test]
        fn lists_one_resource_per_project() {
            let response = respond(&request(1, "resources/list", json!({})), &library());

            assert_eq!(response["result"]["resources"].as_array().unwrap().len(), 1);
        }

        #[test]
        fn names_a_resource_by_its_filename() {
            let response = respond(&request(1, "resources/list", json!({})), &library());

            assert_eq!(
                response["result"]["resources"][0]["name"],
                json!("demo take 2.recast")
            );
        }

        /// A raw Windows path in a URI would carry a drive colon, backslashes
        /// and, routinely, a space.
        #[test]
        fn encodes_the_path_into_the_uri() {
            let response = respond(&request(1, "resources/list", json!({})), &library());
            let uri = response["result"]["resources"][0]["uri"]
                .as_str()
                .unwrap()
                .to_string();

            assert!(!uri.contains(' '), "{uri}");
        }

        #[test]
        fn a_uri_round_trips_back_to_the_path() {
            assert_eq!(
                resource_path(&resource_uri(WINDOWS_PATH)).as_deref(),
                Some(WINDOWS_PATH)
            );
        }

        /// A client lists resources on connect; erroring there reads as a broken
        /// server rather than an empty library.
        #[test]
        fn an_unreachable_app_lists_nothing_rather_than_failing() {
            let response = respond(
                &request(1, "resources/list", json!({})),
                &FakeHost::failing("recast is not running"),
            );

            assert_eq!(response["result"]["resources"], json!([]));
        }

        #[test]
        fn reads_a_project_through_the_show_verb() {
            let host = FakeHost::returning(json!({ "trimStart": 1.0 }));
            let uri = resource_uri(WINDOWS_PATH);

            respond(&request(1, "resources/read", json!({ "uri": uri })), &host);

            assert_eq!(host.last_call().0, "editor.show");
        }

        #[test]
        fn reads_the_project_the_uri_names() {
            let host = FakeHost::returning(json!({ "trimStart": 1.0 }));
            let uri = resource_uri(WINDOWS_PATH);

            respond(&request(1, "resources/read", json!({ "uri": uri })), &host);

            assert_eq!(host.last_call().1["path"], json!(WINDOWS_PATH));
        }

        #[test]
        fn returns_the_state_as_json_text() {
            let host = FakeHost::returning(json!({ "trimStart": 1.0 }));
            let uri = resource_uri(WINDOWS_PATH);

            let response = respond(&request(1, "resources/read", json!({ "uri": uri })), &host);

            assert!(response["result"]["contents"][0]["text"]
                .as_str()
                .unwrap()
                .contains("trimStart"));
        }

        #[test]
        fn rejects_a_uri_from_another_scheme() {
            let response = respond(
                &request(
                    1,
                    "resources/read",
                    json!({ "uri": "file:///tmp/a.recast" }),
                ),
                &FakeHost::default(),
            );

            assert_eq!(response["error"]["code"], json!(INVALID_PARAMS));
        }

        #[test]
        fn a_read_without_a_uri_is_rejected() {
            let response = respond(
                &request(1, "resources/read", json!({})),
                &FakeHost::default(),
            );

            assert_eq!(response["error"]["code"], json!(INVALID_PARAMS));
        }
    }

    mod unknown_methods {
        use super::*;

        #[test]
        fn report_method_not_found() {
            let response = respond(&request(1, "prompts/list", json!({})), &FakeHost::default());

            assert_eq!(response["error"]["code"], json!(METHOD_NOT_FOUND));
        }

        #[test]
        fn an_unknown_notification_gets_no_reply() {
            let notification = json!({ "jsonrpc": "2.0", "method": "notifications/cancelled" });

            assert!(Server::new()
                .handle_line(&notification.to_string(), &FakeHost::default())
                .is_none());
        }
    }
}
