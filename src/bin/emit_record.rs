use anyhow::{anyhow, bail, Context, Result};
use codex_fence::find_repo_root;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn main() {
    if let Err(err) = run() {
        eprintln!("{err:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args = CliArgs::parse()?;
    let repo_root = find_repo_root()?;

    let detect_stack = repo_root.join("bin/detect-stack");
    if !is_executable(&detect_stack) {
        bail!("detect-stack helper not found at {}", detect_stack.display());
    }

    let capabilities_adapter = repo_root.join("tools/capabilities_adapter.sh");
    if !is_executable(&capabilities_adapter) {
        bail!(
            "Capability adapter not found or not executable at {}",
            capabilities_adapter.display()
        );
    }

    let capabilities = load_capabilities(&capabilities_adapter)?;
    if capabilities.is_empty() {
        bail!(
            "No capability IDs returned by {}",
            capabilities_adapter.display()
        );
    }

    validate_capability_id(
        &capabilities,
        &args.primary_capability_id,
        "primary capability id",
    )?;
    let secondary_capability_ids =
        normalize_secondary_ids(&capabilities, &args.secondary_capability_ids)?;

    let capabilities_schema_version = read_capabilities_schema_version(&repo_root)?;

    let payload = match &args.payload_file {
        Some(path) => {
            if !path.is_file() {
                bail!("Payload file not found: {}", path.display());
            }
            read_json_file(path)?
        }
        None => default_payload(),
    };

    let operation_args = parse_json_string(&args.operation_args, "operation args")?;

    let stack_json = run_command_json(&detect_stack, &[&args.run_mode])
        .with_context(|| format!("Failed to execute {}", detect_stack.display()))?;

    let workspace_root = resolve_workspace_root()?;

    let result_json = json!({
        "observed_result": args.status,
        "raw_exit_code": args.raw_exit_code,
        "errno": args.errno,
        "message": args.message,
        "duration_ms": args.duration_ms,
        "error_detail": args.error_detail,
    });

    let primary_capability_snapshot =
        capability_snapshot(&capabilities, &args.primary_capability_id)?;
    let secondary_capability_snapshots =
        snapshots_for_secondary(&capabilities, &secondary_capability_ids)?;

    let record = json!({
        "schema_version": "cfbo-v1",
        "capabilities_schema_version": capabilities_schema_version,
        "stack": stack_json,
        "probe": {
            "id": args.probe_name,
            "version": args.probe_version,
            "primary_capability_id": args.primary_capability_id,
            "secondary_capability_ids": secondary_capability_ids,
        },
        "run": {
            "mode": args.run_mode,
            "workspace_root": workspace_root,
            "command": args.command,
        },
        "operation": {
            "category": args.category,
            "verb": args.verb,
            "target": args.target,
            "args": operation_args,
        },
        "result": result_json,
        "payload": payload,
        "capability_context": {
            "primary": primary_capability_snapshot,
            "secondary": secondary_capability_snapshots,
        }
    });

    println!("{}", serde_json::to_string(&record)?);
    Ok(())
}

struct CliArgs {
    run_mode: String,
    probe_name: String,
    probe_version: String,
    category: String,
    verb: String,
    target: String,
    status: String,
    errno: Option<String>,
    message: Option<String>,
    duration_ms: Option<i64>,
    raw_exit_code: Option<i64>,
    error_detail: Option<String>,
    payload_file: Option<PathBuf>,
    operation_args: String,
    primary_capability_id: String,
    secondary_capability_ids: Vec<String>,
    command: String,
}

impl CliArgs {
    fn parse() -> Result<Self> {
        let mut args = env::args().skip(1);
        let mut config = PartialArgs::default();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--run-mode" => config.run_mode = Some(next_value(&mut args, "--run-mode")?),
                "--probe-name" | "--probe-id" => {
                    config.probe_name = Some(next_value(&mut args, arg.as_str())?)
                }
                "--probe-version" => config.probe_version = Some(next_value(&mut args, "--probe-version")?),
                "--category" => config.category = Some(next_value(&mut args, "--category")?),
                "--verb" => config.verb = Some(next_value(&mut args, "--verb")?),
                "--target" => config.target = Some(next_value(&mut args, "--target")?),
                "--status" => config.status = Some(next_value(&mut args, "--status")?),
                "--errno" => config.errno = Some(next_value(&mut args, "--errno")?),
                "--message" => config.message = Some(next_value(&mut args, "--message")?),
                "--duration-ms" => {
                    config.duration_ms = Some(parse_i64(
                        next_value(&mut args, "--duration-ms")?,
                        "duration-ms",
                    )?)
                }
                "--raw-exit-code" => {
                    config.raw_exit_code = Some(parse_i64(
                        next_value(&mut args, "--raw-exit-code")?,
                        "raw-exit-code",
                    )?)
                }
                "--error-detail" => config.error_detail = Some(next_value(&mut args, "--error-detail")?),
                "--payload-file" => {
                    let value = PathBuf::from(next_value(&mut args, "--payload-file")?);
                    config.payload_file = Some(value);
                }
                "--operation-args" => config.operation_args = Some(next_value(&mut args, "--operation-args")?),
                "--primary-capability-id" => {
                    config.primary_capability_id =
                        Some(next_value(&mut args, "--primary-capability-id")?)
                }
                "--secondary-capability-id" => {
                    config.secondary_capability_ids
                        .push(next_value(&mut args, "--secondary-capability-id")?)
                }
                "--command" => config.command = Some(next_value(&mut args, "--command")?),
                "--help" | "-h" => {
                    print_usage();
                    std::process::exit(1);
                }
                other => {
                    eprintln!("Unknown flag: {other}");
                    print_usage();
                    std::process::exit(1);
                }
            }
        }

        let args = config.build()?;
        validate_status(&args.status)?;
        Ok(args)
    }
}

#[derive(Default)]
struct PartialArgs {
    run_mode: Option<String>,
    probe_name: Option<String>,
    probe_version: Option<String>,
    category: Option<String>,
    verb: Option<String>,
    target: Option<String>,
    status: Option<String>,
    errno: Option<String>,
    message: Option<String>,
    duration_ms: Option<i64>,
    raw_exit_code: Option<i64>,
    error_detail: Option<String>,
    payload_file: Option<PathBuf>,
    operation_args: Option<String>,
    primary_capability_id: Option<String>,
    secondary_capability_ids: Vec<String>,
    command: Option<String>,
}

impl PartialArgs {
    fn build(self) -> Result<CliArgs> {
        let PartialArgs {
            run_mode,
            probe_name,
            probe_version,
            category,
            verb,
            target,
            status,
            errno,
            message,
            duration_ms,
            raw_exit_code,
            error_detail,
            payload_file,
            operation_args,
            primary_capability_id,
            secondary_capability_ids,
            command,
        } = self;

        Ok(CliArgs {
            run_mode: Self::require("--run-mode", run_mode)?,
            probe_name: Self::require("--probe-name", probe_name)?,
            probe_version: Self::require("--probe-version", probe_version)?,
            category: Self::require("--category", category)?,
            verb: Self::require("--verb", verb)?,
            target: Self::require("--target", target)?,
            status: Self::require("--status", status)?,
            errno: errno.filter(not_empty),
            message: message.filter(not_empty),
            duration_ms,
            raw_exit_code,
            error_detail: error_detail.filter(not_empty),
            payload_file,
            operation_args: operation_args.unwrap_or_else(|| "{}".to_string()),
            primary_capability_id: Self::require("--primary-capability-id", primary_capability_id)?,
            secondary_capability_ids,
            command: Self::require("--command", command)?,
        })
    }

    fn require(flag: &str, value: Option<String>) -> Result<String> {
        value.ok_or_else(|| anyhow!("Missing required flag: {flag}"))
    }
}

fn next_value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String> {
    args.next()
        .ok_or_else(|| anyhow!("Missing value for {flag}"))
}

fn parse_i64(value: String, label: &str) -> Result<i64> {
    value
        .parse::<i64>()
        .with_context(|| format!("Failed to parse {label} as integer"))
}

fn validate_status(status: &str) -> Result<()> {
    match status {
        "success" | "denied" | "partial" | "error" => Ok(()),
        other => bail!(
            "Unknown status: {other} (expected success|denied|partial|error)"
        ),
    }
}

fn is_executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = fs::metadata(path) {
            return meta.permissions().mode() & 0o111 != 0;
        }
        return false;
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn load_capabilities(path: &Path) -> Result<BTreeMap<String, CapabilityRecord>> {
    let output = Command::new(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| format!("Failed to execute {}", path.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Capability adapter failed: {stderr}");
    }

    let map: BTreeMap<String, CapabilityRecord> = serde_json::from_slice(&output.stdout)
        .context("Capability adapter emitted invalid JSON")?;

    Ok(map)
}

fn normalize_secondary_ids(
    capabilities: &BTreeMap<String, CapabilityRecord>,
    raw: &[String],
) -> Result<Vec<String>> {
    let mut acc = BTreeSet::new();
    for value in raw {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }
        validate_capability_id(capabilities, trimmed, "secondary capability id")?;
        acc.insert(trimmed.to_string());
    }
    Ok(acc.into_iter().collect())
}

fn validate_capability_id(
    capabilities: &BTreeMap<String, CapabilityRecord>,
    value: &str,
    label: &str,
) -> Result<()> {
    if capabilities.contains_key(value) {
        return Ok(());
    }
    bail!(
        "Unknown {label}: {value}. Expected one of the IDs in schema/capabilities.json."
    );
}

fn read_capabilities_schema_version(repo_root: &Path) -> Result<String> {
    let path = repo_root.join("schema/capabilities.json");
    let contents = fs::read_to_string(&path)
        .with_context(|| format!("Unable to read {}", path.display()))?;
    let value: Value = serde_json::from_str(&contents)
        .with_context(|| format!("Failed to parse JSON from {}", path.display()))?;
    let Some(raw) = value.get("schema_version").and_then(|v| v.as_str()) else {
        bail!("Unable to determine capabilities schema_version from schema/capabilities.json");
    };
    if !is_valid_schema_version(raw) {
        bail!("Invalid capabilities schema_version: {raw}. Expected an alphanumeric string without spaces.");
    }
    Ok(raw.to_string())
}

fn is_valid_schema_version(value: &str) -> bool {
    value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-')
        && !value.is_empty()
}

fn read_json_file(path: &Path) -> Result<Value> {
    let data = fs::read_to_string(path)?;
    serde_json::from_str(&data).context("Payload file contained invalid JSON")
}

fn parse_json_string(raw: &str, label: &str) -> Result<Value> {
    serde_json::from_str(raw).with_context(|| format!("Invalid JSON for {label}"))
}

fn run_command_json(path: &Path, args: &[&str]) -> Result<Value> {
    let output = Command::new(path)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("{} failed: {stderr}", path.display());
    }
    serde_json::from_slice(&output.stdout).context("Failed to parse command output as JSON")
}

fn default_payload() -> Value {
    json!({
        "stdout_snippet": Value::Null,
        "stderr_snippet": Value::Null,
        "raw": {},
    })
}

#[derive(Deserialize, Clone)]
struct CapabilityRecord {
    id: String,
    category: Option<String>,
    layer: Option<String>,
}

fn capability_snapshot(
    capabilities: &BTreeMap<String, CapabilityRecord>,
    cap_id: &str,
) -> Result<Value> {
    let Some(cap) = capabilities.get(cap_id) else {
        bail!("Unable to resolve capability metadata for {cap_id}");
    };

    Ok(json!({
        "id": cap.id,
        "category": cap.category,
        "layer": cap.layer,
    }))
}

fn snapshots_for_secondary(
    capabilities: &BTreeMap<String, CapabilityRecord>,
    ids: &[String],
) -> Result<Vec<Value>> {
    let mut snapshots = Vec::new();
    for id in ids {
        snapshots.push(capability_snapshot(capabilities, id)?);
    }
    Ok(snapshots)
}

fn resolve_workspace_root() -> Result<Option<String>> {
    if let Ok(env_root) = env::var("FENCE_WORKSPACE_ROOT") {
        if !env_root.is_empty() {
            return Ok(Some(env_root));
        }
    }

    if let Ok(output) = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
    {
        if output.status.success() {
            let candidate = String::from_utf8_lossy(&output.stdout)
                .trim()
                .to_string();
            if !candidate.is_empty() {
                return Ok(Some(candidate));
            }
        }
    }

    if let Ok(pwd) = env::var("PWD") {
        if !pwd.is_empty() {
            return Ok(Some(pwd));
        }
    }

    let fallback = env::current_dir()?;
    let display = fallback.display().to_string();
    if display.is_empty() {
        return Ok(None);
    }
    Ok(Some(display))
}

fn print_usage() {
    eprintln!("{}", usage());
}

fn usage() -> &'static str {
    "Usage: emit-record --run-mode MODE --probe-name NAME --probe-version VERSION \
  --primary-capability-id CAP_ID --command COMMAND \
  --category CATEGORY --verb VERB --target TARGET --status STATUS [options]\n\nOptions:\n  --errno ERRNO\n  --message MESSAGE\n  --duration-ms MILLIS\n  --raw-exit-code CODE\n  --error-detail TEXT\n  --secondary-capability-id CAP_ID   # repeat for multiple entries\n  --payload-file PATH\n  --operation-args JSON_OBJECT\n"
}

fn not_empty(value: &String) -> bool {
    !value.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_caps() -> BTreeMap<String, CapabilityRecord> {
        let mut map = BTreeMap::new();
        map.insert(
            "cap_a".to_string(),
            CapabilityRecord {
                id: "cap_a".to_string(),
                category: Some("cat".to_string()),
                layer: Some("layer".to_string()),
            },
        );
        map.insert(
            "cap_b".to_string(),
            CapabilityRecord {
                id: "cap_b".to_string(),
                category: None,
                layer: None,
            },
        );
        map
    }

    #[test]
    fn schema_version_validation() {
        assert!(is_valid_schema_version("macOS_codex_v1"));
        assert!(is_valid_schema_version("abc-1.2"));
        assert!(!is_valid_schema_version(""));
        assert!(!is_valid_schema_version("invalid value"));
    }

    #[test]
    fn validate_status_allows_known_values() {
        for value in ["success", "denied", "partial", "error"] {
            validate_status(value).expect("status should pass");
        }
        assert!(validate_status("bogus").is_err());
    }

    #[test]
    fn normalize_secondary_deduplicates_and_trims() {
        let caps = sample_caps();
        let input = vec![
            " cap_a ".to_string(),
            "cap_b".to_string(),
            "".to_string(),
            "cap_a".to_string(),
        ];
        let normalized = normalize_secondary_ids(&caps, &input).expect("normalized");
        assert_eq!(normalized, vec!["cap_a".to_string(), "cap_b".to_string()]);
    }

    #[test]
    fn normalize_secondary_rejects_unknown() {
        let caps = sample_caps();
        let input = vec!["cap_a".to_string(), "cap_missing".to_string()];
        assert!(normalize_secondary_ids(&caps, &input).is_err());
    }
}