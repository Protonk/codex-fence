pub mod identity;
pub mod model;
pub mod repository;

pub use identity::{
    CatalogKey, CapabilityCategory, CapabilityId, CapabilityLayer, CapabilitySnapshot,
};
pub use model::{
    Capability, CapabilityCatalog, CapabilitySource, DocRef, Operations, Scope,
};
pub use repository::CatalogRepository;

pub use model::load_catalog_from_path;
