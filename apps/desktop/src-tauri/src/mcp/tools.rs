//! The MCP tool surface and how each tool maps onto a control verb.
//! Every mutating tool is branch-scoped and `branch.apply` is absent, so applying stays a human action.

use serde_json::{json, Value};

/// One MCP tool and the control verb behind it.
pub struct Tool {
    pub name: &'static str,
    pub description: &'static str,
    /// Control-socket method this proxies to.
    pub verb: &'static str,
    /// JSON Schema for the tool's arguments.
    pub schema: fn() -> Value,
}

impl Tool {
    pub fn descriptor(&self) -> Value {
        json!({
            "name": self.name,
            "description": self.description,
            "inputSchema": (self.schema)(),
        })
    }
}

pub const TOOLS: &[Tool] = &[
    Tool {
        name: "recast_status",
        description: "Whether Recast is running, and its recording state.",
        verb: "status",
        schema: no_args,
    },
    Tool {
        name: "recast_project_list",
        description: concat!(
            "Recordings in the user's library, newest first, with the absolute path of each. ",
            "Start here: every other tool needs a project path, and this is the only way to learn one. ",
            "`needsMigration` marks a project the user must update in the editor before it can be edited."
        ),
        verb: "project.list",
        schema: no_args,
    },
    Tool {
        name: "recast_project_show",
        description: "Read a project's saved edits (its full render state).",
        verb: "editor.show",
        schema: project_only,
    },
    Tool {
        name: "recast_project_timeline",
        description:
            "Derived timeline for a project: trim window, cuts, kept segments with speeds.",
        verb: "editor.timeline",
        schema: project_only,
    },
    Tool {
        name: "recast_branch_list",
        description: "Open branches of proposed edits for a project.",
        verb: "branch.list",
        schema: project_only,
    },
    Tool {
        name: "recast_branch_create",
        description: concat!(
            "Fork a branch from the project's current state. Edits are proposed here, not applied. ",
            "Reusing the id of a branch that already holds edits is refused rather than overwriting it."
        ),
        verb: "branch.create",
        schema: branch_create_schema,
    },
    Tool {
        name: "recast_branch_append",
        description: concat!(
            "Record edit operations onto a branch as one atomic entry. ",
            "Re-sending an idemKey already on the branch is a no-op. ",
            "Ops are replayed and validated immediately, so a bad edit is rejected here, ",
            "with the offending field, and the branch is left unchanged."
        ),
        verb: "branch.append",
        schema: branch_append_schema,
    },
    Tool {
        name: "recast_branch_diff",
        description: "Field-level changes a branch would make, as dotted paths with before/after.",
        verb: "branch.diff",
        schema: branch_only_schema,
    },
    Tool {
        name: "recast_branch_show",
        description: "The full render state a branch would produce, without applying it.",
        verb: "branch.materialize",
        schema: branch_only_schema,
    },
    Tool {
        name: "recast_branch_truncate",
        description: "Drop every entry after a sequence number, undoing the tail of a branch.",
        verb: "branch.truncate",
        schema: branch_truncate_schema,
    },
    Tool {
        name: "recast_branch_discard",
        description: "Delete a branch and its proposed edits.",
        verb: "branch.discard",
        schema: branch_only_schema,
    },
];

pub fn find(name: &str) -> Option<&'static Tool> {
    TOOLS.iter().find(|tool| tool.name == name)
}

fn object(properties: Value, required: &[&str]) -> Value {
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    })
}

fn project_property() -> Value {
    json!({ "type": "string", "description": "Absolute path to the .recast project." })
}

fn branch_property() -> Value {
    json!({ "type": "string", "description": "Branch id, e.g. `agent-1`." })
}

fn no_args() -> Value {
    object(json!({}), &[])
}

fn project_only() -> Value {
    object(json!({ "path": project_property() }), &["path"])
}

fn branch_only_schema() -> Value {
    object(
        json!({ "path": project_property(), "branch": branch_property() }),
        &["path", "branch"],
    )
}

fn branch_create_schema() -> Value {
    object(
        json!({
            "path": project_property(),
            "branch": branch_property(),
            "author": {
                "type": "string",
                "description": "Who is proposing, e.g. `agent:claude`.",
            },
            "label": {
                "type": "string",
                "description": "Short human-facing summary shown in the review panel.",
            },
        }),
        &["path", "branch", "author"],
    )
}

fn branch_append_schema() -> Value {
    object(
        json!({
            "path": project_property(),
            "branch": branch_property(),
            "idemKey": {
                "type": "string",
                "description": "Unique per entry. Retrying with the same key never double-applies.",
            },
            "ops": {
                "type": "array",
                "description": "Edit operations, each tagged by `op`.",
                "items": { "type": "object" },
            },
            "expectSeq": {
                "type": "integer",
                "description": "Reject unless the branch is at this sequence number.",
            },
        }),
        &["path", "branch", "idemKey", "ops"],
    )
}

fn branch_truncate_schema() -> Value {
    object(
        json!({
            "path": project_property(),
            "branch": branch_property(),
            "seq": {
                "type": "integer",
                "description": "Keep entries up to and including this sequence number.",
            },
        }),
        &["path", "branch", "seq"],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_name_is_unique() {
        let mut names: Vec<&str> = TOOLS.iter().map(|tool| tool.name).collect();
        let total = names.len();
        names.sort_unstable();
        names.dedup();

        assert_eq!(names.len(), total);
    }

    #[test]
    fn every_tool_is_namespaced() {
        for tool in TOOLS {
            assert!(tool.name.starts_with("recast_"), "{} is not", tool.name);
        }
    }

    #[test]
    fn every_tool_describes_itself() {
        for tool in TOOLS {
            assert!(
                !tool.description.is_empty(),
                "{} has no description",
                tool.name
            );
        }
    }

    #[test]
    fn every_schema_is_a_closed_object() {
        for tool in TOOLS {
            let schema = (tool.schema)();
            assert_eq!(
                schema["additionalProperties"],
                json!(false),
                "{}",
                tool.name
            );
        }
    }

    #[test]
    fn every_required_argument_is_declared_as_a_property() {
        for tool in TOOLS {
            let schema = (tool.schema)();
            for required in schema["required"].as_array().expect("required") {
                let key = required.as_str().expect("string");
                assert!(
                    schema["properties"].get(key).is_some(),
                    "{}: required '{key}' is not a property",
                    tool.name
                );
            }
        }
    }

    /// The whole point of the adapter: an agent proposes, a human applies.
    #[test]
    fn no_tool_writes_the_project_directly() {
        let writes_project = |verb: &str| {
            verb == "branch.apply" || verb.starts_with("editor.") && !is_read_verb(verb)
        };

        for tool in TOOLS {
            assert!(
                !writes_project(tool.verb),
                "{} exposes {}",
                tool.name,
                tool.verb
            );
        }
    }

    #[test]
    fn no_tool_starts_a_recording_or_an_export() {
        for tool in TOOLS {
            assert!(
                !tool.verb.starts_with("rec.") && !tool.verb.starts_with("export."),
                "{} exposes {}",
                tool.name,
                tool.verb
            );
        }
    }

    /// Without an argument-free way in, an agent can only work on a path a human
    /// pasted, and the whole surface is unreachable on its own.
    #[test]
    fn a_project_path_is_discoverable_without_already_having_one() {
        let discovery = TOOLS
            .iter()
            .find(|tool| tool.verb == "project.list")
            .expect("a discovery tool");

        assert_eq!((discovery.schema)()["required"], json!([]));
    }

    #[test]
    fn find_resolves_a_known_tool() {
        assert_eq!(
            find("recast_branch_diff").map(|tool| tool.verb),
            Some("branch.diff")
        );
    }

    #[test]
    fn find_rejects_an_unknown_tool() {
        assert!(find("recast_nope").is_none());
    }

    #[test]
    fn a_descriptor_carries_the_schema() {
        let descriptor = find("recast_branch_diff").expect("tool").descriptor();

        assert_eq!(
            descriptor["inputSchema"]["required"],
            json!(["path", "branch"])
        );
    }

    fn is_read_verb(verb: &str) -> bool {
        matches!(
            verb,
            "editor.show" | "editor.timeline" | "editor.session" | "editor.annotations"
        ) || verb.ends_with(".list")
    }
}
