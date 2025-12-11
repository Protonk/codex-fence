//! Connector registry for run modes.
//!
//! This module centralizes how run modes map to connectors (ambient vs
//! external CLI), sandbox defaults, command planning, and optional preflight
//! hooks. Binaries should rely on this registry instead of hard-coding mode
//! strings so new connectors can be added in one place without changing public
//! CLI flags or drifting from the `run.mode` contract in the boundary-object
//! schema and `docs/probes.md`.

use crate::{external_cli_command, external_cli_present};
use anyhow::{Result, bail};
use std::env;
use std::env::VarError;
use std::ffi::OsString;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectorKind {
    Ambient,
    ExternalCli,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunMode {
    Baseline,
    CodexSandbox,
    CodexFull,
}

impl RunMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            RunMode::Baseline => "baseline",
            RunMode::CodexSandbox => "codex-sandbox",
            RunMode::CodexFull => "codex-full",
        }
    }

    pub fn connector(&self) -> ConnectorKind {
        match self {
            RunMode::Baseline | RunMode::CodexFull => ConnectorKind::Ambient,
            RunMode::CodexSandbox => ConnectorKind::ExternalCli,
        }
    }

    pub fn is_external(&self) -> bool {
        matches!(self.connector(), ConnectorKind::ExternalCli)
    }

    pub fn sandbox_env(&self, override_value: Option<String>) -> OsString {
        match self {
            RunMode::Baseline => OsString::from(""),
            RunMode::CodexSandbox => {
                OsString::from(override_value.unwrap_or_else(|| "workspace-write".to_string()))
            }
            RunMode::CodexFull => {
                OsString::from(override_value.unwrap_or_else(|| "danger-full-access".to_string()))
            }
        }
    }

    pub fn sandbox_stack_value(&self, override_value: Option<String>) -> Result<Option<String>> {
        let value = self.sandbox_env(override_value);
        if value.is_empty() {
            return Ok(None);
        }
        match value.into_string() {
            Ok(valid) => Ok(Some(valid)),
            Err(os) => Ok(Some(os.to_string_lossy().into_owned())),
        }
    }

    fn platform_target(&self, platform: &str) -> Result<Option<String>> {
        match self {
            RunMode::CodexSandbox => Ok(Some(external_platform_target(platform)?.to_string())),
            RunMode::Baseline | RunMode::CodexFull => Ok(None),
        }
    }

    fn ensure_connector_present(&self) -> Result<()> {
        match self.connector() {
            ConnectorKind::Ambient => Ok(()),
            ConnectorKind::ExternalCli => ensure_external_available(),
        }
    }

    fn command_spec(
        &self,
        platform_target: Option<&str>,
        probe_path: &Path,
    ) -> Result<CommandSpec> {
        let probe_arg = probe_path.as_os_str().to_os_string();
        let external_cli = external_cli_command();
        match self {
            RunMode::Baseline | RunMode::CodexFull => Ok(CommandSpec {
                program: probe_arg,
                args: Vec::new(),
            }),
            RunMode::CodexSandbox => {
                let target = platform_target
                    .ok_or_else(|| anyhow::anyhow!("missing external platform target"))?;
                Ok(CommandSpec {
                    program: external_cli,
                    args: vec![
                        OsString::from("sandbox"),
                        OsString::from(target),
                        OsString::from("--full-auto"),
                        OsString::from("--"),
                        probe_arg,
                    ],
                })
            }
        }
    }

    fn preflight_plan(&self, platform_target: Option<&str>) -> Result<Option<PreflightPlan>> {
        match self {
            RunMode::CodexSandbox => Ok(Some(PreflightPlan::ExternalTmp {
                platform_target: platform_target
                    .ok_or_else(|| anyhow::anyhow!("missing external platform target"))?
                    .to_string(),
            })),
            RunMode::Baseline | RunMode::CodexFull => Ok(None),
        }
    }
}

impl TryFrom<&str> for RunMode {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            "baseline" => Ok(RunMode::Baseline),
            "codex-sandbox" => Ok(RunMode::CodexSandbox),
            "codex-full" => Ok(RunMode::CodexFull),
            other => bail!("Unknown mode: {other}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModePlan {
    pub run_mode: RunMode,
    pub connector: ConnectorKind,
    pub sandbox_env: OsString,
    pub command: CommandSpec,
    pub preflight: Option<PreflightPlan>,
}

pub fn plan_for_mode(
    requested_mode: &str,
    platform: &str,
    probe_path: &Path,
    sandbox_override: Option<String>,
) -> Result<ModePlan> {
    let run_mode = RunMode::try_from(requested_mode)?;
    run_mode.ensure_connector_present()?;
    let platform_target = run_mode.platform_target(platform)?;
    let sandbox_env = run_mode.sandbox_env(sandbox_override);
    let command = run_mode.command_spec(platform_target.as_deref(), probe_path)?;
    let preflight = run_mode.preflight_plan(platform_target.as_deref())?;

    Ok(ModePlan {
        run_mode,
        connector: run_mode.connector(),
        sandbox_env,
        command,
        preflight,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreflightPlan {
    ExternalTmp { platform_target: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandSpec {
    pub program: OsString,
    pub args: Vec<OsString>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Availability {
    pub external_cli_present: bool,
}

impl Availability {
    pub fn for_host() -> Self {
        Self {
            external_cli_present: external_cli_present(),
        }
    }
}

pub fn default_mode_names(availability: Availability) -> Vec<String> {
    MODE_SPECS
        .iter()
        .filter(|spec| (spec.default_gate)(&availability))
        .map(|spec| spec.run_mode.as_str().to_string())
        .collect()
}

pub fn parse_modes(modes: &[String]) -> Result<Vec<RunMode>> {
    modes
        .iter()
        .map(|mode| RunMode::try_from(mode.as_str()))
        .collect()
}

pub fn allowed_mode_names() -> Vec<&'static str> {
    MODE_SPECS
        .iter()
        .map(|spec| spec.run_mode.as_str())
        .collect()
}

pub fn sandbox_override_from_env() -> Option<String> {
    match env::var("FENCE_SANDBOX_MODE") {
        Ok(value) if !value.is_empty() => Some(value),
        Ok(_) => None,
        Err(VarError::NotPresent) => None,
        Err(VarError::NotUnicode(os)) => Some(os.to_string_lossy().into_owned()),
    }
}

fn external_platform_target(platform: &str) -> Result<&'static str> {
    match platform {
        "Darwin" => Ok("macos"),
        "Linux" => Ok("linux"),
        other => bail!("Unsupported platform for external sandbox mode: {other}"),
    }
}

fn ensure_external_available() -> Result<()> {
    if external_cli_present() {
        return Ok(());
    }
    let runner = external_cli_command();
    bail!(
        "external CLI '{runner}' not found; codex-* modes require the configured external runner. Install codex (or override the runner) or run baseline instead.",
        runner = runner.to_string_lossy()
    )
}

fn always_available(_: &Availability) -> bool {
    true
}

fn external_available(availability: &Availability) -> bool {
    availability.external_cli_present
}

struct ModeSpec {
    run_mode: RunMode,
    default_gate: fn(&Availability) -> bool,
}

const MODE_SPECS: &[ModeSpec] = &[
    ModeSpec {
        run_mode: RunMode::Baseline,
        default_gate: always_available,
    },
    ModeSpec {
        run_mode: RunMode::CodexFull,
        default_gate: always_available,
    },
    ModeSpec {
        run_mode: RunMode::CodexSandbox,
        default_gate: external_available,
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn defaults_gate_on_external_availability() {
        let without_external = Availability {
            external_cli_present: false,
        };
        let with_external = Availability {
            external_cli_present: true,
        };

        assert_eq!(
            default_mode_names(without_external),
            vec!["baseline".to_string(), "codex-full".to_string()]
        );
        assert_eq!(
            default_mode_names(with_external),
            vec![
                "baseline".to_string(),
                "codex-full".to_string(),
                "codex-sandbox".to_string(),
            ]
        );
    }

    #[test]
    fn run_mode_parse_and_strings_round_trip() {
        let baseline = RunMode::try_from("baseline").expect("baseline parses");
        assert_eq!(baseline.as_str(), "baseline");
        let codex_full = RunMode::try_from("codex-full").expect("codex-full parses");
        assert!(!codex_full.is_external());
        assert!(RunMode::try_from("unknown-mode").is_err());
    }

    #[test]
    fn baseline_plan_uses_direct_execution() {
        let plan = plan_for_mode(
            "baseline",
            "Darwin",
            PathBuf::from("/tmp/probe.sh").as_path(),
            None,
        )
        .expect("baseline plan");

        assert_eq!(plan.run_mode, RunMode::Baseline);
        assert_eq!(plan.connector, ConnectorKind::Ambient);
        assert!(plan.preflight.is_none());
        assert_eq!(plan.command.args.len(), 0);
        assert_eq!(plan.command.program, OsString::from("/tmp/probe.sh"));
    }
}
