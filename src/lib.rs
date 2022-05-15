#![feature(arbitrary_self_types)]
#![feature(pin_deref_mut)]
#![feature(ready_macro)]
#![doc = include_str!("../README.md")]

pub mod future;
pub mod combinator;
pub mod executor;
pub(crate) mod helpers;

pub use future::{AbortSafeFuture, AbortSafeFutureExt};
pub use combinator::{ready, pending};