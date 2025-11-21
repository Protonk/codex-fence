use anyhow::{anyhow, Context, Result, bail};
use codex_fence::{
    build_probe_coverage_map, collect_probe_scripts, filter_coverage_probes, find_repo_root,
    resolve_helper_binary, validate_boundary_objects, validate_probe_capabilities, CapabilityIndex,
    ProbeMetadata,
};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    if let Err(err) = run() {
        eprintln!("{err:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse()?;
    let repo_root = match find_repo_root() {
        Ok(path) => Some(path),
        Err(err) => {
            if cli.command.requires_repo_root() {
                return Err(err);
            }
            None
        }
    };

    match cli.command {
        CommandTarget::CoverageMap => {
            let root = require_repo_root(repo_root.as_deref())?;
            run_coverage_map(root, &cli.trailing_args)
        }
        CommandTarget::ValidateCapabilities => {
            let root = require_repo_root(repo_root.as_deref())?;
            run_capability_validation(root, &cli.trailing_args)
        }
        _ => run_helper(&cli, repo_root.as_deref()),
    }
}

struct Cli {
    command: CommandTarget,
    trailing_args: Vec<OsString>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CommandTarget {
    Bang,
    Listen,
    Test,
    CoverageMap,
    ValidateCapabilities,
}

impl CommandTarget {
    fn helper_name(self) -> &'static str {
        match self {
            CommandTarget::Bang => "fence-bang",
            CommandTarget::Listen => "fence-listen",
            CommandTarget::Test => "fence-test",
            CommandTarget::CoverageMap | CommandTarget::ValidateCapabilities => "",
        }
    }

    fn requires_repo_root(self) -> bool {
        matches!(
            self,
            CommandTarget::Test | CommandTarget::CoverageMap | CommandTarget::ValidateCapabilities
        )
    }
}

impl Cli {
    fn parse() -> Result<Self> {
        let mut args = env::args_os();
        let _program = args.next();

        let Some(flag) = args.next() else {
            usage(1);
        };

        let flag_str = flag
            .to_str()
            .with_context(|| "Invalid UTF-8 in command flag")?;

        let command = match flag_str {
            "--bang" | "-b" => CommandTarget::Bang,
            "--listen" | "-l" => CommandTarget::Listen,
            "--test" | "-t" => CommandTarget::Test,
            "--coverage-map" => CommandTarget::CoverageMap,
            "--validate-capabilities" => CommandTarget::ValidateCapabilities,
            "--help" | "-h" => usage(0),
            _ => usage(1),
        };

        let trailing_args = args.collect();
        Ok(Self {
            command,
            trailing_args,
        })
    }
}

fn usage(code: i32) -> ! {
    eprintln!(
        "Usage: codex-fence (--bang | --listen | --test | --coverage-map [OUTPUT] | --validate-capabilities) [args]\n\nCommands:\n  --bang, -b                 Run the probe matrix and emit cfbo-v1 records to stdout (NDJSON).\n  --listen, -l               Read cfbo-v1 JSON from stdin and print a human summary.\n  --test, -t                 Run the static probe contract across every probes/*.sh script.\n  --coverage-map [OUTPUT]    Emit the capability→probe coverage map to stdout or to OUTPUT.\n  --validate-capabilities    Validate that probes and generated outputs reference known capability IDs.\n\nExample:\n  codex-fence --bang | codex-fence --listen"
    );
    std::process::exit(code);
}

fn resolve_helper(name: &str, repo_root: Option<&Path>) -> Result<PathBuf> {
    if let Some(root) = repo_root {
        if let Ok(path) = resolve_helper_binary(root, name) {
            return Ok(path);
        }
    }

    if let Ok(current_exe) = env::current_exe() {
        if let Some(dir) = current_exe.parent() {
            let candidate = dir.join(name);
            if is_executable(&candidate) {
                return Ok(candidate);
            }
        }
    }

    if let Some(path) = find_on_path(name) {
        return Ok(path);
    }

    bail!(
        "Unable to locate helper '{name}'. Run 'make build-bin' (or tools/sync_bin_helpers.sh) or set CODEX_FENCE_ROOT."
    )
}

fn run_helper(cli: &Cli, repo_root: Option<&Path>) -> Result<()> {
    let helper_path = resolve_helper(cli.command.helper_name(), repo_root)?;
    let mut command = Command::new(&helper_path);
    command.args(&cli.trailing_args);

    if let Some(root) = repo_root {
        if env::var_os("CODEX_FENCE_ROOT").is_none() {
            command.env("CODEX_FENCE_ROOT", root);
        }
    }

    let status = command
        .status()
        .with_context(|| format!("Failed to execute {}", helper_path.display()))?;

    if status.success() {
        return Ok(());
    }

    if let Some(code) = status.code() {
        std::process::exit(code);
    }

    bail!("Helper terminated by signal")
}

fn run_coverage_map(repo_root: &Path, args: &[OsString]) -> Result<()> {
    if args.len() > 1 {
        bail!("--coverage-map accepts at most one optional output path");
    }

    let output_path = args.first().map(PathBuf::from);
    let capability_index = CapabilityIndex::load(&repo_root.join("schema/capabilities.json"))
        .context("loading capability catalog")?;
    let probes = load_probe_metadata_from_dirs(&[repo_root.join("probes")])?;
    let coverage_probes = filter_coverage_probes(&probes);
    let coverage = build_probe_coverage_map(&capability_index, &coverage_probes)?;
    let json = serde_json::to_string_pretty(&coverage)?;

    if let Some(path) = output_path {
        fs::write(&path, json).with_context(|| format!("writing {}", path.display()))?;
        eprintln!("coverage map written to {}", path.display());
    } else {
        println!("{json}");
    }

    Ok(())
}

fn run_capability_validation(repo_root: &Path, args: &[OsString]) -> Result<()> {
    if !args.is_empty() {
        bail!("--validate-capabilities does not accept additional arguments");
    }

    let capability_index = CapabilityIndex::load(&repo_root.join("schema/capabilities.json"))
        .context("loading capability catalog")?;
    let probes = load_probe_metadata_from_dirs(&[
        repo_root.join("probes"),
        repo_root.join("tests/library/fixtures"),
    ])?;

    let mut errors = Vec::new();
    errors.extend(validate_probe_capabilities(&capability_index, &probes));
    errors.extend(validate_boundary_objects(
        &capability_index,
        &[repo_root.join("out")],
    )?);

    if errors.is_empty() {
        println!("validate_capabilities: PASS");
        return Ok(());
    }

    eprintln!("validate_capabilities: FAIL");
    for err in &errors {
        eprintln!("  - {err}");
    }
    bail!("capability validation failed");
}

fn load_probe_metadata_from_dirs(dirs: &[PathBuf]) -> Result<Vec<ProbeMetadata>> {
    let scripts = collect_probe_scripts(dirs)?;
    let mut probes = Vec::new();
    for script in scripts {
        probes.push(ProbeMetadata::from_script(&script)?);
    }
    Ok(probes)
}

fn require_repo_root<'a>(repo_root: Option<&'a Path>) -> Result<&'a Path> {
    repo_root.ok_or_else(|| {
        anyhow!(
            "Unable to locate codex-fence repository root. Set CODEX_FENCE_ROOT to the cloned repository."
        )
    })
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    let paths = env::var_os("PATH")?;
    for dir in env::split_paths(&paths) {
        let candidate = dir.join(name);
        if is_executable(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn is_executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = fs::metadata(path) {
            return metadata.permissions().mode() & 0o111 != 0;
        }
        false
    }
    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn finds_helper_in_repo_root() {
        let temp = TempDir::new();
        let bin_dir = temp.root.join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        let helper = bin_dir.join("fence-bang");
        fs::write(&helper, "#!/bin/sh\n").unwrap();
        make_executable(&helper);
        let resolved = resolve_helper("fence-bang", Some(&temp.root)).unwrap();
        assert_eq!(resolved, helper);
    }

    #[test]
    fn finds_helper_on_path() {
        let temp = TempDir::new();
        let helper = temp.root.join("fence-test");
        fs::write(&helper, "#!/bin/sh\n").unwrap();
        make_executable(&helper);
        let original_path = env::var_os("PATH");
        unsafe {
            env::set_var("PATH", temp.root.to_str().unwrap());
        }
        let resolved = resolve_helper("fence-test", None).unwrap();
        assert_eq!(resolved, helper);
        if let Some(path) = original_path {
            unsafe {
                env::set_var("PATH", path);
            }
        } else {
            unsafe {
                env::remove_var("PATH");
            }
        }
    }

    struct TempDir {
        root: PathBuf,
    }

    impl TempDir {
        fn new() -> Self {
            static COUNTER: AtomicUsize = AtomicUsize::new(0);
            let mut dir = env::temp_dir();
            dir.push(format!(
                "codex-fence-test-{}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::SeqCst)
            ));
            fs::create_dir_all(&dir).unwrap();
            Self { root: dir }
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[cfg(unix)]
    fn make_executable(path: &Path) {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).unwrap();
    }

    #[cfg(not(unix))]
    fn make_executable(_path: &Path) {}
}
