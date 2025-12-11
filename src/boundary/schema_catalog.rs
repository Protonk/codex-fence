use crate::catalog::DocRef;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Version marker for boundary-object schema descriptor files.
pub const BOUNDARY_SCHEMA_CATALOG_VERSION: &str = "boundary_object_schema_v1";

#[derive(Clone, Debug, Deserialize)]
/// Descriptor for a boundary-object schema, stored under `catalogs/`.
pub struct BoundarySchemaCatalog {
    pub schema_version: String,
    pub schema: BoundarySchemaDescriptor,
    #[serde(default)]
    pub docs: BTreeMap<String, DocRef>,
}

#[derive(Clone, Debug, Deserialize)]
/// Metadata for a single boundary-object schema.
pub struct BoundarySchemaDescriptor {
    pub key: String,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub notes: Option<String>,
    pub schema_path: String,
}

impl BoundarySchemaCatalog {
    /// Parse a boundary-object schema descriptor from disk and verify its version marker.
    pub fn load(path: &Path) -> Result<Self> {
        let data = fs::read_to_string(path)
            .with_context(|| format!("reading boundary schema catalog {}", path.display()))?;
        let catalog: BoundarySchemaCatalog = serde_json::from_str(&data)
            .with_context(|| format!("parsing boundary schema catalog {}", path.display()))?;

        if catalog.schema_version != BOUNDARY_SCHEMA_CATALOG_VERSION {
            bail!(
                "unsupported boundary schema catalog version '{}', expected {}",
                catalog.schema_version,
                BOUNDARY_SCHEMA_CATALOG_VERSION
            );
        }
        Ok(catalog)
    }
}

impl BoundarySchemaDescriptor {
    /// Resolve the schema path relative to the repository root when needed.
    pub fn schema_path(&self, repo_root: &Path) -> PathBuf {
        let candidate = Path::new(&self.schema_path);
        if candidate.is_absolute() {
            candidate.to_path_buf()
        } else {
            repo_root.join(candidate)
        }
    }
}
