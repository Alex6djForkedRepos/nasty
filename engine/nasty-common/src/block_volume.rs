use std::collections::HashMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Stable identity of a block subvolume, independent of its pool name,
/// subvolume name, mount point, and current loop-device number.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct BlockVolumeId {
    pub filesystem_uuid: String,
    pub subvolume_id: u32,
}

/// Runtime result of restoring block-subvolume loop devices.
#[derive(Debug, Clone, Default)]
pub struct BlockDeviceMappings {
    /// Stable identity to the loop device attached during this boot.
    pub current: HashMap<BlockVolumeId, String>,
    /// Exact loop paths that were already attached to their managed backing
    /// files before this engine start. These are safe evidence for upgrading
    /// legacy export state; freshly allocated cold-boot paths are not.
    pub preexisting: HashMap<String, BlockVolumeId>,
}

impl BlockDeviceMappings {
    pub fn is_empty(&self) -> bool {
        self.current.is_empty()
    }

    pub fn legacy_identity_for_exact_path(&self, path: &str) -> Option<&BlockVolumeId> {
        self.preexisting.get(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_lookup_requires_an_exact_preexisting_path() {
        let identity = BlockVolumeId {
            filesystem_uuid: "pool".into(),
            subvolume_id: 7,
        };
        let mut mappings = BlockDeviceMappings::default();
        mappings
            .current
            .insert(identity.clone(), "/dev/loop1".into());

        assert!(
            mappings
                .legacy_identity_for_exact_path("/dev/loop1")
                .is_none()
        );

        mappings
            .preexisting
            .insert("/dev/loop1".into(), identity.clone());
        assert_eq!(
            mappings.legacy_identity_for_exact_path("/dev/loop1"),
            Some(&identity)
        );
        assert!(
            mappings
                .legacy_identity_for_exact_path("/dev/loop10")
                .is_none()
        );
    }
}
