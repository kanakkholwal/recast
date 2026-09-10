#![forbid(unsafe_code)]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

pub mod bind;
pub mod component;
pub mod composition;
mod scene;
pub mod vars;

pub mod migrate;
pub mod ops;
pub mod v1;

pub use ops::{apply as apply_op, apply_all as apply_ops, Op, OpError, ScenePath};
pub use scene::{
    AudioGraph, BlendMode, CursorSpec, Effect, Layer, LayerId, LayerSource, OutputSpec, Scene,
    SceneFlags, Timeline, TimelineCut, SCHEMA_VERSION,
};
