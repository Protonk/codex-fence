#![cfg(unix)]

mod common;

use anyhow::{Context, Result};
use common::{helper_binary, repo_root, run_command};
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn codex_fence_prefers_repo_helper() -> Result<()> {
    let repo_root = repo_root();
    let temp = TempDir::new().context("failed to allocate temp repo")?;
    let repo = temp.path();

    let bin_dir = repo.join("bin");
    fs::create_dir_all(&bin_dir)?;
    fs::write(bin_dir.join(".gitkeep"), "")?;
    fs::write(repo.join("Makefile"), "all:\n\t@true\n")?;

    let marker = repo.join("helper_invoked");
    let helper_path = bin_dir.join("fence-bang");
    fs::write(
        &helper_path,
        "#!/bin/sh\n[ -n \"$MARK_FILE\" ] && echo invoked > \"$MARK_FILE\"\n",
    )?;
    make_executable(&helper_path)?;

    let codex_fence = helper_binary(&repo_root, "codex-fence");
    let output = Command::new(codex_fence)
        .arg("--bang")
        .env("CODEX_FENCE_ROOT", repo)
        .env("PATH", "")
        .env("MARK_FILE", &marker)
        .output()
        .context("failed to run codex-fence stub")?;

    assert!(output.status.success());
    assert!(
        marker.is_file(),
        "stub helper should have been executed via CODEX_FENCE_ROOT"
    );
    Ok(())
}

#[test]
fn codex_fence_falls_back_to_path() -> Result<()> {
    let repo_root = repo_root();
    let temp = TempDir::new().context("failed to allocate temp dir")?;
    let helper_dir = temp.path();
    let marker = helper_dir.join("path_helper_invoked");
    let helper_path = helper_dir.join("fence-listen");
    fs::write(
        &helper_path,
        "#!/bin/sh\n[ -n \"$MARK_FILE\" ] && echo listen > \"$MARK_FILE\"\n",
    )?;
    make_executable(&helper_path)?;

    // Copy codex-fence outside the repo so it cannot discover CODEX_FENCE_ROOT.
    let source = helper_binary(&repo_root, "codex-fence");
    let runner = temp.path().join("codex-fence");
    fs::copy(&source, &runner)?;
    make_executable(&runner)?;

    let output = Command::new(&runner)
        .arg("--listen")
        .env("PATH", helper_dir)
        .env_remove("CODEX_FENCE_ROOT")
        .env("MARK_FILE", &marker)
        .current_dir(helper_dir)
        .output()
        .context("failed to run codex-fence path test")?;

    assert!(output.status.success());
    assert!(marker.is_file(), "PATH helper should have been executed");
    Ok(())
}

#[test]
fn detect_stack_reports_expected_sandbox_modes() -> Result<()> {
    let repo_root = repo_root();
    let detect_stack = helper_binary(&repo_root, "detect-stack");

    let mut baseline_cmd = Command::new(&detect_stack);
    baseline_cmd.arg("baseline");
    let baseline = run_command(baseline_cmd)?;
    let baseline_json: Value = serde_json::from_slice(&baseline.stdout)?;
    assert!(
        baseline_json
            .get("sandbox_mode")
            .map(|v| v.is_null())
            .unwrap_or(true)
    );

    let mut sandbox_cmd = Command::new(&detect_stack);
    sandbox_cmd.arg("codex-sandbox");
    let sandbox = run_command(sandbox_cmd)?;
    let sandbox_json: Value = serde_json::from_slice(&sandbox.stdout)?;
    assert_eq!(
        sandbox_json
            .get("sandbox_mode")
            .and_then(|v| v.as_str())
            .unwrap_or_default(),
        "workspace-write"
    );

    let override_val = "custom-mode";
    let mut full_cmd = Command::new(&detect_stack);
    full_cmd
        .arg("codex-full")
        .env("FENCE_SANDBOX_MODE", override_val);
    let full = run_command(full_cmd)?;
    let full_json: Value = serde_json::from_slice(&full.stdout)?;
    assert_eq!(
        full_json
            .get("sandbox_mode")
            .and_then(|v| v.as_str())
            .unwrap_or_default(),
        override_val
    );
    Ok(())
}

#[test]
fn json_extract_applies_default_value() -> Result<()> {
    let repo_root = repo_root();
    let helper = helper_binary(&repo_root, "json-extract");
    let temp = TempDir::new().context("failed to allocate json fixture dir")?;
    let json_path = temp.path().join("input.json");
    fs::write(&json_path, br#"{"present":true}"#)?;

    let mut cmd = Command::new(helper);
    cmd.arg("--file")
        .arg(&json_path)
        .arg("--pointer")
        .arg("/missing")
        .arg("--type")
        .arg("bool")
        .arg("--default")
        .arg("false");
    let output = run_command(cmd)?;
    let value: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(value, Value::Bool(false));
    Ok(())
}

#[test]
fn json_extract_rejects_unknown_type() -> Result<()> {
    let repo_root = repo_root();
    let helper = helper_binary(&repo_root, "json-extract");
    let output = Command::new(helper)
        .arg("--stdin")
        .arg("--type")
        .arg("unknown")
        .stdin(std::process::Stdio::piped())
        .output()
        .context("failed to spawn json-extract for error case")?;
    assert!(!output.status.success(), "unknown types should fail");
    Ok(())
}

#[test]
fn portable_path_relpath_matches_basics() -> Result<()> {
    let repo_root = repo_root();
    let helper = helper_binary(&repo_root, "portable-path");
    let temp = TempDir::new().context("failed to allocate temp dir")?;
    let base = temp.path().join("base");
    let target = base.join("nested/child");
    fs::create_dir_all(&target)?;

    let mut cmd = Command::new(helper);
    cmd.arg("relpath").arg(&target).arg(&base);
    let output = run_command(cmd)?;
    let relpath = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert_eq!(relpath, "nested/child");
    Ok(())
}

#[test]
fn portable_path_relpath_handles_parent() -> Result<()> {
    let repo_root = repo_root();
    let helper = helper_binary(&repo_root, "portable-path");
    let temp = TempDir::new().context("failed to allocate temp dir")?;
    let base = temp.path().join("base/child");
    let target = temp.path().join("base/sibling/file.txt");
    fs::create_dir_all(target.parent().unwrap())?;
    fs::create_dir_all(&base)?;
    fs::write(&target, "content")?;

    let mut cmd = Command::new(helper);
    cmd.arg("relpath").arg(&target).arg(&base);
    let output = run_command(cmd)?;
    let relpath = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert_eq!(relpath, "../sibling/file.txt");
    Ok(())
}

#[test]
fn portable_path_relpath_identical_path() -> Result<()> {
    let repo_root = repo_root();
    let helper = helper_binary(&repo_root, "portable-path");
    let temp = TempDir::new().context("failed to allocate temp dir")?;
    let base = temp.path().join("base");
    fs::create_dir_all(&base)?;

    let mut cmd = Command::new(helper);
    cmd.arg("relpath").arg(&base).arg(&base);
    let output = run_command(cmd)?;
    let relpath = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert_eq!(relpath, ".");
    Ok(())
}

fn make_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms)?;
    }
    Ok(())
}
