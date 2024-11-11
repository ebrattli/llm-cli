#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::cargo)]
#![warn(clippy::style)]
#![warn(clippy::complexity)]
#![warn(clippy::perf)]
#![warn(clippy::correctness)]
#![warn(clippy::suspicious)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::multiple_crate_versions)]
#![allow(clippy::return_self_not_must_use)]
#![allow(clippy::lines_filter_map_ok)]
pub mod cli;
pub mod core;
pub mod eventsource;
pub mod providers;
pub mod tools;

pub use cli::run;
