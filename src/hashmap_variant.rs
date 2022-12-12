//! Contains a hashmap optimized for the second layer of the
//! [`IndexedSignature`][crate::signature::IndexedSignature]

use std::{collections::HashMap, hash::Hash, mem};

/// A single entry optimized hashmap intended for use in the second layer map in
/// [`IndexedSignature`][crate::signature::IndexedSignature]
///
/// The [`IndexedSignature`][crate::signature::IndexedSignature] contains a two-layer hashmap. The
/// first layer is keyed on a cheap, but weak, rolling hash that maps to a slower, but stronger,
/// hash to better guarantee an accurate match on the block.
///
/// This means that there are only multiple entries in the second layer map when there is a hash
/// collision from the weak hash in the first layer which is rare. We can use this to optimize the
/// map for the common case of a single entry while [`Box`]ing the fallback of two or more entries.
///
/// With this the current use case of `SecondLayerMap<&[u8], u32>` takes up 24 bytes on 64-bit
/// systems while `HashMap<&[u8], u32>` takes 48. Beyond that a [`SecondLayerMap`] consists of just
/// a match and an if
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SecondLayerMap<K, V>
where
    K: Eq + Hash,
{
    Empty,
    Single(K, V),
    TwoOrMore(Box<HashMap<K, V>>),
}

impl<K, V> Default for SecondLayerMap<K, V>
where
    K: Eq + Hash,
{
    fn default() -> Self {
        Self::Empty
    }
}

impl<K, V> SecondLayerMap<K, V>
where
    K: Eq + Hash,
{
    /// Analogous to [`HashMap::insert`]
    pub fn insert(&mut self, key: K, val: V) -> Option<V> {
        let old_state = mem::replace(self, Self::Empty);

        let (new_state, ret) = match old_state {
            Self::Empty => (Self::Single(key, val), None),
            Self::Single(old_key, old_val) => {
                let mut map = Box::new(HashMap::with_capacity(2));
                map.insert(key, val);
                let ret = map.insert(old_key, old_val);
                (Self::TwoOrMore(map), ret)
            }
            Self::TwoOrMore(mut map) => {
                let ret = map.insert(key, val);
                (Self::TwoOrMore(map), ret)
            }
        };

        *self = new_state;
        ret
    }

    /// Analogous to [`HashMap::get`]
    pub fn get(&self, needle: &K) -> Option<&V> {
        match self {
            Self::Single(key, val) => {
                if needle == key {
                    Some(val)
                } else {
                    None
                }
            }
            Self::TwoOrMore(map) => map.get(needle),
            Self::Empty => None,
        }
    }
}
