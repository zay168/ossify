use std::env;
use std::path::PathBuf;

use crate::generator::LicenseKind;

pub struct ParsedArgs {
    pub command: Command,
    pub output: OutputFormat,
    pub color: ColorChoice,
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

pub enum Command {
    Audit {
        path: PathBuf,
    },
    Init {
        path: PathBuf,
        overwrite: bool,
        license: LicenseKind,
        owner: String,
    },
    Fix {
        path: PathBuf,
        overwrite: bool,
        license: LicenseKind,
        owner: String,
    },
    Help,
    Version,
}

impl ParsedArgs {
    pub fn parse() -> Result<Self, String> {
        let raw: Vec<String> = env::args().skip(1).collect();
        let mut output = OutputFormat::Human;
        let mut color = ColorChoice::Auto;
        let mut command_name: Option<String> = None;
        let mut command_args = Vec::new();

        for arg in raw {
            match arg.as_str() {
                "--json" => output = OutputFormat::Json,
                "--color" => color = ColorChoice::Always,
                "--no-color" => color = ColorChoice::Never,
                "audit" | "init" | "fix" | "help" | "--help" | "-h" | "version" | "--version"
                | "-V"
                    if command_name.is_none() =>
                {
                    command_name = Some(arg);
                }
                value if value.starts_with('-') && command_name.is_none() => {
                    return Err(format!("Unknown global flag: {value}\n\n{}", help_text()));
                }
                value if command_name.is_none() => {
                    command_name = Some(String::from("audit"));
                    command_args.push(value.to_owned());
                }
                value => command_args.push(value.to_owned()),
            }
        }

        let command_name = command_name.unwrap_or_else(|| String::from("audit"));
        let command = match command_name.as_str() {
            "audit" => parse_audit(command_args)?,
            "init" => parse_scaffold(command_args, ScaffoldCommand::Init)?,
            "fix" => parse_scaffold(command_args, ScaffoldCommand::Fix)?,
            "help" | "--help" | "-h" => Command::Help,
            "--version" | "-V" | "version" => Command::Version,
            unknown => {
                return Err(format!(
                    "Unknown command: {unknown}\n\n{}",
                    help_text()
                ))
            }
        };

        Ok(Self {
            command,
            output,
            color,
        })
    }
}

fn parse_audit(args: Vec<String>) -> Result<Command, String> {
    if args.len() > 1 {
        return Err(format!(
            "Too many arguments for audit.\n\n{}",
            help_text()
        ));
    }

    let path = args
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    Ok(Command::Audit { path })
}

enum ScaffoldCommand {
    Init,
    Fix,
}

fn parse_scaffold(args: Vec<String>, command: ScaffoldCommand) -> Result<Command, String> {
    let mut path = PathBuf::from(".");
    let mut overwrite = false;
    let mut license = LicenseKind::Mit;
    let mut owner = String::from("Open Source Maintainers");

    let mut index = 0;
    while index < args.len() {
        let current = &args[index];
        match current.as_str() {
            "--overwrite" => {
                overwrite = true;
                index += 1;
            }
            "--license" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| String::from("Missing value for --license"))?;
                license = LicenseKind::parse(value)?;
                index += 2;
            }
            "--owner" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| String::from("Missing value for --owner"))?;
                owner = value.clone();
                index += 2;
            }
            value if value.starts_with("--") => {
                return Err(format!("Unknown flag: {value}\n\n{}", help_text()));
            }
            value => {
                path = PathBuf::from(value);
                index += 1;
            }
        }
    }

    Ok(match command {
        ScaffoldCommand::Init => Command::Init {
            path,
            overwrite,
            license,
            owner,
        },
        ScaffoldCommand::Fix => Command::Fix {
            path,
            overwrite,
            license,
            owner,
        },
    })
}

pub fn help_text() -> String {
    format!(
        "\
ossify {}

USAGE:
  ossify audit [path]
  ossify init [path] [--overwrite] [--license mit|apache-2.0] [--owner \"Your Name\"]
  ossify fix [path] [--overwrite] [--license mit|apache-2.0] [--owner \"Your Name\"]
  ossify version
  ossify help

GLOBAL OPTIONS:
  --json        Print machine-readable JSON
  --color       Force ANSI colors
  --no-color    Disable ANSI colors

DESCRIPTION:
  Audit a repository, scaffold missing community files, and autofix repository hygiene gaps.
",
        env!("CARGO_PKG_VERSION")
    )
}
