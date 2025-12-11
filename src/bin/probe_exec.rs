//! Executes a probe in the requested run mode while enforcing workspace rules.
//!
//! Responsibilities:
//! - resolve probes strictly within `probes/`
//! - export probe-facing environment expected by probe scripts and `emit-record`
//! - wrap external CLI sandbox/full invocations (Codex by default) when requested
//! - perform external preflight checks and emit preflight boundary records when
//!   sandbox tmpdir setup fails
//! - honor workspace overrides without silently falling back to host defaults

use anyhow::{Context, Result, bail};
use fencerunner::connectors::{
    CommandSpec, PreflightPlan, RunMode, plan_for_mode, sandbox_override_from_env,
};
use fencerunner::fence_run_support::{
    ResolvedProbeMetadata, WorkspaceOverride, WorkspacePlan, canonicalize_path,
    classify_preflight_error, resolve_probe_metadata, workspace_plan_from_override,
    workspace_tmpdir_plan,
};
use fencerunner::{
    ProbeMetadata, external_cli_command, find_repo_root, resolve_boundary_schema_path,
    resolve_catalog_path, resolve_probe,
};
use serde_json::json;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::SystemTime;
use tempfile::NamedTempFile;

fn main() {
    if let Err(err) = run() {
        eprintln!("{err:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args = CliArgs::parse()?;
    let repo_root = find_repo_root()?;
    let catalog_path = resolve_catalog_path(&repo_root, args.catalog_path.as_deref());
    let boundary_schema_path =
        resolve_boundary_schema_path(&repo_root, args.boundary_schema_path.as_deref())?;
    let workspace_root = canonicalize_path(&repo_root);
    let workspace_plan = determine_workspace_plan(&workspace_root, args.workspace_override)?;
    let resolved_probe = resolve_probe(&workspace_root, &args.probe_name)?;
    let parsed_metadata = ProbeMetadata::from_script(&resolved_probe.path)?;
    let resolved_metadata = resolve_probe_metadata(&resolved_probe, parsed_metadata)?;
    ensure_probe_executable(&resolved_probe.path)?;
    let workspace_tmpdir = workspace_tmpdir_plan(&workspace_plan, &workspace_root);
    let command_cwd = command_cwd_for(&workspace_plan, &workspace_root);

    // Allow callers to override the sandbox stack value via env (for tests or
    // external runners) while keeping the default aligned with the chosen
    // RunMode (see `connectors::RunMode::sandbox_env`).
    let sandbox_override = sandbox_override_from_env();
    let platform = detect_platform().unwrap_or_else(|| env::consts::OS.to_string());
    let mode_plan = plan_for_mode(
        &args.run_mode,
        &platform,
        &resolved_probe.path,
        sandbox_override,
    )?;

    if mode_plan.run_mode.is_external() {
        if let Some(tmpdir) = workspace_tmpdir.path.as_ref() {
            if let Some(preflight) = mode_plan.preflight.as_ref() {
                if run_preflight(
                    preflight,
                    &repo_root,
                    &mode_plan.run_mode,
                    tmpdir,
                    &resolved_metadata,
                    &catalog_path,
                    &boundary_schema_path,
                )? {
                    // Preflight emitted a denial record; skip running the probe.
                    return Ok(());
                }
            }
        } else if let Some((attempted, message)) = workspace_tmpdir.last_error.as_ref() {
            let command_str = format!("mkdir -p {}", attempted.display());
            emit_preflight_record(
                &repo_root,
                &resolved_metadata,
                mode_plan.run_mode.as_str(),
                attempted,
                1,
                message,
                &command_str,
                &catalog_path,
                &boundary_schema_path,
            )?;
            return Ok(());
        }
    }

    run_command(
        mode_plan.command,
        &mode_plan.run_mode,
        &mode_plan.sandbox_env,
        &workspace_plan,
        workspace_tmpdir.path.as_deref(),
        &command_cwd,
        &catalog_path,
        &boundary_schema_path,
    )?;
    Ok(())
}

struct CliArgs {
    workspace_override: Option<WorkspaceOverride>,
    catalog_path: Option<PathBuf>,
    boundary_schema_path: Option<PathBuf>,
    run_mode: String,
    probe_name: String,
}

impl CliArgs {
    fn parse() -> Result<Self> {
        let mut args_iter = env::args().skip(1);
        let mut workspace_override = None;
        let mut catalog_path = None;
        let mut boundary_schema_path = None;
        let mut positionals = Vec::new();

        while let Some(arg) = args_iter.next() {
            if arg.starts_with("--workspace-root=") {
                let value = arg.split_once('=').map(|(_, v)| v).unwrap_or("");
                workspace_override = Some(parse_workspace_override(value));
                continue;
            }
            if arg.starts_with("--catalog=") {
                let value = arg.split_once('=').map(|(_, v)| v).unwrap_or("");
                catalog_path = Some(PathBuf::from(value));
                continue;
            }
            if arg.starts_with("--boundary-schema=") {
                let value = arg.split_once('=').map(|(_, v)| v).unwrap_or("");
                boundary_schema_path = Some(PathBuf::from(value));
                continue;
            }

            match arg.as_str() {
                "--workspace-root" => {
                    let value = args_iter.next().unwrap_or_else(|| {
                        eprintln!("Missing path for --workspace-root");
                        usage();
                    });
                    workspace_override = Some(parse_workspace_override(&value));
                }
                "--catalog" => {
                    let value = args_iter.next().unwrap_or_else(|| {
                        eprintln!("Missing path for --catalog");
                        usage();
                    });
                    catalog_path = Some(PathBuf::from(value));
                }
                "--boundary-schema" => {
                    let value = args_iter.next().unwrap_or_else(|| {
                        eprintln!("Missing path for --boundary-schema");
                        usage();
                    });
                    boundary_schema_path = Some(PathBuf::from(value));
                }
                "-h" | "--help" => usage(),
                _ if arg.starts_with("--") => {
                    eprintln!("Unknown option: {arg}");
                    usage();
                }
                _ => {
                    positionals.push(arg);
                    positionals.extend(args_iter);
                    break;
                }
            }
        }

        if positionals.len() != 2 {
            usage();
        }

        Ok(Self {
            workspace_override,
            catalog_path,
            boundary_schema_path,
            run_mode: positionals[0].clone(),
            probe_name: positionals[1].clone(),
        })
    }
}

fn usage() -> ! {
    eprintln!(
        "Usage: probe-exec [--workspace-root PATH] [--catalog PATH] [--boundary-schema PATH] MODE PROBE_NAME\n\nOverrides:\n  --workspace-root PATH     Export PATH via FENCE_WORKSPACE_ROOT (defaults to repo root).\n                            Pass an empty string to defer to emit-record's git/pwd fallback.\n  --catalog PATH            Override capability catalog path (or set FENCE_CATALOG_PATH).\n  --boundary-schema PATH    Override boundary-object schema path (or set FENCE_BOUNDARY_SCHEMA_PATH; default descriptor via FENCE_BOUNDARY_SCHEMA_CATALOG_PATH).\n\nEnvironment:\n  FENCE_WORKSPACE_ROOT      When set, takes precedence over the default repo root export."
    );
    std::process::exit(1);
}

fn parse_workspace_override(value: &str) -> WorkspaceOverride {
    if value.is_empty() {
        WorkspaceOverride::SkipExport
    } else {
        WorkspaceOverride::UsePath(OsString::from(value))
    }
}

fn determine_workspace_plan(
    default_root: &Path,
    cli_override: Option<WorkspaceOverride>,
) -> Result<WorkspacePlan> {
    // CLI override wins; otherwise honor FENCE_WORKSPACE_ROOT if set, and only
    // then fall back to the repo root.
    if let Some(override_value) = cli_override {
        return Ok(workspace_plan_from_override(override_value));
    }

    let env_override = ["FENCE_WORKSPACE_ROOT"]
        .iter()
        .find_map(|key| match env::var_os(key) {
            Some(value) if value.is_empty() => Some(WorkspaceOverride::SkipExport),
            Some(value) => Some(WorkspaceOverride::UsePath(value)),
            None => None,
        });

    if let Some(value) = env_override {
        return Ok(workspace_plan_from_override(value));
    }

    Ok(WorkspacePlan {
        export_value: Some(default_root.as_os_str().to_os_string()),
    })
}

/// Pick the working directory for probe execution. Prefer the exported workspace
/// root so external sandbox profiles align with the trusted tree, otherwise fall
/// back to the repository root.
fn command_cwd_for(plan: &WorkspacePlan, default_root: &Path) -> PathBuf {
    if let Some(value) = plan.export_value.as_ref() {
        return PathBuf::from(value);
    }
    env::current_dir().unwrap_or_else(|_| default_root.to_path_buf())
}

fn ensure_probe_executable(path: &Path) -> Result<()> {
    let metadata = fs::metadata(path)
        .with_context(|| format!("Probe not found or not executable: {}", path.display()))?;
    if !metadata.is_file() {
        bail!("Probe not found or not executable: {}", path.display());
    }
    if !has_execute_bit(&metadata) {
        bail!("Probe is not executable: {}", path.display());
    }
    Ok(())
}

fn has_execute_bit(metadata: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        metadata.is_file()
    }
}

fn detect_platform() -> Option<String> {
    let output = Command::new("uname")
        .arg("-s")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if value.is_empty() { None } else { Some(value) }
}

fn run_command(
    spec: CommandSpec,
    run_mode: &RunMode,
    sandbox_mode: &OsString,
    workspace_plan: &WorkspacePlan,
    workspace_tmpdir: Option<&Path>,
    command_cwd: &Path,
    catalog_path: &Path,
    boundary_schema_path: &Path,
) -> Result<()> {
    let mut command = Command::new(&spec.program);
    for arg in &spec.args {
        command.arg(arg);
    }
    command.current_dir(command_cwd);
    command.env("FENCE_RUN_MODE", run_mode.as_str());
    command.env("FENCE_SANDBOX_MODE", sandbox_mode);
    command.env("FENCE_CATALOG_PATH", catalog_path);
    command.env("FENCE_BOUNDARY_SCHEMA_PATH", boundary_schema_path);
    if let Some(value) = workspace_plan.export_value.as_ref() {
        command.env("FENCE_WORKSPACE_ROOT", value);
    }
    if let Some(tmpdir) = workspace_tmpdir {
        command.env("TMPDIR", tmpdir);
    }

    let status = command
        .status()
        .with_context(|| format!("Failed to execute {}", spec.program.to_string_lossy()))?;
    if !status.success() {
        if let Some(code) = status.code() {
            std::process::exit(code);
        } else {
            bail!("Probe terminated by signal");
        }
    }
    Ok(())
}

fn write_temp_payload(value: &serde_json::Value) -> Result<PathBuf> {
    let mut file = NamedTempFile::new().context("create payload temp file")?;
    serde_json::to_writer(&mut file, value)?;
    let (_file, path) = file.keep().context("persist payload temp file")?;
    Ok(path)
}

fn emit_preflight_record(
    repo_root: &Path,
    metadata: &ResolvedProbeMetadata,
    run_mode: &str,
    target_path: &Path,
    exit_code: i32,
    stderr: &str,
    command_str: &str,
    catalog_path: &Path,
    boundary_schema_path: &Path,
) -> Result<()> {
    let emit_record = fencerunner::resolve_helper_binary(repo_root, "emit-record")?;
    let (status, errno, message) = classify_preflight_error(stderr);

    let payload = json!({
        "stdout_snippet": "",
        "stderr_snippet": stderr,
        "raw": {
            "preflight_target": target_path.to_string_lossy(),
            "preflight_kind": "external_tmp",
            "stderr": stderr,
            "exit_code": exit_code
        }
    });

    let operation_args = json!({
        "preflight": true,
        "target_path": target_path.to_string_lossy(),
        "run_mode": run_mode
    });

    let payload_file = write_temp_payload(&payload)?;

    let mut cmd = Command::new(&emit_record);
    cmd.arg("--run-mode")
        .arg(run_mode)
        .arg("--probe-name")
        .arg(&metadata.id)
        .arg("--probe-version")
        .arg(&metadata.version)
        .arg("--primary-capability-id")
        .arg(&metadata.primary_capability.0)
        .arg("--command")
        .arg(command_str)
        .arg("--category")
        .arg("preflight")
        .arg("--verb")
        .arg("mktemp")
        .arg("--target")
        .arg(target_path.to_string_lossy().to_string())
        .arg("--status")
        .arg(status)
        .arg("--message")
        .arg(&message)
        .arg("--raw-exit-code")
        .arg(exit_code.to_string())
        .arg("--operation-args")
        .arg(operation_args.to_string())
        .arg("--payload-file")
        .arg(payload_file)
        .env("FENCE_ROOT", repo_root)
        .env("FENCE_CATALOG_PATH", catalog_path)
        .env("FENCE_BOUNDARY_SCHEMA_PATH", boundary_schema_path);

    if let Some(errno_val) = errno {
        cmd.arg("--errno").arg(errno_val);
    } else {
        cmd.arg("--errno").arg("");
    }

    let status_out = cmd.status().context("failed to emit preflight record")?;
    if !status_out.success() {
        bail!(
            "emit-record failed for preflight (exit {:?})",
            status_out.code()
        );
    }

    Ok(())
}

fn run_preflight(
    plan: &PreflightPlan,
    repo_root: &Path,
    run_mode: &RunMode,
    workspace_tmpdir: &Path,
    metadata: &ResolvedProbeMetadata,
    catalog_path: &Path,
    boundary_schema_path: &Path,
) -> Result<bool> {
    match plan {
        PreflightPlan::ExternalTmp { platform_target } => run_external_preflight(
            repo_root,
            run_mode,
            platform_target,
            workspace_tmpdir,
            metadata,
            catalog_path,
            boundary_schema_path,
        ),
    }
}

fn run_external_preflight(
    repo_root: &Path,
    run_mode: &RunMode,
    platform_target: &str,
    workspace_tmpdir: &Path,
    metadata: &ResolvedProbeMetadata,
    catalog_path: &Path,
    boundary_schema_path: &Path,
) -> Result<bool> {
    // Detect hosts that block external sandbox-managed writes before invoking
    // the probe.
    // When blocked, emit a boundary object describing the denial so matrix runs
    // keep producing output for the affected mode.
    let suffix = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros();
    let target = workspace_tmpdir.join(format!("probe-preflight-{suffix}"));

    let mut args: Vec<OsString> = Vec::new();
    if matches!(run_mode, RunMode::CodexSandbox) {
        args.push(OsString::from("sandbox"));
        args.push(OsString::from(platform_target));
        args.push(OsString::from("--full-auto"));
        args.push(OsString::from("--"));
        args.push(OsString::from("/bin/mkdir"));
        args.push(OsString::from(target.as_os_str()));
    } else {
        return Ok(false);
    }

    let cli = external_cli_command();
    let command_str = format!(
        "{} {}",
        cli.to_string_lossy(),
        args.iter()
            .map(|arg| arg.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ")
    );

    let mut cmd = Command::new(&cli);
    cmd.args(&args);
    cmd.current_dir(workspace_tmpdir);
    let output = cmd
        .output()
        .context("external sandbox preflight failed to spawn")?;

    if output.status.success() {
        // Clean up the directory if we successfully created it.
        let _ = fs::remove_dir_all(&target);
        return Ok(false);
    }

    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);
    emit_preflight_record(
        repo_root,
        metadata,
        run_mode.as_str(),
        &target,
        code,
        &stderr,
        &command_str,
        catalog_path,
        boundary_schema_path,
    )?;
    Ok(true)
}
