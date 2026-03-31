use std::env;
use std::path::PathBuf;

use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};

use crate::generator::LicenseKind;

#[derive(Debug, Parser)]
#[command(
    name = "ossify",
    version,
    about = "Audit repository readiness and scaffold GitHub-aware open source files."
)]
pub struct ParsedArgs {
    #[arg(long, global = true)]
    pub json: bool,
    #[arg(long, global = true, conflicts_with = "no_color")]
    pub color: bool,
    #[arg(long = "no-color", global = true)]
    pub no_color: bool,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Audit(AuditArgs),
    Doctor(DoctorArgs),
    Init(ScaffoldArgs),
    Fix(FixArgs),
    Prompt(PromptArgs),
    Version,
}

#[derive(Debug, Clone, Args)]
pub struct DoctorArgs {
    #[command(subcommand)]
    pub command: DoctorCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum DoctorCommand {
    Docs(DocsDoctorArgs),
    Workflow(WorkflowDoctorArgs),
}

#[derive(Debug, Clone, Args)]
pub struct DocsDoctorArgs {
    #[arg(default_value = ".")]
    pub path: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub struct WorkflowDoctorArgs {
    #[arg(default_value = ".")]
    pub path: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub struct AuditArgs {
    #[arg(default_value = ".")]
    pub path: PathBuf,
    #[arg(long)]
    pub config: Option<PathBuf>,
    #[arg(long)]
    pub strict: bool,
    #[arg(long, conflicts_with = "json")]
    pub interactive: bool,
}

#[derive(Debug, Clone, Args)]
pub struct ScaffoldArgs {
    #[arg(default_value = ".")]
    pub path: PathBuf,
    #[arg(long)]
    pub overwrite: bool,
    #[arg(long, value_enum)]
    pub license: Option<LicenseArg>,
    #[arg(long)]
    pub owner: Option<String>,
    #[arg(long)]
    pub funding: Option<String>,
    #[arg(long)]
    pub config: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct FixArgs {
    #[command(flatten)]
    pub scaffold: ScaffoldArgs,
    #[arg(long)]
    pub plan: bool,
    #[arg(long, conflicts_with = "json", requires = "plan")]
    pub interactive: bool,
}

#[derive(Debug, Clone, Args)]
pub struct PromptArgs {
    #[arg(default_value = ".")]
    pub path: PathBuf,
    #[arg(long)]
    pub config: Option<PathBuf>,
    #[arg(long)]
    pub rule: Option<String>,
    #[arg(long, default_value_t = 0)]
    pub count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum LicenseArg {
    Mit,
    #[value(name = "apache-2.0")]
    Apache20,
}

impl From<LicenseArg> for LicenseKind {
    fn from(value: LicenseArg) -> Self {
        match value {
            LicenseArg::Mit => LicenseKind::Mit,
            LicenseArg::Apache20 => LicenseKind::Apache2,
        }
    }
}

#[derive(Clone, Copy)]
pub enum OutputFormat {
    Human,
    Json,
}

#[derive(Clone, Copy)]
pub enum ColorChoice {
    Auto,
    Always,
    Never,
}

impl ColorChoice {
    pub fn enabled(self) -> bool {
        match self {
            Self::Always => true,
            Self::Never => false,
            Self::Auto => env::var_os("NO_COLOR").is_none(),
        }
    }
}

impl ParsedArgs {
    pub fn parse() -> Result<Self, String> {
        let raw = env::args().collect::<Vec<String>>();
        let prepared = prepare_default_command(raw);
        Self::try_parse_from(prepared).map_err(|error| error.to_string())
    }

    pub fn try_parse_from<I, T>(itr: I) -> Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        let parsed = <Self as Parser>::try_parse_from(itr)?;
        parsed.validate()
    }

    pub fn command_or_default(self) -> Command {
        self.command.unwrap_or(Command::Audit(AuditArgs {
            path: PathBuf::from("."),
            config: None,
            strict: false,
            interactive: false,
        }))
    }

    pub fn output_format(&self) -> OutputFormat {
        if self.json {
            OutputFormat::Json
        } else {
            OutputFormat::Human
        }
    }

    pub fn color_choice(&self) -> ColorChoice {
        if self.color {
            ColorChoice::Always
        } else if self.no_color {
            ColorChoice::Never
        } else {
            ColorChoice::Auto
        }
    }

    fn validate(self) -> Result<Self, clap::Error> {
        if self.json {
            let interactive = match self.command.as_ref() {
                Some(Command::Audit(command)) => command.interactive,
                Some(Command::Fix(command)) => command.interactive,
                _ => false,
            };
            if interactive {
                return Err(Self::command().error(
                    clap::error::ErrorKind::ArgumentConflict,
                    "`--interactive` cannot be used together with `--json`.",
                ));
            }
        }

        Ok(self)
    }
}

fn prepare_default_command(mut raw: Vec<String>) -> Vec<String> {
    const SUBCOMMANDS: &[&str] = &[
        "audit", "doctor", "init", "fix", "prompt", "help", "version",
    ];

    if raw.len() <= 1 {
        return raw;
    }

    if let Some((index, value)) = raw
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, value)| !value.starts_with('-'))
    {
        if !SUBCOMMANDS.contains(&value.as_str()) {
            raw.insert(index, String::from("audit"));
        }
    }

    raw
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fix_plan_flag_parses() {
        let args = ParsedArgs::try_parse_from(["ossify", "fix", ".", "--plan", "--interactive"])
            .expect("parse fix --plan");

        match args.command.expect("command") {
            Command::Fix(command) => {
                assert!(command.plan);
                assert!(command.interactive);
                assert_eq!(command.scaffold.path, PathBuf::from("."));
            }
            _ => panic!("expected fix command"),
        }
    }

    #[test]
    fn init_rejects_plan_flag() {
        let result = ParsedArgs::try_parse_from(["ossify", "init", ".", "--plan"]);
        assert!(result.is_err());
    }

    #[test]
    fn audit_interactive_parses() {
        let args = ParsedArgs::try_parse_from(["ossify", "audit", ".", "--interactive"])
            .expect("parse audit --interactive");

        match args.command.expect("command") {
            Command::Audit(command) => assert!(command.interactive),
            _ => panic!("expected audit command"),
        }
    }

    #[test]
    fn fix_interactive_requires_plan() {
        let result = ParsedArgs::try_parse_from(["ossify", "fix", ".", "--interactive"]);
        assert!(result.is_err());
    }

    #[test]
    fn interactive_conflicts_with_json() {
        let result =
            ParsedArgs::try_parse_from(["ossify", "--json", "audit", ".", "--interactive"]);
        assert!(result.is_err());
    }

    #[test]
    fn prompt_rule_parses() {
        let args = ParsedArgs::try_parse_from([
            "ossify", "prompt", ".", "--rule", "readme", "--count", "2",
        ])
        .expect("parse prompt");

        match args.command.expect("command") {
            Command::Prompt(command) => {
                assert_eq!(command.path, PathBuf::from("."));
                assert_eq!(command.rule.as_deref(), Some("readme"));
                assert_eq!(command.count, 2);
            }
            _ => panic!("expected prompt command"),
        }
    }

    #[test]
    fn doctor_docs_parses() {
        let args = ParsedArgs::try_parse_from(["ossify", "doctor", "docs", "."])
            .expect("parse doctor docs");

        match args.command.expect("command") {
            Command::Doctor(command) => match command.command {
                DoctorCommand::Docs(command) => assert_eq!(command.path, PathBuf::from(".")),
                DoctorCommand::Workflow(_) => panic!("expected docs doctor"),
            },
            _ => panic!("expected doctor command"),
        }
    }

    #[test]
    fn doctor_workflow_parses() {
        let args = ParsedArgs::try_parse_from(["ossify", "doctor", "workflow", "."])
            .expect("parse doctor workflow");

        match args.command.expect("command") {
            Command::Doctor(command) => match command.command {
                DoctorCommand::Workflow(command) => {
                    assert_eq!(command.path, PathBuf::from("."))
                }
                _ => panic!("expected workflow doctor"),
            },
            _ => panic!("expected doctor command"),
        }
    }
}
