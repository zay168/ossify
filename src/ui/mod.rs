pub mod model;
mod static_render;
mod tui;

use std::io;

use crate::generator::InitReport;
use crate::prompt::BugPromptReport;

pub use model::UiReport;
pub use static_render::{render_audit, render_fix, render_init, render_plan, render_prompt};
pub use tui::{run_audit_tui, run_plan_tui, supports_interactive};

pub fn render_init_report(report: &InitReport, color: bool) -> String {
    render_init(report, color)
}

pub fn render_prompt_report(report: &BugPromptReport, color: bool) -> String {
    render_prompt(report, color)
}

pub fn run_interactive_audit(report: UiReport) -> io::Result<()> {
    run_audit_tui(report)
}

pub fn run_interactive_plan(report: UiReport) -> io::Result<()> {
    run_plan_tui(report)
}
