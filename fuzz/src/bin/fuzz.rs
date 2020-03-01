#[macro_use]
extern crate honggfuzz;

use fast_rsync::{apply_limited, ApplyError};
use rand::rngs::SmallRng;
use rand::{RngCore, SeedableRng};
use std::io::Cursor;

fn main() {
    const MAX_LEN: usize = 1 << 28;
    const MAX_OUT: usize = 1 << 28;
    let mut base_data = vec![0; MAX_LEN];
    SmallRng::seed_from_u64(0).fill_bytes(&mut base_data);
    let mut out_data = Vec::with_capacity(MAX_OUT);
    let mut librsync_data = vec![0; MAX_OUT];
    loop {
        fuzz!(|data: &[u8]| {
            if data.len() < 4 {
                return;
            }
            let (base_len, delta) = data.split_at(4);
            let base_len = u32::from_be_bytes([base_len[0], base_len[1], base_len[2], base_len[3]])
                as usize
                % MAX_LEN;
            let base_data = &base_data[..base_len];
            out_data.clear();

            let mut librsync_data_cursor = Cursor::new(&mut librsync_data[..]);
            let mut librsync_delta_cursor = &delta[..];
            let fast_rsync_result = apply_limited(base_data, delta, &mut out_data, MAX_OUT);
            let librsync_result = librsync::Patch::with_buf_read(
                &mut Cursor::new(base_data),
                &mut librsync_delta_cursor,
            )
            .and_then(|mut job| Ok(std::io::copy(&mut job, &mut librsync_data_cursor)?));
            match fast_rsync_result {
                Ok(()) => {
                    assert!(out_data.len() <= MAX_OUT);
                    assert!(librsync_result.is_ok());
                    // There must be no unconsumed input.
                    assert_eq!(librsync_delta_cursor, &[]);
                    let res = &out_data[..];
                    let librsync_len = librsync_data_cursor.position() as usize;
                    assert_eq!(res, &librsync_data[0..librsync_len]);
                }
                Err(ApplyError::UnexpectedEof {
                    reading: "literal",
                    expected,
                    ..
                }) if expected > u32::max_value() as usize => {
                    // librsync bug: literal lengths are truncated to 32 bits
                }
                Err(e) => {
                    // librsync can return success if there is still unconsumed
                    // input, but `fast_rsync` considers that an error. Account for
                    // that.
                    assert!(
                        librsync_result.is_err() || !librsync_delta_cursor.is_empty(),
                        "unexpected error: {:?}, delta={:?}, librsync len={}",
                        e,
                        delta,
                        librsync_data_cursor.position(),
                    );
                }
            }
        });
    }
}
