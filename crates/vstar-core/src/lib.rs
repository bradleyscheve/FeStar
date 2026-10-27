//! Core data model and data sources for VStar-RS.
//!
//! This crate has no user-interface code, so it can be tested on its own and
//! reused by the desktop app, command-line tools and future AI analysis.
//!
//! Parts of this crate are ported from AAVSO VStar
//! (<https://github.com/AAVSO/VStar>), which is licensed under the AGPL v3.

pub mod ai;
pub mod analysis;
pub mod bands;
pub mod error;
pub mod http;
pub mod model;
pub mod secrets;
pub mod sources;
pub mod time;
pub mod vartypes;

pub use error::{Error, Result};
pub use model::{Dataset, Observation, StarInfo, StarQuery, Validation};
