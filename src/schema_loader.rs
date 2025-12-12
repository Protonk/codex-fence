//! Shared JSON Schema loader with optional descriptor/canonical enforcement.
//!
//! Callers can validate descriptor wrappers (inline or via `schema_path`),
//! enforce canonical copies, patch `schema_version` consts, and compile a
//! JSONSchema validator from the resulting schema payload.

use anyhow::{Context, Result, anyhow, bail};
use jsonschema::JSONSchema;
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

/// Result of loading and compiling a JSON Schema.
pub(crate) struct SchemaLoadResult {
    pub compiled: JSONSchema,
}

/// Controls how schemas are loaded and normalized before compilation.
pub(crate) struct SchemaLoadOptions<'a> {
    /// Optional descriptor contract path; when present, descriptors are
    /// validated and unwrapped. Validation failures are fatal.
    pub descriptor_schema_path: Option<&'a Path>,
    /// Allowed schema_version values; enforced when present.
    pub allowed_versions: Option<&'a BTreeSet<String>>,
}

impl<'a> Default for SchemaLoadOptions<'a> {
    fn default() -> Self {
        Self {
            descriptor_schema_path: None,
            allowed_versions: None,
        }
    }
}

pub(crate) fn load_json_schema(
    path: &Path,
    options: SchemaLoadOptions<'_>,
) -> Result<SchemaLoadResult> {
    let descriptor_or_schema: Value = serde_json::from_reader(
        File::open(path).with_context(|| format!("opening schema {}", path.display()))?,
    )
    .with_context(|| format!("parsing schema {}", path.display()))?;

    let mut schema_value = descriptor_or_schema.clone();
    let descriptor_has_schema = descriptor_or_schema.get("schema").is_some()
        || descriptor_or_schema.get("schema_path").is_some();

    if let Some(descriptor_schema_path) = options.descriptor_schema_path {
        if descriptor_has_schema {
            let descriptor_contract: Value =
                serde_json::from_reader(File::open(descriptor_schema_path).with_context(|| {
                    format!(
                        "opening descriptor contract {}",
                        descriptor_schema_path.display()
                    )
                })?)
                .with_context(|| {
                    format!(
                        "parsing descriptor contract {}",
                        descriptor_schema_path.display()
                    )
                })?;
            let contract_arc = Arc::new(descriptor_contract);
            let contract_static: &'static Value = unsafe { &*(Arc::as_ptr(&contract_arc)) };
            let compiled_descriptor = JSONSchema::compile(contract_static).with_context(|| {
                format!(
                    "compiling descriptor contract {}",
                    descriptor_schema_path.display()
                )
            })?;
            if let Err(errors) = compiled_descriptor.validate(&descriptor_or_schema) {
                let details = errors
                    .map(|err| err.to_string())
                    .collect::<Vec<_>>()
                    .join("\n");
                bail!(
                    "schema descriptor {} failed validation:\n{}",
                    path.display(),
                    details
                );
            }
        }
    }

    if descriptor_has_schema {
        if let Some(schema_path) = descriptor_or_schema
            .get("schema_path")
            .and_then(Value::as_str)
        {
            let resolved = if Path::new(schema_path).is_absolute() {
                Path::new(schema_path).to_path_buf()
            } else if let Some(base) = path.parent() {
                base.join(schema_path)
            } else {
                Path::new(schema_path).to_path_buf()
            };
            let nested_path = resolved
                .canonicalize()
                .unwrap_or_else(|_| resolved.to_path_buf());
            let schema_file = File::open(&nested_path).with_context(|| {
                format!(
                    "opening schema {} referenced by {}",
                    nested_path.display(),
                    path.display()
                )
            })?;
            schema_value = serde_json::from_reader(schema_file).with_context(|| {
                format!(
                    "parsing schema {} referenced by {}",
                    nested_path.display(),
                    path.display()
                )
            })?;
        } else if let Some(inline_schema) = descriptor_or_schema.get("schema") {
            schema_value = inline_schema.clone();
        } else {
            bail!(
                "schema descriptor {} missing 'schema' or 'schema_path' field",
                path.display()
            );
        }
    }

    let schema_version = extract_schema_version(&schema_value, "/properties/schema_version/const")
        .ok_or_else(|| anyhow!("schema missing schema_version const"))?;

    if let Some(allowed) = options.allowed_versions {
        if !allowed.contains(&schema_version) {
            bail!(
                "schema_version '{}' not in allowed set {:?}",
                schema_version,
                allowed
            );
        }
    }

    let raw = Arc::new(schema_value);
    let raw_static: &'static Value = unsafe { &*(Arc::as_ptr(&raw)) };
    let compiled = JSONSchema::compile(raw_static)
        .with_context(|| format!("compiling schema {}", path.display()))?;

    Ok(SchemaLoadResult { compiled })
}

fn extract_schema_version(schema: &Value, pointer: &str) -> Option<String> {
    let version = schema.pointer(pointer).and_then(Value::as_str)?;
    if version
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
    {
        Some(version.to_string())
    } else {
        None
    }
}
