use std::mem;

use crate::consts::{
    DELTA_MAGIC, RS_OP_COPY_N1_N1, RS_OP_COPY_N8_N8, RS_OP_END, RS_OP_LITERAL_1, RS_OP_LITERAL_64,
    RS_OP_LITERAL_N1, RS_OP_LITERAL_N8,
};

/// Indicates that a delta could not be applied, either because it was invalid
/// or because a hash collision of some kind was detected.
#[derive(Debug)]
pub struct ApplyError;

/// Apply `delta` to the base data `base`, writing the result to `out`.
/// Errors if more than `limit` bytes would be written to `out`.
pub fn apply_limited(
    base: &[u8],
    mut delta: &[u8],
    out: &mut Vec<u8>,
    mut limit: usize,
) -> Result<(), ApplyError> {
    macro_rules! read_n {
        ($n:expr) => {{
            let n = $n;
            if delta.len() < n {
                return Err(ApplyError);
            }
            let (prefix, rest) = delta.split_at(n);
            delta = rest;
            prefix
        }};
    }
    macro_rules! read_int {
        ($ty:ty) => {{
            let mut b = [0; mem::size_of::<$ty>()];
            b.copy_from_slice(read_n!(mem::size_of::<$ty>()));
            <$ty>::from_be_bytes(b)
        }};
    }
    macro_rules! read_varint {
        ($len:expr) => {{
            let len = $len;
            let mut b = [0; 8];
            b[8 - len..8].copy_from_slice(read_n!(len));
            u64::from_be_bytes(b)
        }};
    }
    macro_rules! safe_cast {
        ($val:expr, $ty:ty) => {{
            let val = $val;
            if val as u64 > <$ty>::max_value() as u64 {
                return Err(ApplyError);
            }
            val as $ty
        }};
    }
    if read_int!(u32) != DELTA_MAGIC {
        return Err(ApplyError);
    }
    loop {
        let cmd = read_int!(u8);
        match cmd {
            RS_OP_END => {
                break;
            }
            RS_OP_LITERAL_1..=RS_OP_LITERAL_N8 => {
                let n = if cmd <= RS_OP_LITERAL_64 {
                    // <=64, length is encoded in `cmd`
                    (1 + cmd - RS_OP_LITERAL_1) as usize
                } else {
                    safe_cast!(read_varint!(1 << (cmd - RS_OP_LITERAL_N1) as usize), usize)
                };
                if n > limit {
                    return Err(ApplyError);
                }
                out.extend_from_slice(read_n!(n));
                limit -= n;
            }
            RS_OP_COPY_N1_N1..=RS_OP_COPY_N8_N8 => {
                let mode = cmd - RS_OP_COPY_N1_N1;
                let offset_len = 1 << (mode / 4) as usize;
                let len_len = 1 << (mode % 4) as usize;
                let offset = safe_cast!(read_varint!(offset_len), usize);
                let len = safe_cast!(read_varint!(len_len), usize);
                if len == 0 {
                    return Err(ApplyError);
                }
                let end = offset.checked_add(len).ok_or(ApplyError)?;
                if end > base.len() {
                    return Err(ApplyError);
                }
                if end - offset > limit {
                    return Err(ApplyError);
                }
                out.extend_from_slice(&base[offset..end]);
                limit -= end - offset;
            }
            _ => return Err(ApplyError),
        }
    }
    if delta.is_empty() {
        Ok(())
    } else {
        // extra content after EOF
        Err(ApplyError)
    }
}

/// Apply `delta` to the base data `base`, writing the result to `out`.
pub fn apply(base: &[u8], delta: &[u8], out: &mut Vec<u8>) -> Result<(), ApplyError> {
    apply_limited(base, delta, out, usize::max_value())
}
