use std::collections::HashMap;
use std::error::Error;
use std::fmt;

use arrayref::array_ref;

use crate::consts::{BLAKE2_MAGIC, MD4_MAGIC};
use crate::crc::Crc;
use crate::hasher::BuildCrcHasher;
use crate::hashmap_variant::SecondLayerMap;
use crate::md4::{md4, md4_many, MD4_SIZE};

/// An rsync signature.
///
/// A signature contains hashed information about a block of data. It is used to compute a delta
/// against that data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Signature {
    signature_type: SignatureType,
    block_size: u32,
    crypto_hash_size: u32,
    // This contains a valid serialized signature which must contain the correct magic for `signature_type`
    // and a matching `block_size` and `crypto_hash_size`.
    signature: Vec<u8>,
}

/// A signature with a block index, suitable for calculating deltas.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexedSignature<'a> {
    pub(crate) signature_type: SignatureType,
    pub(crate) block_size: u32,
    pub(crate) crypto_hash_size: u32,
    /// crc -> crypto hash -> block index
    pub(crate) blocks: HashMap<Crc, SecondLayerMap<&'a [u8], u32>, BuildCrcHasher>,
}

/// The hash type used with within the signature.
/// Note that this library generally only supports MD4 signatures.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum SignatureType {
    Md4,
    Blake2,
}

impl SignatureType {
    const SIZE: usize = 4;
    fn from_magic(bytes: [u8; Self::SIZE]) -> Option<Self> {
        match u32::from_be_bytes(bytes) {
            BLAKE2_MAGIC => Some(SignatureType::Blake2),
            MD4_MAGIC => Some(SignatureType::Md4),
            _ => None,
        }
    }
    fn to_magic(self) -> [u8; Self::SIZE] {
        match self {
            SignatureType::Md4 => MD4_MAGIC,
            SignatureType::Blake2 => BLAKE2_MAGIC,
        }
        .to_be_bytes()
    }
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

impl Signature {
    const HEADER_SIZE: usize = SignatureType::SIZE + 2 * 4; // magic, block_size, then crypto_hash_size

    /// Compute an MD4 signature for the given data.
    ///
    /// `options.block_size` must be greater than zero. `options.crypto_hash_size` must be at most 16, the length of an MD4 hash.
    /// Panics if the provided options are invalid.
    pub fn calculate(buf: &[u8], options: SignatureOptions) -> Signature {
        assert!(options.block_size > 0);
        assert!(options.crypto_hash_size <= MD4_SIZE as u32);
        let num_blocks = buf.chunks(options.block_size as usize).len();

        let signature_type = SignatureType::Md4;

        let mut signature = Vec::with_capacity(
            Self::HEADER_SIZE + num_blocks * (Crc::SIZE + options.crypto_hash_size as usize),
        );

        signature.extend_from_slice(&signature_type.to_magic());
        signature.extend_from_slice(&options.block_size.to_be_bytes());
        signature.extend_from_slice(&options.crypto_hash_size.to_be_bytes());

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
            let crc = Crc::new().update(block);
            let crypto_hash = &md4_hash[..options.crypto_hash_size as usize];
            signature.extend_from_slice(&crc.to_bytes());
            signature.extend_from_slice(crypto_hash);
        }
        Signature {
            signature_type: SignatureType::Md4,
            block_size: options.block_size,
            crypto_hash_size: options.crypto_hash_size,
            signature,
        }
    }

    /// Read a binary signature.
    pub fn deserialize(signature: Vec<u8>) -> Result<Signature, SignatureParseError> {
        if signature.len() < Self::HEADER_SIZE {
            return Err(SignatureParseError(()));
        }
        let signature_type = SignatureType::from_magic(*array_ref![signature, 0, 4])
            .ok_or(SignatureParseError(()))?;
        let block_size = u32::from_be_bytes(*array_ref![signature, 4, 4]);
        let crypto_hash_size = u32::from_be_bytes(*array_ref![signature, 8, 4]);
        let block_signature_size = Crc::SIZE + crypto_hash_size as usize;
        if (signature.len() - Self::HEADER_SIZE) % block_signature_size != 0 {
            return Err(SignatureParseError(()));
        }
        Ok(Signature {
            signature_type,
            block_size,
            crypto_hash_size,
            signature,
        })
    }

    /// Get the serialized form of this signature.
    pub fn serialized(&self) -> &[u8] {
        &self.signature
    }

    /// Get ownership of the serialized form of this signature.
    pub fn into_serialized(self) -> Vec<u8> {
        self.signature
    }

    fn blocks(&self) -> impl ExactSizeIterator<Item = (Crc, &[u8])> {
        self.signature[Self::HEADER_SIZE..]
            .chunks(Crc::SIZE + self.crypto_hash_size as usize)
            .map(|b| {
                (
                    Crc::from_bytes(*array_ref!(b, 0, Crc::SIZE)),
                    &b[Crc::SIZE..],
                )
            })
    }

    /// Convert a signature to a form suitable for computing deltas.
    pub fn index(&self) -> IndexedSignature<'_> {
        let blocks = self.blocks();
        let mut block_index: HashMap<Crc, SecondLayerMap<&[u8], u32>, BuildCrcHasher> =
            HashMap::with_capacity_and_hasher(blocks.len(), BuildCrcHasher::default());
        for (idx, (crc, crypto_hash)) in blocks.enumerate() {
            block_index
                .entry(crc)
                .or_default()
                .insert(crypto_hash, idx as u32);
        }

        // Multiple blocks having the same `Crc` value means that the hashmap will reserve more
        // capacity than needed. This is particularly noticable when `self.blocks` contains a very
        // large number of values
        block_index.shrink_to_fit();

        IndexedSignature {
            signature_type: self.signature_type,
            block_size: self.block_size,
            crypto_hash_size: self.crypto_hash_size,
            blocks: block_index,
        }
    }
}
