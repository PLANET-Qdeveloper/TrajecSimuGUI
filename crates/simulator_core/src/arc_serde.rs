//! Serde adapter for `Arc<[T]>` fields.
//!
//! Serde's `rc` feature gives us `Arc<T>` support for sized `T`, but
//! `Arc<[T]>` (an `Arc` to a DST slice) needs a manual round-trip
//! through `Vec<T>`. This module provides the `serialize_with` /
//! `deserialize_with` pair so each `Arc<[T]>` field can be tagged
//! with `#[serde(with = "crate::arc_serde::slice")]`.

pub mod slice {
    use std::sync::Arc;

    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S, T>(arc: &Arc<[T]>, s: S) -> Result<S::Ok, S::Error>
    where
        T: Serialize,
        S: Serializer,
    {
        // Defer to the slice impl so the wire format is identical to `Vec<T>`.
        arc.as_ref().serialize(s)
    }

    pub fn deserialize<'de, D, T>(d: D) -> Result<Arc<[T]>, D::Error>
    where
        T: Deserialize<'de>,
        D: Deserializer<'de>,
    {
        let v: Vec<T> = Vec::deserialize(d)?;
        Ok(Arc::from(v))
    }
}
