//! Indexed view of a capability catalog instance.
//!
//! The index enforces the expected catalog schema version and provides fast
//! lookup by capability id. It is intentionally strict about duplicates and
//! unknown schema versions so helper binaries cannot silently consume
//! mismatched catalogs.

use crate::catalog::load_catalog_from_path;
use crate::catalog::{Capability, CapabilityCatalog, CapabilityId, CatalogKey, CatalogMetadata};
use anyhow::{Context, Result, bail};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

// The harness currently ships a single catalog; reject unexpected versions
// rather than risk emitting records with mismatched metadata. Allow callers to
// widen the accepted set via env while keeping a sane default.
const DEFAULT_SCHEMA_VERSION: &str = "sandbox_catalog_v1";
const ENV_ALLOWED_SCHEMA_VERSIONS: &str = "FENCE_ALLOWED_CATALOG_SCHEMAS";

#[derive(Debug)]
/// Capability catalog plus a derived index keyed by capability id.
pub struct CapabilityIndex {
    catalog_key: CatalogKey,
    catalog: CapabilityCatalog,
    by_id: BTreeMap<CapabilityId, Capability>,
}

impl CapabilityIndex {
    /// Load and validate the catalog from disk.
    ///
    /// Validates the schema key, ensures capability ids are unique, and builds
    /// a deterministic BTreeMap for fast lookups.
    pub fn load(path: &Path) -> Result<Self> {
        let catalog =
            load_catalog_from_path(path).with_context(|| format!("loading {}", path.display()))?;
        validate_schema_version(&catalog.schema_version)?;
        validate_catalog_metadata(&catalog.catalog)?;
        let by_id = build_index(&catalog)?;
        Ok(Self {
            catalog_key: catalog.catalog.key.clone(),
            catalog,
            by_id,
        })
    }

    /// The catalog key declared in the loaded file.
    pub fn key(&self) -> &CatalogKey {
        &self.catalog_key
    }

    /// Resolve a capability by id.
    ///
    /// Returns `None` instead of erroring; callers surface errors with the CLI
    /// context that referenced the missing id.
    pub fn capability(&self, id: &CapabilityId) -> Option<&Capability> {
        self.by_id.get(id)
    }

    /// Iterates capability ids in stable order.
    pub fn ids(&self) -> impl Iterator<Item = &CapabilityId> {
        self.by_id.keys()
    }

    /// Access the underlying catalog (categories, docs, etc.).
    pub fn catalog(&self) -> &CapabilityCatalog {
        &self.catalog
    }
}

fn validate_schema_version(schema_version: &str) -> Result<()> {
    if schema_version.is_empty() {
        bail!("schema_version must not be empty");
    }

    if !schema_version
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
    {
        bail!(
            "schema_version must match ^[A-Za-z0-9_.-]+$, got {}",
            schema_version
        );
    }

    let allowed = allowed_schema_versions();
    if !allowed.contains(schema_version) {
        bail!(
            "schema_version '{}' not in allowed set {:?}",
            schema_version,
            allowed
        );
    }

    Ok(())
}

fn allowed_schema_versions() -> BTreeSet<String> {
    let mut versions: BTreeSet<String> = BTreeSet::new();
    versions.insert(DEFAULT_SCHEMA_VERSION.to_string());
    if let Ok(raw) = std::env::var(ENV_ALLOWED_SCHEMA_VERSIONS) {
        for v in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            versions.insert(v.to_string());
        }
    }
    versions
}

fn validate_catalog_metadata(meta: &CatalogMetadata) -> Result<()> {
    validate_catalog_key(&meta.key)?;
    if meta.title.trim().is_empty() {
        bail!("catalog.title must not be empty");
    }
    if meta.labels.iter().any(|label| label.trim().is_empty()) {
        bail!("catalog.labels must not contain empty entries");
    }
    Ok(())
}

fn validate_catalog_key(key: &CatalogKey) -> Result<()> {
    if key.0.is_empty() {
        bail!("catalog.key must not be empty");
    }

    if !key
        .0
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
    {
        bail!("catalog.key must match ^[A-Za-z0-9_.-]+$, got {}", key.0);
    }

    Ok(())
}

fn build_index(catalog: &CapabilityCatalog) -> Result<BTreeMap<CapabilityId, Capability>> {
    if catalog.capabilities.is_empty() {
        bail!("catalog contains no capabilities");
    }

    let mut layer_ids: BTreeSet<String> = BTreeSet::new();
    for layer in &catalog.scope.policy_layers {
        if layer.id.trim().is_empty() {
            bail!("policy_layers must not contain empty ids");
        }
        layer_ids.insert(layer.id.clone());
    }

    let category_ids: BTreeSet<String> = catalog.scope.categories.keys().cloned().collect();
    if category_ids.is_empty() {
        bail!("catalog scope must define at least one category");
    }

    let doc_keys: BTreeSet<String> = catalog.docs.keys().cloned().collect();

    let mut map = BTreeMap::new();
    for cap in &catalog.capabilities {
        if cap.id.0.trim().is_empty() {
            bail!("encountered capability with no id");
        }
        if map.contains_key(&cap.id) {
            bail!("duplicate capability id {}", cap.id.0);
        }
        if !category_ids.contains(cap.category.as_str()) {
            bail!(
                "capability {} references unknown category {}",
                cap.id.0,
                cap.category.as_str()
            );
        }
        if !layer_ids.contains(cap.layer.as_str()) {
            bail!(
                "capability {} references unknown layer {}",
                cap.id.0,
                cap.layer.as_str()
            );
        }
        for source in &cap.sources {
            if !doc_keys.contains(&source.doc) {
                bail!(
                    "capability {} references unknown doc '{}'",
                    cap.id.0,
                    source.doc
                );
            }
        }
        map.insert(cap.id.clone(), cap.clone());
    }
    Ok(map)
}
