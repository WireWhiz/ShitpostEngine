pub use std::hash::Hash;
use std::{
    marker::PhantomData,
    ops::{Index, IndexMut},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StableTableKey {
    pub index: usize,
    pub version: usize,
}

#[macro_export]
macro_rules! define_stable_key {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name(pub crate::stable_table::StableTableKey);

        impl From<crate::stable_table::StableTableKey> for $name {
            fn from(key: crate::stable_table::StableTableKey) -> Self {
                Self(key)
            }
        }

        impl From<$name> for crate::stable_table::StableTableKey {
            fn from(key: $name) -> Self {
                key.0
            }
        }

        impl AsRef<crate::stable_table::StableTableKey> for $name {
            fn as_ref(&self) -> &crate::stable_table::StableTableKey {
                &self.0
            }
        }
    };
}

/// Contiguous data structure with hash-less O(1) lookup that guarentess that valid keys will never change due to mutation
pub struct StableTable<Key, Value> {
    pub entries: Vec<StableTableEntry<Value>>,
    pub unused_keys: Vec<Key>,
}

impl<Key, Value> StableTable<Key, Value>
where
    Key: Hash
        + PartialEq
        + Into<StableTableKey>
        + From<StableTableKey>
        + AsRef<StableTableKey>
        + Copy,
{
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, value: Value) -> Key {
        if let Some(unused_key) = self.unused_keys.pop() {
            // We don't bump they key or entry verison here, because we bump it when we invalidate the index
            self.entries[unused_key.as_ref().index].value = Some(value);
            unused_key
        } else {
            let index = self.entries.len();
            self.entries.push(StableTableEntry {
                version: 0,
                value: Some(value),
            });
            StableTableKey { index, version: 0 }.into()
        }
    }

    pub fn get(&self, key: Key) -> Option<&Value> {
        if let Some(entry) = self.entries.get(key.as_ref().index) {
            if entry.version != key.as_ref().version {
                None
            } else {
                Some(entry.value.as_ref().unwrap())
            }
        } else {
            None
        }
    }

    pub fn get_mut(&mut self, key: Key) -> Option<&mut Value> {
        if let Some(entry) = self.entries.get_mut(key.as_ref().index) {
            if entry.version != key.as_ref().version {
                None
            } else {
                Some(entry.value.as_mut().unwrap())
            }
        } else {
            None
        }
    }

    pub fn remove(&mut self, key: Key) -> bool {
        if let Some(entry) = self.entries.get_mut(key.as_ref().index) {
            if entry.version != key.as_ref().version {
                false
            } else {
                entry.version += 1;
                let mut key = key.into();
                key.version = entry.version;
                self.unused_keys.push(key.into());
                true
            }
        } else {
            false
        }
    }

    pub fn iter(&self) -> StableTableIter<'_, Key, Value> {
        self.into_iter()
    }

    pub fn iter_mut(&mut self) -> StableTableIterMut<'_, Key, Value> {
        self.into_iter()
    }
}

impl<Key, Value> Default for StableTable<Key, Value>
where
    Key: Hash + PartialEq + Into<StableTableKey> + From<StableTableKey>,
{
    fn default() -> Self {
        Self {
            entries: Default::default(),
            unused_keys: Default::default(),
        }
    }
}

impl<Key, Value> Index<Key> for StableTable<Key, Value>
where
    Key: Hash
        + PartialEq
        + Into<StableTableKey>
        + From<StableTableKey>
        + AsRef<StableTableKey>
        + Copy,
{
    type Output = Value;

    fn index(&self, key: Key) -> &Self::Output {
        if let Some(value) = self.get(key) {
            return value;
        }
        panic!("Tried to access StableTable by invalid or deleted key");
    }
}

impl<Key, Value> IndexMut<Key> for StableTable<Key, Value>
where
    Key: Hash
        + PartialEq
        + Into<StableTableKey>
        + From<StableTableKey>
        + AsRef<StableTableKey>
        + Copy,
{
    fn index_mut(&mut self, key: Key) -> &mut Self::Output {
        if let Some(value) = self.get_mut(key) {
            return value;
        }
        panic!("Tried to access StableTable by invalid or deleted key");
    }
}

pub struct StableTableEntry<Value> {
    pub version: usize,
    pub value: Option<Value>,
}

pub struct StableTableIter<'a, Key, Value> {
    entries: std::iter::Enumerate<std::slice::Iter<'a, StableTableEntry<Value>>>,
    _ghost: PhantomData<Key>,
}

pub struct StableTableIterMut<'a, Key, Value> {
    entries: std::iter::Enumerate<std::slice::IterMut<'a, StableTableEntry<Value>>>,
    _ghost: PhantomData<Key>,
}

impl<'a, Key, Value> Iterator for StableTableIter<'a, Key, Value>
where
    Key: From<StableTableKey>,
{
    type Item = (Key, &'a Value);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.entries.next() {
                None => return None,
                Some((index, entry)) => {
                    if let Some(value) = entry.value.as_ref() {
                        return Some((
                            StableTableKey {
                                index,
                                version: entry.version,
                            }
                            .into(),
                            value,
                        ));
                    }
                }
            }
        }
    }
}

impl<'a, Key, Value> Iterator for StableTableIterMut<'a, Key, Value>
where
    Key: From<StableTableKey>,
{
    type Item = (Key, &'a mut Value);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.entries.next() {
                None => return None,
                Some((index, entry)) => {
                    if let Some(value) = entry.value.as_mut() {
                        return Some((
                            StableTableKey {
                                index,
                                version: entry.version,
                            }
                            .into(),
                            value,
                        ));
                    }
                }
            }
        }
    }
}

// IntoIterator impls on StableTable
impl<'a, Key, Value> IntoIterator for &'a StableTable<Key, Value>
where
    Key: Hash
        + PartialEq
        + Into<StableTableKey>
        + From<StableTableKey>
        + AsRef<StableTableKey>
        + Copy,
{
    type Item = (Key, &'a Value);
    type IntoIter = StableTableIter<'a, Key, Value>;

    fn into_iter(self) -> Self::IntoIter {
        StableTableIter {
            entries: self.entries.iter().enumerate(),
            _ghost: Default::default(),
        }
    }
}

impl<'a, Key, Value> IntoIterator for &'a mut StableTable<Key, Value>
where
    Key: Hash
        + PartialEq
        + Into<StableTableKey>
        + From<StableTableKey>
        + AsRef<StableTableKey>
        + Copy,
{
    type Item = (Key, &'a mut Value);
    type IntoIter = StableTableIterMut<'a, Key, Value>;

    fn into_iter(self) -> Self::IntoIter {
        StableTableIterMut {
            entries: self.entries.iter_mut().enumerate(),
            _ghost: Default::default(),
        }
    }
}
