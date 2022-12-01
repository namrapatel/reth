#![warn(missing_docs, unreachable_pub)]
#![deny(unused_must_use, rust_2018_idioms)]
#![doc(test(
    no_crate_inject,
    attr(deny(warnings, rust_2018_idioms), allow(dead_code, unused_variables))
))]

//! Reth executor executes transaction in block of data.

pub mod config;
/// Executor
pub mod executor;
/// Wrapper around revm database and types
pub mod revm_wrap;
/// A threaded version of revm_wrap
pub mod threaded_revm_wrap;
pub use config::Config;
