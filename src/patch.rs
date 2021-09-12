use std::error::Error;
use std::io::{self, Write};
use std::{fmt, mem};

use crate::consts::{
    DELTA_MAGIC, RS_OP_COPY_N1_N1, RS_OP_COPY_N8_N8, RS_OP_END, RS_OP_LITERAL_1, RS_OP_LITERAL_64,
    RS_OP_LITERAL_N1, RS_OP_LITERAL_N8,
};

/// Indicates that a delta could not be applied because it was invalid.
#[derive(Debug)]
pub enum ApplyError {
    /// The delta started with the wrong magic, perhaps because it is not really an rsync delta.
    WrongMagic {
        /// The magic number encountered.
        magic: u32,
    },
    /// The delta ended unexpectedly, perhaps because it was truncated.
    UnexpectedEof {
        /// The item being read.
        reading: &'static str,
        /// The expected length of that item.
        expected: usize,
        /// The remaining length of the input.
        available: usize,
    },
    /// The resulting data would have exceeded the output limit given to [apply_limited()].
    OutputLimit {
        /// The item being written.
        what: &'static str,
        /// The length of that item.
        wanted: usize,
        /// The remaining output limit.
        available: usize,
    },
    /// The delta contained an out-of-bounds reference to the base data: that is, `offset + len > data_len`.
    CopyOutOfBounds {
        /// The copy offset.
        offset: u64,
        /// The copy length.
        len: u64,
        /// The length of the base data.
        data_len: usize,
    },
    /// The delta contained a zero-length copy command.
    CopyZero,
    /// The delta contained an unrecognized command.
    UnknownCommand {
        /// The command byte encountered.
        command: u8,
    },
    /// The delta contained data after its end command.
    TrailingData {
        /// The length of the trailing data.
        length: usize,
    },
    /// There was an IO error while writing the output
    Io(io::Error),
}

impl fmt::Display for ApplyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApplyError::WrongMagic { magic } => write!(f, "incorrect magic: 0x{:08x}", magic),
            ApplyError::UnexpectedEof {
                reading,
                expected,
                available,
            } => write!(
                f,
                "unexpected end of input when reading {} (expected={}, available={})",
                reading, expected, available
            ),
            ApplyError::OutputLimit {
                what,
                wanted,
                available,
            } => write!(
                f,
                "exceeded output size limit when writing {} (wanted={}, available={})",
                what, wanted, available
            ),
            ApplyError::CopyOutOfBounds {
                offset,
                len,
                data_len,
            } => write!(
                f,
                "requested copy is out of bounds (offset={}, len={}, data_len={})",
                offset, len, data_len
            ),
            ApplyError::CopyZero => f.write_str("copy length is empty"),
            ApplyError::UnknownCommand { command } => {
                write!(f, "unexpected command byte: 0x{:02x}", command)
            }
            ApplyError::TrailingData { length } => {
                write!(f, "unexpected data after end command (len={})", length)
            }
            Self::Io(source) => write!(f, "io error while writing the output (source={})", source),
        }
    }
}

impl Error for ApplyError {}

impl From<io::Error> for ApplyError {
    fn from(source: io::Error) -> Self {
        Self::Io(source)
    }
}

/// Apply `delta` to the base data `base`, writing the result to `out`.
/// Errors if more than `limit` bytes would be written to `out`.
pub fn apply_limited(
    base: &[u8],
    mut delta: &[u8],
    out: &mut impl Write,
    mut limit: usize,
) -> Result<(), ApplyError> {
    macro_rules! read_n {
        ($n:expr, $what:expr) => {{
            let n = $n;
            if delta.len() < n {
                return Err(ApplyError::UnexpectedEof {
                    reading: $what,
                    expected: n,
                    available: delta.len(),
                });
            }
            let (prefix, rest) = delta.split_at(n);
            delta = rest;
            prefix
        }};
    }
    macro_rules! read_int {
        ($ty:ty, $what:expr) => {{
            let mut b = [0; mem::size_of::<$ty>()];
            b.copy_from_slice(read_n!(mem::size_of::<$ty>(), $what));
            <$ty>::from_be_bytes(b)
        }};
    }
    macro_rules! read_varint {
        ($len:expr, $what:expr) => {{
            let len = $len;
            let mut b = [0; 8];
            b[8 - len..8].copy_from_slice(read_n!(len, $what));
            u64::from_be_bytes(b)
        }};
    }
    macro_rules! safe_cast {
        ($val:expr, $ty:ty, $err:expr) => {{
            let val = $val;
            if val as u64 > <$ty>::max_value() as u64 {
                return Err($err);
            }
            val as $ty
        }};
    }
    macro_rules! safe_extend {
        ($slice:expr, $what:expr) => {{
            let slice: &[u8] = $slice;
            if slice.len() > limit {
                return Err(ApplyError::OutputLimit {
                    what: $what,
                    wanted: slice.len(),
                    available: limit,
                });
            }
            limit -= slice.len();
            out.write_all(slice)?;
        }};
    }
    let magic = read_int!(u32, "magic");
    if magic != DELTA_MAGIC {
        return Err(ApplyError::WrongMagic { magic });
    }
    loop {
        let cmd = read_int!(u8, "cmd");
        match cmd {
            RS_OP_END => {
                break;
            }
            RS_OP_LITERAL_1..=RS_OP_LITERAL_N8 => {
                let n = if cmd <= RS_OP_LITERAL_64 {
                    // <=64, length is encoded in `cmd`
                    (1 + cmd - RS_OP_LITERAL_1) as usize
                } else {
                    safe_cast!(
                        read_varint!(1 << (cmd - RS_OP_LITERAL_N1) as usize, "literal length"),
                        usize,
                        ApplyError::OutputLimit {
                            what: "literal",
                            wanted: usize::max_value(),
                            available: limit,
                        }
                    )
                };
                safe_extend!(read_n!(n, "literal"), "literal");
            }
            RS_OP_COPY_N1_N1..=RS_OP_COPY_N8_N8 => {
                let mode = cmd - RS_OP_COPY_N1_N1;
                let offset_len = 1 << (mode / 4) as usize;
                let len_len = 1 << (mode % 4) as usize;
                let offset = read_varint!(offset_len, "copy offset");
                let len = read_varint!(len_len, "copy length");
                let make_oob_error = || ApplyError::CopyOutOfBounds {
                    offset,
                    len,
                    data_len: base.len(),
                };
                let offset = safe_cast!(offset, usize, make_oob_error());
                let len = safe_cast!(len, usize, make_oob_error());
                if len == 0 {
                    return Err(ApplyError::CopyZero);
                }
                let end = offset.checked_add(len).ok_or_else(make_oob_error)?;
                let subslice = base.get(offset..end).ok_or_else(make_oob_error)?;
                safe_extend!(subslice, "copy");
            }
            _ => return Err(ApplyError::UnknownCommand { command: cmd }),
        }
    }
    if delta.is_empty() {
        Ok(())
    } else {
        // extra content after EOF
        Err(ApplyError::TrailingData {
            length: delta.len(),
        })
    }
}

/// Apply `delta` to the base data `base`, appending the result to `out`.
///
/// # Security
/// This function should not be used with untrusted input, as a delta may create an arbitrarily
/// large output which can exhaust available memory. Use [apply_limited()] instead to set an upper
/// bound on the size of `out`.
pub fn apply(base: &[u8], delta: &[u8], out: &mut impl Write) -> Result<(), ApplyError> {
    apply_limited(base, delta, out, usize::max_value())
}
