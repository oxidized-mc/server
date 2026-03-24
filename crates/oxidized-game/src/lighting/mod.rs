//! Lighting engine for sky light and block light propagation.
//!
//! Implements batched BFS with parallel section processing per ADR-017.
//!
//! # Architecture
//!
//! - [`queue::LightUpdateQueue`] accumulates pending light changes during a tick.
//! - [`engine::LightEngine`] processes the queue, propagating light via BFS.
//!
//! # Status
//!
//! This module is scaffolding only. The BFS propagation algorithm will be
//! implemented in Phase P13. See ADR-017 for the full design.

pub mod engine;
pub mod queue;
