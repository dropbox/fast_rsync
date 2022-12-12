//! Utilities for loading and transposing data from memory on AArch64.
//! This is useful for SPMD-style operations.
#![cfg(target_endian = "little")] // only little-endian supported

use arrayref::array_ref;

use std::arch::aarch64::{uint32x4_t, vtrnq_u32, vzipq_u32};

#[inline(always)]
/// Loads four u32s (little-endian), potentially unaligned
unsafe fn load_u32x4(slice: &[u8; 16]) -> uint32x4_t {
    core::mem::transmute(*slice)
}

/// Load 16 bytes (1 u32x4) out of each lane of `data`, transposed.
#[inline]
#[target_feature(enable = "neon")]
unsafe fn load_transpose4(data: [&[u8; 16]; 4]) -> [uint32x4_t; 4] {
    let i0 = load_u32x4(data[0]);
    let i1 = load_u32x4(data[1]);
    let i2 = load_u32x4(data[2]);
    let i3 = load_u32x4(data[3]);
    // [[data[0][0], data[2][0], data[0][2], data[2][2]], [data[0][1], data[2][1], data[0][3], data[2][3]]
    let tr02 = vtrnq_u32(i0, i2);
    // [[data[1][0], data[3][0], data[1][2], data[3][2]], [data[1][1], data[3][1], data[1][3], data[3][3]]
    let tr13 = vtrnq_u32(i1, i3);
    // [[data[0][0], data[1][0], data[2][0], data[3][0]], [data[0][2], data[1][2], data[2][2], data[3][2]]]
    let zip02 = vzipq_u32(tr02.0, tr13.0);
    let zip13 = vzipq_u32(tr02.1, tr13.1);
    [zip02.0, zip13.0, zip02.1, zip13.1]
}

macro_rules! get_blocks {
    ($data: ident, ($($lane: tt)*), $from: expr, $width: expr) => ([$(array_ref![&$data($lane), $from, $width]),*]);
}

#[inline]
#[target_feature(enable = "neon")]
pub unsafe fn load_16x4<'a, F: Fn(usize) -> &'a [u8; 64]>(data: F) -> [uint32x4_t; 16] {
    core::mem::transmute::<[[uint32x4_t; 4]; 4], [uint32x4_t; 16]>([
        load_transpose4(get_blocks!(data, (0 1 2 3), 0, 16)),
        load_transpose4(get_blocks!(data, (0 1 2 3), 16, 16)),
        load_transpose4(get_blocks!(data, (0 1 2 3), 32, 16)),
        load_transpose4(get_blocks!(data, (0 1 2 3), 48, 16)),
    ])
}

#[test]
fn test_transpose() {
    let mut input = [[0; 64]; 4];
    for lane in 0..4 {
        for i in 0..16 {
            let value = (lane * 16 + i) as u32;
            input[lane][i * 4..i * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
    }
    unsafe {
        let output = load_16x4(|lane| &input[lane]);
        let transmuted = core::mem::transmute::<_, [[u32; 4]; 16]>(output);
        for lane in 0..4 {
            for i in 0..16 {
                assert_eq!(transmuted[i][lane], (lane * 16 + i) as u32);
            }
        }
    }
}
