//! The agent harness: guards, receipts, checks and perception over a project, shared by the control socket, the CLI and MCP.
//! Everything here is a pure function of project state; nothing holds a session, so any tool result can be reconstructed from the journal.

pub mod axis;
pub mod check;
pub mod guard;
pub mod instructions;
pub mod intents;
pub mod perception;
pub mod receipt;
pub mod schema;
pub mod track;
