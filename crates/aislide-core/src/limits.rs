use serde::{Deserialize, Serialize};

pub const MIB: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CapacityProfile {
    Legacy,
    Standard,
    #[default]
    Large,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct CapacityLimits {
    pub slides: usize,
    pub elements_per_slide: usize,
    pub elements_total: usize,
    pub group_depth: usize,
    pub document_bytes: usize,
    pub request_bytes: usize,
    pub archive_bytes: usize,
    pub image_encoded_bytes: usize,
    pub unique_images: usize,
    pub raster_work_bytes: usize,
    pub json_nodes: usize,
    pub json_depth: usize,
}

pub const LEGACY: CapacityLimits = CapacityLimits {
    slides: 32, elements_per_slide: 256, elements_total: 2048, group_depth: 8,
    document_bytes: 2 * MIB, request_bytes: 4 * MIB, archive_bytes: 3 * MIB - 1024,
    image_encoded_bytes: 3 * MIB, unique_images: 128, raster_work_bytes: 64 * MIB, json_nodes: 250_000, json_depth: 64,
};

pub const STANDARD: CapacityLimits = CapacityLimits {
    slides: 128, elements_total: 8192, document_bytes: 8 * MIB,
    request_bytes: 16 * MIB, archive_bytes: 8 * MIB,
    ..LEGACY
};

pub const LARGE: CapacityLimits = CapacityLimits {
    slides: 256, document_bytes: 32 * MIB, request_bytes: 96 * MIB,
    archive_bytes: 16 * MIB,
    ..STANDARD
};

pub const MAX_PROVENANCE_IDENTITIES: usize = LARGE.slides + 8 + 32;

impl CapacityProfile {
    pub const fn limits(self) -> &'static CapacityLimits {
        match self { Self::Legacy => &LEGACY, Self::Standard => &STANDARD, Self::Large => &LARGE }
    }
}