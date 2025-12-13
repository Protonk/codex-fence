#![cfg(unix)]

// Catalog repository and capability lookup guard rails.
mod support;
#[path = "support/common.rs"]
mod common;

use anyhow::Result;
use fencerunner::{CapabilityCategory, CapabilityIndex, CapabilityLayer, CatalogRepository, load_catalog_from_path};
use serde_json::json;
use tempfile::NamedTempFile;

use common::{catalog_path, sample_boundary_object};

#[test]
fn repository_lookup_context_matches_capabilities() -> Result<()> {
    let catalog = load_catalog_from_path(&catalog_path())?;
    let key = catalog.catalog.key.clone();
    let primary = catalog.capabilities.first().expect("cap present");
    let secondary = catalog
        .capabilities
        .get(1)
        .map(|cap| vec![cap])
        .unwrap_or_default();
    let primary_id = primary.id.clone();
    let secondary_ids: Vec<_> = secondary.iter().map(|cap| cap.id.clone()).collect();
    let bo = sample_boundary_object().with_capabilities(key.clone(), primary, &secondary);
    let mut repo = CatalogRepository::default();
    repo.register(catalog);

    let (resolved_primary, resolved_secondary) = repo.lookup_context(&bo).expect("context");
    assert_eq!(resolved_primary.id, primary_id);
    if let Some(expected_secondary) = secondary_ids.first() {
        assert_eq!(resolved_secondary.first().unwrap().id, *expected_secondary);
    }
    Ok(())
}

#[test]
fn load_real_catalog_smoke() -> Result<()> {
    let catalog = load_catalog_from_path(&catalog_path())?;
    assert!(!catalog.catalog.key.0.is_empty());
    assert!(!catalog.capabilities.is_empty());
    for cap in catalog.capabilities {
        assert!(!cap.id.0.is_empty());
        assert!(
            !matches!(cap.category, CapabilityCategory::Other(ref v) if v.is_empty()),
            "category should not be empty"
        );
        assert!(
            !matches!(cap.layer, CapabilityLayer::Other(ref v) if v.is_empty()),
            "layer should not be empty"
        );
    }
    Ok(())
}

#[test]
fn finds_capability_in_registered_catalog() -> Result<()> {
    let catalog = load_catalog_from_path(&catalog_path())?;
    let key = catalog.catalog.key.clone();
    let known_capability = catalog
        .capabilities
        .first()
        .expect("catalog should have capabilities")
        .id
        .clone();

    let mut repo = CatalogRepository::default();
    repo.register(catalog);

    let resolved = repo.find_capability(&key, &known_capability);
    assert!(resolved.is_some());
    Ok(())
}

#[test]
fn capability_index_enforces_schema_version() -> Result<()> {
    let mut file = NamedTempFile::new()?;
    serde_json::to_writer(
        &mut file,
        &json!({
            "schema_version": "unexpected",
            "scope": {"description": "test", "policy_layers": [], "categories": {}},
            "docs": {},
            "capabilities": []
        }),
    )?;
    assert!(CapabilityIndex::load(file.path()).is_err());
    Ok(())
}

#[test]
fn capability_index_accepts_allowed_schema_version_override() -> Result<()> {
    // Custom schema versions are no longer allowed; ensure rejection path is covered.
    let mut temp = NamedTempFile::new()?;
    serde_json::to_writer(
        &mut temp,
        &json!({
            "schema_version": "custom_catalog_v1",
            "catalog": {"key": "custom_catalog_v1", "title": "custom catalog"},
            "scope": {
                "description": "test",
                "policy_layers": [{"id": "os_sandbox", "description": "fixture layer"}],
                "categories": {"filesystem": "fixture"}
            },
            "docs": {},
            "capabilities": [{
                "id": "cap_fs_custom",
                "category": "filesystem",
                "layer": "os_sandbox",
                "description": "cap fs",
                "operations": {"allow": [], "deny": []}
            }]
        }),
    )?;

    assert!(
        CapabilityIndex::load(temp.path()).is_err(),
        "custom catalog schema_version should be rejected"
    );
    Ok(())
}
