use std::path::Path;
use std::process::ExitCode;

use ossify::audit::audit_repository;
use ossify::cli::{
    AuditArgs, Command, DepsDoctorArgs, DocsDoctorArgs, DoctorArgs, DoctorCommand, FixArgs,
    OutputFormat, ParsedArgs, PlanArgs, PromptArgs, ReleaseDoctorArgs, ScaffoldArgs,
    WorkflowDoctorArgs,
};
use ossify::config::OssifyConfig;
use ossify::doctor::{doctor_deps, doctor_docs, doctor_release, doctor_workflow, DoctorEcosystem};
use ossify::generator::{fix_repository, generate_missing_files, plan_fix_repository, InitOptions};
use ossify::prompt::build_bug_prompt_report;
use ossify::report::{
    print_audit_report, print_bug_prompt_report, print_deps_doctor_report,
    print_docs_doctor_report, print_fix_report, print_init_report, print_plan_report,
    print_release_doctor_report, print_workflow_doctor_report, OutputOptions,
};

fn main() -> ExitCode {
    let args = match ParsedArgs::parse() {
        Ok(args) => args,
        Err(error) => {
            let exit_code = error.exit_code();
            let _ = error.print();
            return ExitCode::from(exit_code.clamp(0, u8::MAX as i32) as u8);
        }
    };
    let output = OutputOptions {
        format: args.output_format(),
        color: matches!(args.output_format(), OutputFormat::Human) && args.color_choice().enabled(),
        interactive: false,
    };

    match args.command_or_default() {
        Command::Audit(command) => run_audit(command, &output),
        Command::Doctor(command) => run_doctor(command, &output),
        Command::Init(command) => run_init(command, &output),
        Command::Fix(command) => run_fix(command, &output),
        Command::Plan(command) => run_plan(command, &output),
        Command::Docs(command) => run_docs(command, &output),
        Command::Workflow(command) => run_workflow(command, &output),
        Command::Deps(command) => run_deps(command, &output),
        Command::Release(command) => run_release(command, &output),
        Command::Prompt(command) => run_prompt(command, &output),
        Command::Version => {
            println!("ossify {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
    }
}

fn run_doctor(command: DoctorArgs, output: &OutputOptions) -> ExitCode {
    match command.command {
        DoctorCommand::Docs(command) => run_docs(command, output),
        DoctorCommand::Workflow(command) => run_workflow(command, output),
        DoctorCommand::Deps(command) => run_deps(command, output),
        DoctorCommand::Release(command) => run_release(command, output),
    }
}

fn run_docs(command: DocsDoctorArgs, output: &OutputOptions) -> ExitCode {
    match doctor_docs(&command.path) {
        Ok(report) => {
            if let Err(error) = print_docs_doctor_report(&report, output) {
                eprintln!(
                    "Failed to present docs doctor report for {}: {error}",
                    command.path.display()
                );
                return ExitCode::from(1);
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!(
                "Failed to doctor docs for {}: {error}",
                command.path.display()
            );
            ExitCode::from(1)
        }
    }
}

fn run_workflow(command: WorkflowDoctorArgs, output: &OutputOptions) -> ExitCode {
    match doctor_workflow(&command.path) {
        Ok(report) => {
            if let Err(error) = print_workflow_doctor_report(&report, output) {
                eprintln!(
                    "Failed to present workflow doctor report for {}: {error}",
                    command.path.display()
                );
                return ExitCode::from(1);
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!(
                "Failed to doctor workflows for {}: {error}",
                command.path.display()
            );
            ExitCode::from(1)
        }
    }
}

fn run_deps(command: DepsDoctorArgs, output: &OutputOptions) -> ExitCode {
    match doctor_deps(&command.path, DoctorEcosystem::from(command.ecosystem)) {
        Ok(report) => {
            if let Err(error) = print_deps_doctor_report(&report, output) {
                eprintln!(
                    "Failed to present deps doctor report for {}: {error}",
                    command.path.display()
                );
                return ExitCode::from(1);
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!(
                "Failed to doctor dependencies for {}: {error}",
                command.path.display()
            );
            ExitCode::from(1)
        }
    }
}

fn run_release(command: ReleaseDoctorArgs, output: &OutputOptions) -> ExitCode {
    match doctor_release(&command.path, DoctorEcosystem::from(command.ecosystem)) {
        Ok(report) => {
            if let Err(error) = print_release_doctor_report(&report, output) {
                eprintln!(
                    "Failed to present release doctor report for {}: {error}",
                    command.path.display()
                );
                return ExitCode::from(1);
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!(
                "Failed to doctor release surface for {}: {error}",
                command.path.display()
            );
            ExitCode::from(1)
        }
    }
}

fn run_audit(command: AuditArgs, output: &OutputOptions) -> ExitCode {
    let output = OutputOptions {
        interactive: command.interactive,
        ..*output
    };
    let config = match load_config(&command.path, command.config.as_deref()) {
        Ok(config) => config,
        Err(error) => {
            eprintln!(
                "Failed to load config for {}: {error}",
                command.path.display()
            );
            return ExitCode::from(1);
        }
    };

    match audit_repository(&command.path, &config) {
        Ok(report) => {
            if let Err(error) = print_audit_report(&report, &output) {
                eprintln!("Failed to present {}: {error}", command.path.display());
                return ExitCode::from(1);
            }
            if command.strict && !report.strict_passed {
                ExitCode::from(3)
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(error) => {
            eprintln!("Failed to audit {}: {error}", command.path.display());
            ExitCode::from(1)
        }
    }
}

fn run_init(command: ScaffoldArgs, output: &OutputOptions) -> ExitCode {
    let config = match load_config(&command.path, command.config.as_deref()) {
        Ok(config) => config,
        Err(error) => {
            eprintln!(
                "Failed to load config for {}: {error}",
                command.path.display()
            );
            return ExitCode::from(1);
        }
    };
    let options = init_options(&command, &config);

    match generate_missing_files(&command.path, &options, &config) {
        Ok(report) => {
            if let Err(error) = print_init_report(&report, output) {
                eprintln!("Failed to present {}: {error}", command.path.display());
                return ExitCode::from(1);
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Failed to initialize {}: {error}", command.path.display());
            ExitCode::from(1)
        }
    }
}

fn run_fix(command: FixArgs, output: &OutputOptions) -> ExitCode {
    let output = OutputOptions {
        interactive: command.interactive,
        ..*output
    };
    let config = match load_config(&command.scaffold.path, command.scaffold.config.as_deref()) {
        Ok(config) => config,
        Err(error) => {
            eprintln!(
                "Failed to load config for {}: {error}",
                command.scaffold.path.display()
            );
            return ExitCode::from(1);
        }
    };
    let options = init_options(&command.scaffold, &config);

    if command.plan {
        match plan_fix_repository(&command.scaffold.path, &options, &config) {
            Ok(report) => {
                if let Err(error) = print_plan_report(&report, &output) {
                    eprintln!(
                        "Failed to present fix plan for {}: {error}",
                        command.scaffold.path.display()
                    );
                    return ExitCode::from(1);
                }
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!(
                    "Failed to plan fix for {}: {error}",
                    command.scaffold.path.display()
                );
                ExitCode::from(1)
            }
        }
    } else {
        match fix_repository(&command.scaffold.path, &options, &config) {
            Ok(report) => {
                if let Err(error) = print_fix_report(&report, &output) {
                    eprintln!(
                        "Failed to present {}: {error}",
                        command.scaffold.path.display()
                    );
                    return ExitCode::from(1);
                }
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("Failed to fix {}: {error}", command.scaffold.path.display());
                ExitCode::from(1)
            }
        }
    }
}

fn run_plan(command: PlanArgs, output: &OutputOptions) -> ExitCode {
    let output = OutputOptions {
        interactive: command.interactive,
        ..*output
    };
    let config = match load_config(&command.scaffold.path, command.scaffold.config.as_deref()) {
        Ok(config) => config,
        Err(error) => {
            eprintln!(
                "Failed to load config for {}: {error}",
                command.scaffold.path.display()
            );
            return ExitCode::from(1);
        }
    };
    let options = init_options(&command.scaffold, &config);

    match plan_fix_repository(&command.scaffold.path, &options, &config) {
        Ok(report) => {
            if let Err(error) = print_plan_report(&report, &output) {
                eprintln!(
                    "Failed to present fix plan for {}: {error}",
                    command.scaffold.path.display()
                );
                return ExitCode::from(1);
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!(
                "Failed to plan fix for {}: {error}",
                command.scaffold.path.display()
            );
            ExitCode::from(1)
        }
    }
}

fn run_prompt(command: PromptArgs, output: &OutputOptions) -> ExitCode {
    let config = match load_config(&command.path, command.config.as_deref()) {
        Ok(config) => config,
        Err(error) => {
            eprintln!(
                "Failed to load config for {}: {error}",
                command.path.display()
            );
            return ExitCode::from(1);
        }
    };

    let audit = match audit_repository(&command.path, &config) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("Failed to audit {}: {error}", command.path.display());
            return ExitCode::from(1);
        }
    };
    let prompt = match build_bug_prompt_report(&audit, command.rule.as_deref(), command.count) {
        Ok(prompt) => prompt,
        Err(error) => {
            eprintln!(
                "Failed to generate bug prompt for {}: {error}",
                command.path.display()
            );
            return ExitCode::from(1);
        }
    };

    if let Err(error) = print_bug_prompt_report(&prompt, output) {
        eprintln!(
            "Failed to present bug prompt for {}: {error}",
            command.path.display()
        );
        return ExitCode::from(1);
    }

    ExitCode::SUCCESS
}

fn load_config(target: &Path, explicit: Option<&Path>) -> std::io::Result<OssifyConfig> {
    OssifyConfig::load_for_target(target, explicit)
}

fn init_options(command: &ScaffoldArgs, config: &OssifyConfig) -> InitOptions {
    let license = command
        .license
        .map(Into::into)
        .or_else(|| {
            config
                .default_license()
                .and_then(|value| ossify::generator::LicenseKind::parse(value).ok())
        })
        .unwrap_or(ossify::generator::LicenseKind::Mit);
    let owner = command
        .owner
        .clone()
        .or_else(|| config.default_owner().map(str::to_owned))
        .unwrap_or_else(|| String::from("Open Source Maintainers"));
    let funding = command
        .funding
        .clone()
        .or_else(|| config.default_funding().map(str::to_owned));

    InitOptions {
        overwrite: command.overwrite,
        license,
        owner,
        funding,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ossify::generator::LicenseKind;
    use std::fs;

    #[test]
    fn cli_options_override_config_defaults() {
        let root = std::env::temp_dir().join("ossify-main-config-priority");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create temp directory");
        fs::write(
            root.join("ossify.toml"),
            "version = 1\n[defaults]\nowner = \"@config-owner\"\nlicense = \"apache-2.0\"\nfunding = \"github:config-owner\"\n",
        )
        .expect("write config");

        let config = OssifyConfig::load_for_target(&root, None).expect("load config");
        let args = ScaffoldArgs {
            path: root.clone(),
            overwrite: true,
            license: Some(ossify::cli::LicenseArg::Mit),
            owner: Some(String::from("@cli-owner")),
            funding: Some(String::from("github:cli-owner")),
            config: None,
        };

        let options = init_options(&args, &config);
        assert!(matches!(options.license, LicenseKind::Mit));
        assert_eq!(options.owner, "@cli-owner");
        assert_eq!(options.funding.as_deref(), Some("github:cli-owner"));

        let _ = fs::remove_dir_all(&root);
    }
}
