//! No-op implementations when profiling is disabled.
//!
//! All functions in this module are `#[inline(always)]` empty functions
//! that will be completely eliminated by the compiler.

#![allow(dead_code)]

use std::time::Duration;

/// No-op initialization.
#[inline(always)]
pub fn init() {}

/// No-op shutdown.
#[inline(always)]
pub fn shutdown() {}

/// No-op event logging.
#[inline(always)]
pub fn log_event<T>(_: T) {}

/// No-op event logging with metadata.
#[inline(always)]
pub fn log_event_with_meta<T, M>(_: T, _: M) {}

/// No-op timed scope logging.
#[inline(always)]
pub fn log_timed(_: &str, _: Duration) {}
