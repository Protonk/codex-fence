#![cfg(unix)]

use anyhow::{Context, Result, anyhow, bail};
use codex_fence::{BoundaryObject, find_repo_root};
use jsonschema::JSONSchema;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Mutex, MutexGuard, OnceLock};
use tempfile::{NamedTempFile, TempDir};

const TEMP_PROBE_NAMES: &[&str] = &["tests_fixture_probe"];

#[test]
fn capability_map_sync() -> Result<()> {
    let repo_root = repo_root();
    let capability_ids = load_capability_ids(&repo_root)?;
    anyhow::ensure!(
        !capability_ids.is_empty(),
        "capabilities_adapter returned no ids"
    );

    let coverage = load_coverage_map(&repo_root)?;
    let probes = load_probe_metadata(&repo_root.join("probes"))?;

    let mut errors = Vec::new();

    for (cap_id, entry) in &coverage {
        if !capability_ids.contains(cap_id) {
            errors.push(format!("coverage references unknown capability '{cap_id}'"));
        }
        if entry.raw_has_probe.is_none() {
            errors.push(format!(
                "coverage entry for '{cap_id}' missing has_probe flag"
            ));
        }
    }

    for cap in &capability_ids {
        if !coverage.contains_key(cap) {
            errors.push(format!(
                "docs/data/probe_cap_coverage_map.json missing entry for '{cap}'"
            ));
        }
    }

    let mut capability_to_probes: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut probe_to_cap: BTreeMap<String, String> = BTreeMap::new();

    for probe in &probes {
        let rel = relative_to_repo(&probe.script, &repo_root);
        let Some(name) = &probe.name else {
            errors.push(format!("{rel} is missing probe_name"));
            continue;
        };
        let Some(primary) = &probe.primary_capability else {
            errors.push(format!("{rel} is missing primary_capability_id"));
            continue;
        };
        if !capability_ids.contains(primary) {
            errors.push(format!("{rel} references unknown capability '{primary}'"));
            continue;
        }
        if TEMP_PROBE_NAMES
            .iter()
            .any(|ignored| *ignored == name.as_str())
        {
            continue;
        }

        capability_to_probes
            .entry(primary.clone())
            .or_default()
            .push(name.clone());
        probe_to_cap.insert(name.clone(), primary.clone());
    }

    for probes in capability_to_probes.values_mut() {
        probes.sort();
    }

    for (cap_id, entry) in &coverage {
        let actual = capability_to_probes
            .get(cap_id)
            .cloned()
            .unwrap_or_default();
        if entry.has_probe && actual.is_empty() {
            errors.push(format!(
                "{cap_id} marked has_probe=true but no probes declare it"
            ))
        }
        if !entry.has_probe && !actual.is_empty() {
            errors.push(format!(
                "{cap_id} marked has_probe=false but probes {:?} target it",
                actual
            ))
        }

        for listed in &entry.probe_ids {
            match probe_to_cap.get(listed) {
                None => errors.push(format!("{cap_id} lists unknown probe '{listed}'")),
                Some(actual_cap) if actual_cap != cap_id => errors.push(format!(
                    "{cap_id} lists probe '{listed}' but script targets '{actual_cap}'"
                )),
                _ => {}
            }
        }

        if !actual.is_empty() {
            for probe_name in &actual {
                if entry.probe_ids.is_empty() || !entry.probe_ids.contains(probe_name) {
                    errors.push(format!(
                        "{cap_id} missing probe '{probe_name}' in coverage list"
                    ));
                }
            }
        }
    }

    if !errors.is_empty() {
        bail!("capability map drift:\n{}", errors.join("\n"));
    }

    Ok(())
}

#[test]
fn boundary_object_schema() -> Result<()> {
    let repo_root = repo_root();
    let emit_record = repo_root.join("bin/emit-record");
    let payload = json!({
        "stdout_snippet": "fixture-stdout",
        "stderr_snippet": "fixture-stderr",
        "raw": {"detail": "schema-test"}
    });

    let mut payload_file = NamedTempFile::new().context("failed to allocate payload file")?;
    serde_json::to_writer(&mut payload_file, &payload)?;

    let mut emit_cmd = Command::new(&emit_record);
    emit_cmd
        .arg("--run-mode")
        .arg("baseline")
        .arg("--probe-name")
        .arg("schema_test_fixture")
        .arg("--probe-version")
        .arg("1")
        .arg("--primary-capability-id")
        .arg("cap_fs_read_workspace_tree")
        .arg("--command")
        .arg("printf fixture")
        .arg("--category")
        .arg("fs")
        .arg("--verb")
        .arg("read")
        .arg("--target")
        .arg("/dev/null")
        .arg("--status")
        .arg("success")
        .arg("--raw-exit-code")
        .arg("0")
        .arg("--message")
        .arg("fixture")
        .arg("--operation-args")
        .arg("{\"fixture\":true}")
        .arg("--payload-file")
        .arg(payload_file.path());
    let output = run_command(emit_cmd)?;

    let (record, value) = parse_boundary_object(&output.stdout)?;

    assert_eq!(record.schema_version, "cfbo-v1");
    assert!(value.get("capabilities_schema_version").is_some());
    if let Some(cap_schema) = value.get("capabilities_schema_version") {
        if let Some(cap_schema_str) = cap_schema.as_str() {
            assert!(
                cap_schema_str
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-')),
                "capabilities_schema_version must match ^[A-Za-z0-9_.-]+$"
            );
        } else {
            assert!(cap_schema.is_null());
        }
    }

    assert!(value.get("stack").map(|s| s.is_object()).unwrap_or(false));
    assert_eq!(record.probe.id, "schema_test_fixture");
    assert_eq!(record.probe.version, "1");
    assert_eq!(
        record.probe.primary_capability_id.0,
        "cap_fs_read_workspace_tree"
    );
    assert!(
        value
            .get("probe")
            .and_then(|probe| probe.get("secondary_capability_ids"))
            .map(|ids| ids.is_array())
            .unwrap_or(false)
    );

    assert!(matches!(
        record.run.mode.as_str(),
        "baseline" | "codex-sandbox" | "codex-full"
    ));
    assert!(record.run.workspace_root.is_some());
    assert!(
        value
            .get("run")
            .and_then(|run| run.get("command"))
            .and_then(Value::as_str)
            .is_some()
    );

    assert_eq!(record.operation.category, "fs");
    assert_eq!(record.operation.verb, "read");
    assert_eq!(record.operation.target, "/dev/null");
    assert!(
        value
            .get("operation")
            .and_then(|op| op.get("args"))
            .map(|args| args.is_object())
            .unwrap_or(false)
    );

    assert!(matches!(
        record.result.observed_result.as_str(),
        "success" | "denied" | "partial" | "error"
    ));
    let result_obj = value.get("result").expect("result present");
    for key in [
        "raw_exit_code",
        "errno",
        "message",
        "duration_ms",
        "error_detail",
    ] {
        assert!(result_obj.get(key).is_some(), "result missing {key}");
    }

    assert_eq!(
        value
            .pointer("/payload/stdout_snippet")
            .and_then(Value::as_str),
        Some("fixture-stdout")
    );
    assert_eq!(
        value
            .pointer("/payload/stderr_snippet")
            .and_then(Value::as_str),
        Some("fixture-stderr")
    );
    assert!(
        value
            .pointer("/payload/raw")
            .map(|raw| raw.is_object())
            .unwrap_or(false)
    );

    let capability_context = value
        .get("capability_context")
        .expect("capability_context present");
    assert!(capability_context.is_object());
    let primary_ctx = capability_context
        .get("primary")
        .expect("primary context present");
    assert_eq!(
        primary_ctx.get("id").and_then(Value::as_str),
        Some("cap_fs_read_workspace_tree")
    );
    for key in ["category", "layer"] {
        assert!(
            primary_ctx.get(key).is_some(),
            "primary context missing {key}"
        );
    }
    assert!(
        capability_context
            .get("secondary")
            .map(|sec| sec.is_array())
            .unwrap_or(false)
    );

    static BOUNDARY_OBJECT_SCHEMA: OnceLock<Value> = OnceLock::new();
    let schema = if let Some(existing) = BOUNDARY_OBJECT_SCHEMA.get() {
        existing
    } else {
        let schema_path = repo_root.join("schema/boundary_object.json");
        let schema_value: Value = serde_json::from_reader(File::open(&schema_path)?)?;
        BOUNDARY_OBJECT_SCHEMA.get_or_init(move || schema_value)
    };
    let compiled = JSONSchema::compile(schema)?;
    if let Err(errors) = compiled.validate(&value) {
        let details = errors
            .map(|err| err.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        bail!("boundary object failed schema validation:\n{details}");
    }

    Ok(())
}

#[test]
fn harness_smoke_probe_fixture() -> Result<()> {
    let repo_root = repo_root();
    let _guard = repo_guard();
    let fixture = FixtureProbe::install(&repo_root, "tests_fixture_probe")?;

    let mut baseline_cmd = Command::new(repo_root.join("bin/fence-run"));
    baseline_cmd.arg("baseline").arg(fixture.probe_id());
    let output = run_command(baseline_cmd)?;

    let (record, value) = parse_boundary_object(&output.stdout)?;

    assert_eq!(record.probe.id, fixture.probe_id());
    assert_eq!(record.operation.category, "fs");
    assert_eq!(record.result.observed_result, "success");
    assert_eq!(
        value.pointer("/payload/raw/probe").and_then(Value::as_str),
        Some("fixture")
    );
    assert_eq!(
        record.run.workspace_root.as_deref(),
        Some(repo_root.to_str().expect("repo root utf-8"))
    );

    Ok(())
}

#[test]
fn baseline_no_codex_smoke() -> Result<()> {
    let repo_root = repo_root();
    let _guard = repo_guard();
    let fixture = FixtureProbe::install(&repo_root, "tests_fixture_probe")?;

    let jq_path = find_in_path("jq").context("jq must be available on PATH")?;
    let temp_bin = TempDir::new().context("failed to allocate temp bin path")?;
    let jq_dest = temp_bin.path().join("jq");
    if symlink(&jq_path, &jq_dest).is_err() {
        fs::copy(&jq_path, &jq_dest)?;
    }

    let sanitized_path = sanitized_path_without_codex(temp_bin.path())?;

    let fence_run = repo_root.join("bin/fence-run");
    let mut baseline_cmd = Command::new(&fence_run);
    baseline_cmd
        .env("PATH", &sanitized_path)
        .arg("baseline")
        .arg(fixture.probe_id());
    let baseline_output = run_command(baseline_cmd)?;
    let (record, _) = parse_boundary_object(&baseline_output.stdout)?;
    assert_eq!(record.probe.id, fixture.probe_id());
    assert_eq!(record.result.observed_result, "success");

    let sandbox_result = Command::new(&fence_run)
        .env("PATH", &sanitized_path)
        .arg("codex-sandbox")
        .arg(fixture.probe_id())
        .output()
        .context("failed to execute fence-run codex-sandbox")?;
    assert!(
        !sandbox_result.status.success(),
        "codex-sandbox unexpectedly succeeded without codex (stdout: {}, stderr: {})",
        String::from_utf8_lossy(&sandbox_result.stdout),
        String::from_utf8_lossy(&sandbox_result.stderr)
    );

    Ok(())
}

#[test]
fn workspace_root_fallback() -> Result<()> {
    let repo_root = repo_root();
    let _guard = repo_guard();
    let fixture = FixtureProbe::install(&repo_root, "tests_fixture_probe")?;
    let temp_run_dir = TempDir::new()?;

    let mut fallback_cmd = Command::new(repo_root.join("bin/fence-run"));
    fallback_cmd
        .current_dir(temp_run_dir.path())
        .env("FENCE_WORKSPACE_ROOT", "")
        .arg("baseline")
        .arg(fixture.probe_id());
    let output = run_command(fallback_cmd)?;
    let (record, _) = parse_boundary_object(&output.stdout)?;
    let expected_workspace = fs::canonicalize(temp_run_dir.path())?;
    let actual_root = record
        .run
        .workspace_root
        .as_deref()
        .expect("workspace_root recorded");
    let actual_workspace = fs::canonicalize(Path::new(actual_root))?;
    assert_eq!(actual_workspace, expected_workspace);

    Ok(())
}

#[test]
fn probe_resolution_guards() -> Result<()> {
    let repo_root = repo_root();
    let _guard = repo_guard();

    let mut script = NamedTempFile::new()?;
    writeln!(script, "#!/usr/bin/env bash")?;
    writeln!(script, "echo should_never_run")?;
    writeln!(script, "exit 0")?;
    let temp_path = script.into_temp_path();
    let outside_script = temp_path.to_path_buf();
    let mut perms = fs::metadata(&outside_script)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&outside_script, perms)?;

    let abs_result = Command::new(repo_root.join("bin/fence-run"))
        .arg("baseline")
        .arg(&outside_script)
        .output()
        .context("failed to execute fence-run outside script")?;
    assert!(
        !abs_result.status.success(),
        "fence-run executed script outside probes/ (stdout: {}, stderr: {})",
        String::from_utf8_lossy(&abs_result.stdout),
        String::from_utf8_lossy(&abs_result.stderr)
    );

    let symlink_path = repo_root.join("probes/tests_probe_resolution_symlink.sh");
    if symlink_path.exists() {
        bail!(
            "symlink fixture already exists at {}",
            symlink_path.display()
        );
    }
    symlink(&outside_script, &symlink_path)?;
    let _symlink_guard = FileGuard {
        path: symlink_path.clone(),
    };

    let symlink_result = Command::new(repo_root.join("bin/fence-run"))
        .arg("baseline")
        .arg("tests_probe_resolution_symlink")
        .output()
        .context("failed to execute fence-run via symlink")?;
    assert!(
        !symlink_result.status.success(),
        "fence-run followed a symlink that escapes probes/ (stdout: {}, stderr: {})",
        String::from_utf8_lossy(&symlink_result.stdout),
        String::from_utf8_lossy(&symlink_result.stderr)
    );

    Ok(())
}

struct CoverageEntry {
    has_probe: bool,
    raw_has_probe: Option<bool>,
    probe_ids: Vec<String>,
}

struct ProbeMetadata {
    script: PathBuf,
    name: Option<String>,
    primary_capability: Option<String>,
}

struct FixtureProbe {
    path: PathBuf,
    name: String,
}

impl FixtureProbe {
    fn install(repo_root: &Path, name: &str) -> Result<Self> {
        let source = repo_root.join("tests/library/fixtures/probe_fixture.sh");
        let dest = repo_root.join("probes").join(format!("{name}.sh"));
        if dest.exists() {
            bail!("fixture already exists at {}", dest.display());
        }
        fs::copy(&source, &dest)
            .with_context(|| format!("failed to copy fixture to {}", dest.display()))?;
        let mut perms = fs::metadata(&dest)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&dest, perms)?;
        Ok(Self {
            path: dest,
            name: name.to_string(),
        })
    }

    fn probe_id(&self) -> &str {
        &self.name
    }
}

impl Drop for FixtureProbe {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

struct FileGuard {
    path: PathBuf,
}

impl Drop for FileGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

struct RepoGuard {
    _guard: MutexGuard<'static, ()>,
}

fn repo_guard() -> RepoGuard {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let mutex = LOCK.get_or_init(|| Mutex::new(()));
    let guard = mutex.lock().unwrap_or_else(|err| err.into_inner());
    RepoGuard { _guard: guard }
}

fn repo_root() -> PathBuf {
    find_repo_root().expect("tests require repository root")
}

fn load_capability_ids(repo_root: &Path) -> Result<BTreeSet<String>> {
    let adapter = repo_root.join("tools/capabilities_adapter.sh");
    let output = run_command(Command::new(&adapter))?;
    let value: Value = serde_json::from_slice(&output.stdout)?;
    let obj = value
        .as_object()
        .ok_or_else(|| anyhow!("capabilities_adapter output must be an object"))?;
    Ok(obj.keys().cloned().collect())
}

fn load_coverage_map(repo_root: &Path) -> Result<BTreeMap<String, CoverageEntry>> {
    let path = repo_root.join("docs/data/probe_cap_coverage_map.json");
    let value: Value = serde_json::from_reader(File::open(&path)?)?;
    let obj = value
        .as_object()
        .ok_or_else(|| anyhow!("coverage map must be an object"))?;
    let mut map = BTreeMap::new();
    for (key, entry) in obj {
        let has_probe = entry
            .get("has_probe")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let raw_has_probe = entry.get("has_probe").and_then(Value::as_bool);
        let probe_ids = entry
            .get("probe_ids")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        map.insert(
            key.clone(),
            CoverageEntry {
                has_probe,
                raw_has_probe,
                probe_ids,
            },
        );
    }
    Ok(map)
}

fn load_probe_metadata(probes_dir: &Path) -> Result<Vec<ProbeMetadata>> {
    fn collect(dir: &Path, acc: &mut Vec<PathBuf>) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                collect(&path, acc)?;
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("sh") {
                acc.push(path);
            }
        }
        Ok(())
    }

    let mut scripts = Vec::new();
    collect(probes_dir, &mut scripts)?;
    scripts.sort();

    let mut probes = Vec::new();
    for script in scripts {
        let contents = fs::read_to_string(&script)?;
        let name = parse_probe_assignment(&contents, "probe_name");
        let primary = parse_probe_assignment(&contents, "primary_capability_id");
        probes.push(ProbeMetadata {
            script,
            name,
            primary_capability: primary,
        });
    }
    Ok(probes)
}

fn parse_probe_assignment(contents: &str, var_name: &str) -> Option<String> {
    let prefix = var_name;
    for line in contents.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            continue;
        }
        let rest = match trimmed.strip_prefix(&prefix) {
            Some(r) => r,
            None => continue,
        };
        let rest = rest.trim_start();
        if !rest.starts_with('=') {
            continue;
        }
        let mut value = rest[1..].trim_start();
        if value.is_empty() {
            continue;
        }
        if value.starts_with('"') {
            value = &value[1..];
            if let Some(end) = value.find('"') {
                return Some(value[..end].to_string());
            }
        } else if value.starts_with('\'') {
            value = &value[1..];
            if let Some(end) = value.find('\'') {
                return Some(value[..end].to_string());
            }
        } else {
            let token = value.split_whitespace().next().unwrap_or("").trim();
            if !token.is_empty() {
                return Some(token.to_string());
            }
        }
    }
    None
}

fn parse_boundary_object(bytes: &[u8]) -> Result<(BoundaryObject, Value)> {
    let value: Value = serde_json::from_slice(bytes)?;
    let record: BoundaryObject = serde_json::from_value(value.clone())?;
    Ok((record, value))
}

fn run_command(cmd: Command) -> Result<Output> {
    let mut cmd = cmd;
    let output = cmd
        .output()
        .with_context(|| format!("failed to run command: {:?}", cmd))?;
    if output.status.success() {
        Ok(output)
    } else {
        bail!(
            "command {:?} failed: status {:?}\nstdout: {}\nstderr: {}",
            cmd,
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    }
}

fn sanitized_path_without_codex(temp_bin: &Path) -> Result<OsString> {
    let original = env::var_os("PATH").unwrap_or_default();
    let mut entries: Vec<PathBuf> = Vec::new();
    entries.push(temp_bin.to_path_buf());
    let codex_dir = find_in_path("codex").and_then(|path| path.parent().map(PathBuf::from));
    for entry in env::split_paths(&original) {
        if let Some(dir) = &codex_dir {
            if same_path(&entry, dir) {
                continue;
            }
        }
        entries.push(entry);
    }
    Ok(env::join_paths(entries)?)
}

fn same_path(a: &Path, b: &Path) -> bool {
    if let (Ok(a_real), Ok(b_real)) = (fs::canonicalize(a), fs::canonicalize(b)) {
        return a_real == b_real;
    }
    a == b
}

fn find_in_path(program: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    for entry in env::split_paths(&path) {
        let candidate = entry.join(program);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn relative_to_repo(path: &Path, repo_root: &Path) -> String {
    path.strip_prefix(repo_root)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}
