use std::hash::{BuildHasherDefault, Hasher};

/// A very simple hasher designed for hashing `Crc`.
#[derive(Default)]
pub struct CrcHasher {
    state: u32,
}

impl Hasher for CrcHasher {
    fn write(&mut self, _: &[u8]) {
        panic!("not designed for general writes");
    }
    #[inline]
    fn write_u32(&mut self, val: u32) {
        assert_eq!(self.state, 0, "can't hash more than one u32");
        self.state = val;
    }
    #[inline]
    fn finish(&self) -> u64 {
        // the avalanche function from xxhash
        // TODO: maybe use the 32-bit version on 32-bit?
        let mut val = self.state as u64;
        val ^= val >> 33;
        val = val.wrapping_mul(0xC2B2AE3D27D4EB4F);
        val ^= val >> 29;
        val = val.wrapping_mul(0x165667B19E3779F9);
        val ^= val >> 32;
        val
    }
}

pub type BuildCrcHasher = BuildHasherDefault<CrcHasher>;
