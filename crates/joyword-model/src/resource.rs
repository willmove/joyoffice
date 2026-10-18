//! Immutable binary resources identified by content hash and source
//! (ARCHITECTURE §2.2). Equal bytes from different sources stay separate
//! resources: deduplication changes what preserved content refers to, so it is
//! an explicit operation, never an import side effect. Changing a resource means
//! adding a new one and repointing the referring object.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use sha2::{Digest, Sha256};

use crate::id::{ResourceId, SourceRef};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentHash([u8; 32]);

impl ContentHash {
    pub fn of(data: &[u8]) -> Self {
        Self(Sha256::digest(data).into())
    }

    pub fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|byte| write!(f, "{byte:02x}"))
    }
}

impl fmt::Debug for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sha256:{self}")
    }
}

#[derive(Debug)]
pub struct Resource {
    id: ResourceId,
    hash: ContentHash,
    media_type: Arc<str>,
    data: Arc<[u8]>,
    source: Option<Arc<SourceRef>>,
}

impl Resource {
    pub fn id(&self) -> ResourceId {
        self.id
    }

    pub fn hash(&self) -> ContentHash {
        self.hash
    }

    pub fn media_type(&self) -> &str {
        &self.media_type
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn source(&self) -> Option<&SourceRef> {
        self.source.as_deref()
    }
}

#[derive(Clone, Debug, Default)]
pub struct ResourceTable {
    resources: BTreeMap<ResourceId, Arc<Resource>>,
}

impl ResourceTable {
    pub fn insert(
        &mut self,
        id: ResourceId,
        media_type: &str,
        data: Arc<[u8]>,
        source: Option<Arc<SourceRef>>,
    ) -> ResourceId {
        let resource = Resource {
            id,
            hash: ContentHash::of(&data),
            media_type: media_type.into(),
            data,
            source,
        };
        let previous = self.resources.insert(id, Arc::new(resource));
        assert!(previous.is_none(), "resource {id} inserted twice");
        id
    }

    pub fn get(&self, id: ResourceId) -> Option<&Arc<Resource>> {
        self.resources.get(&id)
    }

    pub fn find_by_hash(&self, hash: ContentHash) -> impl Iterator<Item = &Arc<Resource>> {
        self.resources
            .values()
            .filter(move |resource| resource.hash == hash)
    }

    /// An existing resource with the same media type and bytes.
    ///
    /// Import keeps one resource per package part, even when the bytes match,
    /// because each part has its own source. Editor insertion uses this to
    /// avoid storing a second copy of the same image.
    pub fn find_same(&self, media_type: &str, data: &[u8]) -> Option<ResourceId> {
        let hash = ContentHash::of(data);
        self.resources
            .values()
            .find(|resource| {
                resource.hash == hash
                    && resource.media_type.as_ref() == media_type
                    && resource.data.as_ref() == data
            })
            .map(|resource| resource.id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Arc<Resource>> {
        self.resources.values()
    }

    pub fn len(&self) -> usize {
        self.resources.len()
    }

    pub fn is_empty(&self) -> bool {
        self.resources.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::IdAllocator;

    #[test]
    fn equal_bytes_from_different_sources_stay_distinct() {
        let mut ids = IdAllocator::default();
        let mut table = ResourceTable::default();
        let bytes: Arc<[u8]> = Arc::from(&b"\x89PNG original"[..]);
        let source = |part: &str| {
            Some(Arc::new(SourceRef {
                part: part.into(),
                locator: "rId1".into(),
            }))
        };
        let a = table.insert(
            ids.resource(),
            "image/png",
            bytes.clone(),
            source("/word/media/a.png"),
        );
        let b = table.insert(
            ids.resource(),
            "image/png",
            bytes,
            source("/word/media/b.png"),
        );
        assert_ne!(a, b);
        let hash = table.get(a).unwrap().hash();
        assert_eq!(hash, table.get(b).unwrap().hash());
        assert_eq!(table.find_by_hash(hash).count(), 2);

        // Editing `a` adds a new resource; `b` and its users are unaffected.
        let edited = table.insert(ids.resource(), "image/png", Arc::from(&b"edited"[..]), None);
        assert_ne!(table.get(edited).unwrap().hash(), hash);
        assert_eq!(table.get(b).unwrap().data(), b"\x89PNG original");
    }

    #[test]
    fn hash_matches_known_sha256_vector() {
        assert_eq!(
            ContentHash::of(b"abc").to_string(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
