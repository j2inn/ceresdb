// Copyright 2022 CeresDB Project Authors. Licensed under Apache-2.0.

//! Memory profiler for running application based on jemalloc features.
//! Supported on Unix only.

#[cfg(unix)]
mod profiler;

#[cfg(unix)]
pub use profiler::*;
