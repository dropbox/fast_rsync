use crate::consts::{
    DELTA_MAGIC, RS_OP_COPY_N1_N1, RS_OP_END, RS_OP_LITERAL_1, RS_OP_LITERAL_N1, RS_OP_LITERAL_N2,
    RS_OP_LITERAL_N4, RS_OP_LITERAL_N8,
};
use crate::crc::Crc;
use crate::hasher::BuildCrcHasher;
use crate::md4::{md4, MD4_SIZE};
use crate::signature::{IndexedSignature, SignatureType};
use std::collections::HashMap;

/// This controls how many times we will allow ourselves to fail at matching a
/// given crc before permanently giving up on it (essentially removing it from
/// the signature).
const MAX_CRC_COLLISIONS: u32 = 1024;

/// Indicates that a delta could not be calculated, generally because the
/// provided signature was invalid or unsupported.
#[derive(Debug)]
pub struct DiffError;

fn insert_command(len: u64, out: &mut Vec<u8>) {
    assert!(len != 0);
    if len <= 64 {
        out.push(RS_OP_LITERAL_1 + (len - 1) as u8);
    } else if len <= u8::max_value() as u64 {
        out.extend_from_slice(&[RS_OP_LITERAL_N1, len as u8]);
    } else if len <= u16::max_value() as u64 {
        out.reserve(3);
        out.push(RS_OP_LITERAL_N2);
        out.extend_from_slice(&(len as u16).to_be_bytes());
    } else if len <= u32::max_value() as u64 {
        out.reserve(5);
        out.push(RS_OP_LITERAL_N4);
        out.extend_from_slice(&(len as u32).to_be_bytes());
    } else {
        out.reserve(9);
        out.push(RS_OP_LITERAL_N8);
        out.extend_from_slice(&len.to_be_bytes());
    }
}

fn copy_command(offset: u64, len: u64, out: &mut Vec<u8>) {
    fn varint(val: u64, out: &mut Vec<u8>) -> u8 {
        if val <= u8::max_value() as u64 {
            out.push(val as u8);
            0
        } else if val <= u16::max_value() as u64 {
            out.extend_from_slice(&(val as u16).to_be_bytes());
            1
        } else if val <= u32::max_value() as u64 {
            out.extend_from_slice(&(val as u32).to_be_bytes());
            2
        } else {
            out.extend_from_slice(&val.to_be_bytes());
            3
        }
    }
    let command_offset = out.len();
    out.push(0); // dummy
    let offset_len = varint(offset, out);
    let len_len = varint(len, out);
    out[command_offset] = RS_OP_COPY_N1_N1 + offset_len * 4 + len_len;
}

struct OutputState {
    emitted: usize,
    queued_copy: Option<(u64, usize)>,
}

impl OutputState {
    fn emit(&mut self, until: usize, data: &[u8], out: &mut Vec<u8>) {
        if self.emitted == until {
            return;
        }
        if let Some((offset, len)) = self.queued_copy {
            copy_command(offset as u64, len as u64, out);
            self.emitted += len as usize;
        }
        if self.emitted < until {
            let to_emit = &data[self.emitted..until];
            insert_command(to_emit.len() as u64, out);
            out.extend_from_slice(to_emit);
            self.emitted = until;
        }
    }

    fn copy(&mut self, offset: u64, len: usize, here: usize, data: &[u8], out: &mut Vec<u8>) {
        if let Some((queued_offset, queued_len)) = self.queued_copy {
            if self.emitted + queued_len == here && queued_offset + queued_len as u64 == offset {
                // just extend the copy
                self.queued_copy = Some((queued_offset, queued_len + len));
                return;
            }
        }
        self.emit(here, data, out);
        self.queued_copy = Some((offset, len));
    }
}

/// Calculate a delta and write it to `out`.
/// This delta can be applied to the base data represented by `signature` to
/// reconstruct `data`.
pub fn diff(
    signature: &IndexedSignature<'_>,
    data: &[u8],
    out: &mut Vec<u8>,
) -> Result<(), DiffError> {
    let block_size = signature.block_size;
    let crypto_hash_size = signature.crypto_hash_size as usize;
    if let SignatureType::Md4 = signature.signature_type {
        if crypto_hash_size > MD4_SIZE {
            return Err(DiffError);
        }
    } else {
        return Err(DiffError);
    }
    out.extend_from_slice(&DELTA_MAGIC.to_be_bytes());
    let mut state = OutputState {
        emitted: 0,
        queued_copy: None,
    };
    let mut here = 0;
    let mut collisions: HashMap<Crc, u32, BuildCrcHasher> =
        HashMap::with_hasher(BuildCrcHasher::default());
    while data.len() - here >= block_size as usize {
        let mut crc = Crc::new().update(&data[here..here + block_size as usize]);
        loop {
            // if we detect too many CRC collisions, blacklist the CRC to avoid DoS
            if collisions
                .get(&crc)
                .map_or(true, |&count| count < MAX_CRC_COLLISIONS)
            {
                if let Some(blocks) = signature.blocks.get(&crc) {
                    let digest = md4(&data[here..here + block_size as usize]);
                    if let Some(&idx) = blocks.get(&digest[..crypto_hash_size]) {
                        // match found
                        state.copy(
                            idx as u64 * block_size as u64,
                            block_size as usize,
                            here,
                            data,
                            out,
                        );
                        here += block_size as usize;
                        break;
                    }
                    // CRC collision
                    *collisions.entry(crc).or_insert(0) += 1;
                }
            }
            // no match, try to extend
            here += 1;
            if here + block_size as usize > data.len() {
                break;
            }
            crc = crc.rotate(
                block_size,
                data[here - 1],
                data[here + block_size as usize - 1],
            );
        }
    }
    state.emit(data.len(), data, out);
    out.push(RS_OP_END);
    Ok(())
}
