use std::hash::{DefaultHasher, Hash, Hasher};
use std::marker::PhantomData;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A content-addressable identifier for any topology/geometry entity.
/// Deterministic: same content always produces the same Id.
pub struct Id<T> {
    hash: u64,
    _phantom: PhantomData<T>,
}

// Manual impls to avoid requiring T: PartialEq/Eq/Hash/etc.
impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Id<T> {}
impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}
impl<T> Eq for Id<T> {}
impl<T> std::hash::Hash for Id<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hash.hash(state);
    }
}
impl<T> std::fmt::Debug for Id<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Id({:#x})", self.hash)
    }
}

impl<T> Id<T> {
    pub fn from_hash(hash: u64) -> Self {
        Self {
            hash,
            _phantom: PhantomData,
        }
    }

    pub fn hash_value(&self) -> u64 {
        self.hash
    }
}

impl<T> Serialize for Id<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.hash.serialize(serializer)
    }
}

impl<'de, T> Deserialize<'de> for Id<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        u64::deserialize(deserializer).map(Id::from_hash)
    }
}

/// Trait for types that produce a deterministic content hash.
pub trait ContentHash {
    fn content_hash(&self) -> u64;

    fn id(&self) -> Id<Self>
    where
        Self: Sized,
    {
        Id::from_hash(self.content_hash())
    }
}

/// Helper: hash any hashable value deterministically.
pub fn hash_deterministic<H: Hash>(value: &H) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}
