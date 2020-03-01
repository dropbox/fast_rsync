use std::collections::HashMap;
use std::error::Error;
use std::fmt;

use crate::consts::{BLAKE2_MAGIC, MD4_MAGIC};
use crate::crc::Crc;
use crate::hasher::BuildCrcHasher;
use crate::md4::{md4, md4_many, MD4_SIZE};

/// An rsync signature.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Signature<'a> {
    pub(crate) signature_type: SignatureType,
    pub(crate) block_size: u32,
    pub(crate) crypto_hash_size: u32,
    pub(crate) blocks: Vec<BlockSignature<'a>>,
}

/// The signature of a single block.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) struct BlockSignature<'a> {
    pub(crate) crc: Crc,
    pub(crate) crypto_hash: &'a [u8],
}

/// A signature with a block index, suitable for calculating deltas.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexedSignature<'a> {
    pub(crate) signature_type: SignatureType,
    pub(crate) block_size: u32,
    pub(crate) crypto_hash_size: u32,
    /// crc -> crypto hash -> block index
    pub(crate) blocks: HashMap<Crc, HashMap<&'a [u8], u32>, BuildCrcHasher>,
}

/// The hash type used with within the signature.
/// Note that this library generally only supports MD4 signatures.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SignatureType {
    Md4,
    Blake2,
}

/// Indicates that a signature was not valid.
#[derive(Debug)]
pub struct SignatureParseError(());

impl fmt::Display for SignatureParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid or unsupported signature")
    }
}

impl Error for SignatureParseError {}

/// Options for [Signature::calculate].
#[derive(Copy, Clone, Debug)]
pub struct SignatureOptions {
    /// The granularity of the signature.
    /// Smaller block sizes yield larger, but more precise, signatures.
    pub block_size: u32,
    /// The number of bytes to use from the MD4 hash. Must be at most 16.
    /// The larger this is, the less likely that a delta will be mis-applied.
    pub crypto_hash_size: u32,
}

impl<'a> Signature<'a> {
    /// Compute an MD4 signature.
    /// `storage` will be overwritten and the resulting signature will contain
    /// references into it.
    pub fn calculate(
        buf: &[u8],
        storage: &'a mut Vec<u8>,
        options: SignatureOptions,
    ) -> Signature<'a> {
        assert!(options.block_size > 0);
        assert!(options.crypto_hash_size <= MD4_SIZE as u32);
        let num_blocks = buf.chunks(options.block_size as usize).len();
        let mut blocks = Vec::with_capacity(num_blocks);

        // Create space in `storage` for our crypto hashes
        storage.resize(num_blocks * options.crypto_hash_size as usize, 0);
        let mut storage = storage.as_mut_slice();

        // Hash all the blocks (with the CRC as well as MD4)
        let chunks = buf.chunks_exact(options.block_size as usize);
        let remainder = chunks.remainder();
        for (block, md4_hash) in md4_many(chunks).chain(if remainder.is_empty() {
            None
        } else {
            // Manually tack on the last block if necessary, since `md4_many`
            // requires every block to be identical in size
            Some((remainder, md4(remainder)))
        }) {
            // would be nice to use `chunks_exact_mut`, but it doesn't work for zero sizes
            let (crypto_hash, rest) = storage.split_at_mut(options.crypto_hash_size as usize);
            storage = rest;

            crypto_hash.copy_from_slice(&md4_hash[..crypto_hash.len()]);
            let crc = Crc::new().update(&block);
            blocks.push(BlockSignature { crc, crypto_hash });
        }
        Signature {
            signature_type: SignatureType::Md4,
            block_size: options.block_size,
            crypto_hash_size: options.crypto_hash_size,
            blocks,
        }
    }

    /// Read a binary signature.
    pub fn deserialize(mut buf: &'a [u8]) -> Result<Signature<'a>, SignatureParseError> {
        macro_rules! read_n {
            ($n:expr) => {{
                let n = $n;
                if buf.len() < n {
                    return Err(SignatureParseError(()));
                }
                let (prefix, rest) = buf.split_at(n);
                buf = rest;
                prefix
            }};
        }
        macro_rules! read_u32 {
            () => {{
                let mut b = [0; 4];
                b.copy_from_slice(read_n!(4));
                u32::from_be_bytes(b)
            }};
        }

        let magic = read_u32!();
        let signature_type = match magic {
            MD4_MAGIC => SignatureType::Md4,
            BLAKE2_MAGIC => SignatureType::Blake2,
            _ => return Err(SignatureParseError(())),
        };
        let block_size = read_u32!();
        let crypto_hash_size = read_u32!();
        let block_signature_size = (4 + crypto_hash_size) as usize;
        if buf.len() % block_signature_size != 0 {
            return Err(SignatureParseError(()));
        }
        let mut blocks = Vec::with_capacity(buf.len() % block_signature_size);
        while !buf.is_empty() {
            let crc = Crc(read_u32!());
            let crypto_hash = read_n!(crypto_hash_size as usize);
            blocks.push(BlockSignature { crc, crypto_hash });
        }
        Ok(Signature {
            signature_type,
            block_size,
            crypto_hash_size,
            blocks,
        })
    }

    /// Write a signature to the given vector.
    pub fn serialize(&self, out: &mut Vec<u8>) {
        out.reserve(12 + (4 + self.crypto_hash_size as usize) * self.blocks.len());
        let magic = match self.signature_type {
            SignatureType::Md4 => MD4_MAGIC,
            SignatureType::Blake2 => BLAKE2_MAGIC,
        };
        out.extend_from_slice(&magic.to_be_bytes());
        out.extend_from_slice(&self.block_size.to_be_bytes());
        out.extend_from_slice(&self.crypto_hash_size.to_be_bytes());
        for block in &self.blocks {
            out.extend_from_slice(&block.crc.0.to_be_bytes());
            out.extend_from_slice(block.crypto_hash);
        }
    }

    /// Convert a signature to a form suitable for computing deltas.
    pub fn index(&self) -> IndexedSignature<'a> {
        let mut blocks: HashMap<Crc, HashMap<&[u8], u32>, BuildCrcHasher> =
            HashMap::with_capacity_and_hasher(self.blocks.len(), BuildCrcHasher::default());
        for (idx, block) in self.blocks.iter().enumerate() {
            blocks
                .entry(block.crc)
                .or_default()
                .insert(block.crypto_hash, idx as u32);
        }
        IndexedSignature {
            signature_type: self.signature_type,
            block_size: self.block_size,
            crypto_hash_size: self.crypto_hash_size,
            blocks,
        }
    }
}
