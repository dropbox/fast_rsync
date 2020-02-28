#[macro_use]
extern crate honggfuzz;

use fast_rsync::apply_limited;
use rand::rngs::SmallRng;
use rand::{RngCore, SeedableRng};
use std::io::Cursor;

fn main() {
    const MAX_LEN: usize = 1 << 28;
    const MAX_OUT: usize = 1 << 28;
    let mut base_data = vec![0; MAX_LEN];
    SmallRng::seed_from_u64(0).fill_bytes(&mut base_data);
    let mut out_data = Vec::with_capacity(MAX_OUT);
    let mut librsync_data = Vec::with_capacity(MAX_OUT);
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
            if apply_limited(base_data, delta, &mut out_data, MAX_OUT).is_ok() {
                let res = &out_data[..];
                librsync_data.clear();
                librsync::whole::patch(
                    &mut Cursor::new(base_data),
                    &mut &delta[..],
                    &mut librsync_data,
                )
                .unwrap();
                assert_eq!(res, &librsync_data[..]);
            }
            // only compare in the success case - librsync is annoyingly non-robust against weird input
        });
    }
}
