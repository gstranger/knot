//! Serde helpers for Arc-wrapped types used throughout the kernel.

/// Serialize / deserialize `Arc<[T]>` as a sequence.
///
/// Usage: `#[serde(with = "knot_core::serde_arc::arc_slice")]`
pub mod arc_slice {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::sync::Arc;

    pub fn serialize<T: Serialize, S: Serializer>(
        data: &Arc<[T]>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        data.as_ref().serialize(serializer)
    }

    pub fn deserialize<'de, T: Deserialize<'de>, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Arc<[T]>, D::Error> {
        Vec::<T>::deserialize(deserializer).map(Arc::from)
    }
}
