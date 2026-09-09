//! The v3 project: one markup document (`project.rcx`) with `src` references to media and tracks, inside a `Name.recast/` directory.
//! The document is the disk file, the agent's file and the git file; the engine consumes it through one typed mapping to `Scene`.

#![forbid(unsafe_code)]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

pub mod address;
pub mod diff;
pub mod document;
pub mod hash;
pub mod ids;
pub mod ops;
pub mod parse;
pub mod scene;
pub mod schema;
pub mod serialize;
pub mod validate;
pub mod value;
pub mod vars;

#[cfg(feature = "native")]
pub mod layout;
#[cfg(feature = "native")]
pub mod migrate;
#[cfg(feature = "native")]
pub mod package;
#[cfg(feature = "native")]
pub mod store;

pub use address::{Address, AddressError, Location};
pub use diff::diff;
pub use document::{Document, Node};
pub use hash::DocHash;
pub use ids::{Id, IdGen};
pub use ops::{apply_all, Op, OpError};
pub use parse::{parse, ParseError, Position};
pub use serialize::serialize;
pub use validate::{validate, Issue, Level, Report};
pub use vars::{Var, VarType, Vars};

/// The document version this crate reads and writes.
pub const FORMAT_VERSION: u32 = 3;
