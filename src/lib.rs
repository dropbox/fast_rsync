#![allow(clippy::unreadable_literal)]

mod consts;
mod crc;
mod diff;
mod hasher;
mod md4;
mod patch;
mod signature;

#[cfg(test)]
mod tests;

pub use diff::{diff, DiffError};
pub use patch::{apply, apply_limited, ApplyError};
pub use signature::{IndexedSignature, Signature, SignatureOptions, SignatureParseError};
