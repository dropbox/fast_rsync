use std::{collections::HashMap, hash::Hash, mem};

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
    pub fn insert(&mut self, key: K, val: V) -> Option<V> {
        let old_state = mem::replace(self, Self::Empty);

        let (new_state, ret) = match old_state {
            Self::Empty => (Self::Single(key, val), None),
            Self::Single(old_key, old_val) => {
                let mut map = Box::new(HashMap::new());
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

    pub fn get(&self, needle: &K) -> Option<&V> {
        match self {
            Self::Single(key, val) => {
                if needle == key {
                    Some(&val)
                } else {
                    None
                }
            }
            Self::TwoOrMore(map) => map.get(needle),
            Self::Empty => None,
        }
    }
}
