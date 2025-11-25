//! Runs a probe/mode matrix and streams boundary objects as NDJSON.
//!
//! This binary underpins `codex-fence --bang`: it discovers probes
//! (or honors `PROBES`/`PROBES_RAW`), selects modes (`MODES` or defaults based
//! on Codex availability), executes each probe via `fence-run`, and prints each
//! emitted JSON object on its own line.

use anyhow::{Context, Result, bail};
use codex_fence::connectors::{
    Availability, RunMode, allowed_mode_names, default_mode_names, parse_modes,
};
use codex_fence::{
    Probe, find_repo_root, list_probes, resolve_helper_binary, resolve_probe, split_list,
};
use serde_json::Value;
use std::{
    env,
    path::Path,
    process::{Command, Stdio},
};

fn main() {
    if let Err(err) = run() {
        eprintln!("{err:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let repo_root = find_repo_root()?;
    let probes = resolve_probes(&repo_root)?;
    let modes = resolve_modes()?;

    let mut errors: Vec<String> = Vec::new();
    for mode in modes {
        for probe in &probes {
            if let Err(err) = run_probe(&repo_root, probe, mode) {
                let message = format!(
                    "probe {} in mode {} failed: {err:#}",
                    probe.id,
                    mode.as_str()
                );
                eprintln!("fence-bang: {message}");
                errors.push(message);
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        bail!(
            "{} probe(s) failed; see stderr for details:\n{}",
            errors.len(),
            errors.join("\n")
        )
    }
}

fn resolve_modes() -> Result<Vec<RunMode>> {
    let requested = env::var("MODES")
        .ok()
        .and_then(|raw| {
            let parsed = split_list(&raw);
            if parsed.is_empty() {
                None
            } else {
                Some(parsed)
            }
        })
        .unwrap_or_else(|| default_mode_names(Availability::for_host()));

    if requested.is_empty() {
        bail!("No modes resolved; check MODES env var");
    }

    let allowed = allowed_mode_names();
    if let Some(bad) = requested
        .iter()
        .find(|mode| !allowed.contains(&mode.as_str()))
    {
        bail!("Unsupported mode requested: {bad}");
    }

    parse_modes(&requested)
}

fn resolve_probes(repo_root: &Path) -> Result<Vec<Probe>> {
    let requested = env::var("PROBES")
        .or_else(|_| env::var("PROBES_RAW"))
        .ok()
        .map(|raw| split_list(&raw))
        .unwrap_or_default();

    if requested.is_empty() {
        return list_probes(repo_root);
    }

    let mut probes = Vec::new();
    for raw in requested {
        probes.push(resolve_probe(repo_root, &raw)?);
    }
    Ok(probes)
}

fn run_probe(repo_root: &Path, probe: &Probe, mode: RunMode) -> Result<()> {
    let runner = resolve_helper_binary(repo_root, "fence-run")?;
    let output = Command::new(&runner)
        .arg(mode.as_str())
        .arg(&probe.path)
        .current_dir(repo_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()
        .with_context(|| format!("Failed to execute {}", runner.display()))?;

    if !output.status.success() {
        // Gracefully skip codex modes when the host blocks sandbox application.
        if mode.is_codex()
            && (output.status.code() == Some(71)
                || String::from_utf8_lossy(&output.stderr).contains("sandbox_apply"))
        {
            eprintln!(
                "fence-bang: skipping mode {} for probe {}: codex sandbox unavailable",
                mode.as_str(),
                probe.id
            );
            return Ok(());
        }
        let code = output.status.code().unwrap_or(-1);
        bail!(
            "Probe {} in mode {} returned non-zero exit code {code}",
            probe.id,
            mode.as_str()
        );
    }

    let json_value: Value = serde_json::from_slice(&output.stdout).with_context(|| {
        format!(
            "Failed to parse boundary object for probe {} in mode {}",
            probe.id,
            mode.as_str()
        )
    })?;
    let compact = serde_json::to_string(&json_value)?;
    println!("{compact}");
    Ok(())
}
