mod audit;
mod cli;
mod generator;
mod project;
mod report;
mod templates;

use std::path::PathBuf;
use std::process::ExitCode;

use audit::audit_repository;
use cli::{Command, OutputFormat, ParsedArgs};
use generator::{fix_repository, generate_missing_files, InitOptions};
use report::{print_audit_report, print_fix_report, print_init_report, OutputOptions};

fn main() -> ExitCode {
    let args = match ParsedArgs::parse() {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };
    let output = OutputOptions {
        format: args.output,
        color: matches!(args.output, OutputFormat::Human) && args.color.enabled(),
    };

    match args.command {
        Command::Audit { path } => run_audit(path, &output),
        Command::Init {
            path,
            overwrite,
            license,
            owner,
        } => run_init(path, overwrite, license, owner, &output),
        Command::Fix {
            path,
            overwrite,
            license,
            owner,
        } => run_fix(path, overwrite, license, owner, &output),
        Command::Help => {
            println!("{}", cli::help_text());
            ExitCode::SUCCESS
        }
        Command::Version => {
            println!("ossify {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
    }
}

fn run_audit(path: PathBuf, output: &OutputOptions) -> ExitCode {
    match audit_repository(&path) {
        Ok(report) => {
            print_audit_report(&report, output);
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Failed to audit {}: {error}", path.display());
            ExitCode::from(1)
        }
    }
}

fn run_init(
    path: PathBuf,
    overwrite: bool,
    license: generator::LicenseKind,
    owner: String,
    output: &OutputOptions,
) -> ExitCode {
    let options = InitOptions {
        overwrite,
        license,
        owner,
    };

    match generate_missing_files(&path, &options) {
        Ok(report) => {
            print_init_report(&report, output);
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Failed to initialize {}: {error}", path.display());
            ExitCode::from(1)
        }
    }
}

fn run_fix(
    path: PathBuf,
    overwrite: bool,
    license: generator::LicenseKind,
    owner: String,
    output: &OutputOptions,
) -> ExitCode {
    let options = InitOptions {
        overwrite,
        license,
        owner,
    };

    match fix_repository(&path, &options) {
        Ok(report) => {
            print_fix_report(&report, output);
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Failed to fix {}: {error}", path.display());
            ExitCode::from(1)
        }
    }
}
