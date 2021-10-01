//! A faster implementation of [librsync](https://github.com/librsync/librsync) in
//! pure Rust.
//!
//! This crate offers three major APIs:
//!
//! 1. [Signature::calculate()], which takes a block of data and returns a
//!    "signature" of that data which is much smaller than the original data.
//! 2. [diff()], which takes a signature for some block A, and a block of data B, and
//!    returns a delta between block A and block B. If A and B are "similar", then
//!    the delta is usually much smaller than block B.
//! 3. [apply()], which takes a block A and a delta (as constructed by [diff()]), and
//!    (usually) returns the block B.

#![allow(clippy::unreadable_literal)]
#![deny(missing_docs)]

mod consts;
mod crc;
mod diff;
mod hasher;
mod hashmap_variant;
mod md4;
mod patch;
mod signature;

#[cfg(test)]
mod tests;

pub use diff::{diff, DiffError};
pub use patch::{apply, apply_limited, ApplyError};
pub use signature::{IndexedSignature, Signature, SignatureOptions, SignatureParseError};
