use std::env;
use std::io::{self, IsTerminal, Stdout};
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Clear, Gauge, List, ListItem, ListState, Paragraph, Tabs, Wrap,
};
use ratatui::{Frame, Terminal};

use crate::audit::{CheckStatus, RuleCategory};

use super::model::{UiDiagnostic, UiMode, UiReport};

const FRAME_INTERVAL: Duration = Duration::from_millis(33);
const NAV_REPEAT_INTERVAL: Duration = Duration::from_millis(120);
const VIEW_REPEAT_INTERVAL: Duration = Duration::from_millis(180);
const ENTRY_ANIMATION: Duration = Duration::from_millis(700);
const INTERACTION_PULSE: Duration = Duration::from_millis(220);
const CATEGORY_BAR_WIDTH: usize = 14;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveView {
    Overview,
    Checks,
    Diagnostics,
    Plan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusFilter {
    All,
    Strong,
    Partial,
    Missing,
}

impl StatusFilter {
    fn next(self) -> Self {
        match self {
            Self::All => Self::Strong,
            Self::Strong => Self::Partial,
            Self::Partial => Self::Missing,
            Self::Missing => Self::All,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Strong => "strong",
            Self::Partial => "partial",
            Self::Missing => "missing",
        }
    }

    fn matches(self, status: CheckStatus) -> bool {
        match self {
            Self::All => true,
            Self::Strong => status == CheckStatus::Strong,
            Self::Partial => status == CheckStatus::Partial,
            Self::Missing => status == CheckStatus::Missing,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExplorerApp {
    report: UiReport,
    active_view: ActiveView,
    status_filter: StatusFilter,
    category_filter: Option<RuleCategory>,
    detail_open: bool,
    selected_check: usize,
    selected_diagnostic: usize,
    selected_file: usize,
    started_at: Instant,
    last_nav_repeat_at: Option<Instant>,
    last_view_repeat_at: Option<Instant>,
    last_interaction_at: Instant,
}

impl ExplorerApp {
    pub fn new(report: UiReport) -> Self {
        let now = Instant::now();
        Self {
            active_view: ActiveView::Overview,
            report,
            status_filter: StatusFilter::All,
            category_filter: None,
            detail_open: false,
            selected_check: 0,
            selected_diagnostic: 0,
            selected_file: 0,
            started_at: now,
            last_nav_repeat_at: None,
            last_view_repeat_at: None,
            last_interaction_at: now,
        }
    }

    #[cfg(test)]
    pub fn active_view(&self) -> ActiveView {
        self.active_view
    }

    #[cfg(test)]
    pub fn status_filter(&self) -> StatusFilter {
        self.status_filter
    }

    #[cfg(test)]
    pub fn category_filter(&self) -> Option<RuleCategory> {
        self.category_filter
    }

    #[cfg(test)]
    pub fn detail_open(&self) -> bool {
        self.detail_open
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        self.handle_key_at(key, Instant::now())
    }

    fn handle_key_at(&mut self, key: KeyEvent, now: Instant) -> bool {
        if matches!(key.kind, KeyEventKind::Release) {
            return false;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return true,
            KeyCode::Tab => {
                if self.accept_repeat(now, key.kind, RepeatBucket::View) {
                    self.next_view(now);
                }
            }
            KeyCode::BackTab => {
                if self.accept_repeat(now, key.kind, RepeatBucket::View) {
                    self.prev_view(now);
                }
            }
            KeyCode::Down => {
                if self.accept_repeat(now, key.kind, RepeatBucket::Navigation) {
                    self.move_selection(1, now);
                }
            }
            KeyCode::Up => {
                if self.accept_repeat(now, key.kind, RepeatBucket::Navigation) {
                    self.move_selection(-1, now);
                }
            }
            KeyCode::Enter if key.kind != KeyEventKind::Repeat => {
                self.detail_open = !self.detail_open;
                self.pulse(now);
            }
            KeyCode::Char('f') => {
                self.status_filter = self.status_filter.next();
                self.clamp_selection();
                self.pulse(now);
            }
            KeyCode::Char('c') => {
                self.category_filter = next_category(self.category_filter);
                self.clamp_selection();
                self.pulse(now);
            }
            _ => {}
        }

        false
    }

    fn accept_repeat(&mut self, now: Instant, kind: KeyEventKind, bucket: RepeatBucket) -> bool {
        if matches!(kind, KeyEventKind::Press) {
            match bucket {
                RepeatBucket::Navigation => self.last_nav_repeat_at = Some(now),
                RepeatBucket::View => self.last_view_repeat_at = Some(now),
            }
            return true;
        }

        let (slot, interval) = match bucket {
            RepeatBucket::Navigation => (&mut self.last_nav_repeat_at, NAV_REPEAT_INTERVAL),
            RepeatBucket::View => (&mut self.last_view_repeat_at, VIEW_REPEAT_INTERVAL),
        };

        match slot {
            Some(last) if now.duration_since(*last) < interval => false,
            _ => {
                *slot = Some(now);
                true
            }
        }
    }

    fn views(&self) -> Vec<ActiveView> {
        let mut views = vec![
            ActiveView::Overview,
            ActiveView::Checks,
            ActiveView::Diagnostics,
        ];
        if self.report.mode == UiMode::Plan {
            views.push(ActiveView::Plan);
        }
        views
    }

    fn next_view(&mut self, now: Instant) {
        let views = self.views();
        let index = views
            .iter()
            .position(|view| *view == self.active_view)
            .unwrap_or(0);
        self.active_view = views[(index + 1) % views.len()];
        self.clamp_selection();
        self.pulse(now);
    }

    fn prev_view(&mut self, now: Instant) {
        let views = self.views();
        let index = views
            .iter()
            .position(|view| *view == self.active_view)
            .unwrap_or(0);
        self.active_view = views[(index + views.len() - 1) % views.len()];
        self.clamp_selection();
        self.pulse(now);
    }

    fn move_selection(&mut self, delta: isize, now: Instant) {
        match self.active_view {
            ActiveView::Checks => {
                self.selected_check =
                    next_index(self.selected_check, self.filtered_checks().len(), delta)
            }
            ActiveView::Diagnostics => {
                self.selected_diagnostic = next_index(
                    self.selected_diagnostic,
                    self.filtered_diagnostics().len(),
                    delta,
                )
            }
            ActiveView::Plan => {
                self.selected_file = next_index(self.selected_file, self.report.files.len(), delta)
            }
            ActiveView::Overview => {}
        }
        self.pulse(now);
    }

    fn filtered_checks(&self) -> Vec<usize> {
        self.report
            .checks
            .iter()
            .enumerate()
            .filter(|(_, check)| {
                self.status_filter.matches(check.status)
                    && self
                        .category_filter
                        .map(|category| check.category == category)
                        .unwrap_or(true)
            })
            .map(|(index, _)| index)
            .collect()
    }

    fn filtered_diagnostics(&self) -> Vec<usize> {
        self.report
            .diagnostics
            .iter()
            .enumerate()
            .filter(|(_, diagnostic)| {
                self.status_filter.matches(diagnostic.check_status)
                    && self
                        .category_filter
                        .map(|category| diagnostic.category == category)
                        .unwrap_or(true)
            })
            .map(|(index, _)| index)
            .collect()
    }

    fn clamp_selection(&mut self) {
        self.selected_check = self
            .selected_check
            .min(self.filtered_checks().len().saturating_sub(1));
        self.selected_diagnostic = self
            .selected_diagnostic
            .min(self.filtered_diagnostics().len().saturating_sub(1));
        self.selected_file = self
            .selected_file
            .min(self.report.files.len().saturating_sub(1));
    }

    fn pulse(&mut self, now: Instant) {
        self.last_interaction_at = now;
    }

    fn animation_progress(&self, now: Instant) -> f32 {
        let elapsed = now.duration_since(self.started_at);
        (elapsed.as_secs_f32() / ENTRY_ANIMATION.as_secs_f32()).clamp(0.0, 1.0)
    }

    fn interaction_strength(&self, now: Instant) -> f32 {
        let elapsed = now.saturating_duration_since(self.last_interaction_at);
        let remaining = (INTERACTION_PULSE.as_secs_f32() - elapsed.as_secs_f32()).max(0.0);
        (remaining / INTERACTION_PULSE.as_secs_f32()).clamp(0.0, 1.0)
    }

    fn displayed_score(&self, now: Instant) -> u8 {
        let target = self.report.current.score as f32;
        let start = self
            .report
            .previous
            .map(|previous| previous.score as f32)
            .unwrap_or(0.0);
        let eased = ease_out_cubic(self.animation_progress(now));
        (start + (target - start) * eased).round().clamp(0.0, 100.0) as u8
    }

    fn displayed_category_score(&self, target: u8, now: Instant) -> u8 {
        let eased = ease_out_cubic(self.animation_progress(now));
        (target as f32 * eased).round().clamp(0.0, 100.0) as u8
    }
}

#[derive(Debug, Clone, Copy)]
enum RepeatBucket {
    Navigation,
    View,
}

pub fn supports_interactive() -> bool {
    interactive_supported(io::stdout().is_terminal(), env::var("TERM").ok())
}

pub fn interactive_supported(is_tty: bool, term: Option<String>) -> bool {
    is_tty && !matches!(term.as_deref(), Some("dumb"))
}

pub fn run_audit_tui(report: UiReport) -> io::Result<()> {
    run_explorer(report)
}

pub fn run_plan_tui(report: UiReport) -> io::Result<()> {
    run_explorer(report)
}

fn run_explorer(report: UiReport) -> io::Result<()> {
    let mut terminal = init_terminal()?;
    let mut app = ExplorerApp::new(report);

    loop {
        terminal.draw(|frame| draw_app(frame, &app))?;

        if event::poll(FRAME_INTERVAL)? {
            if let Event::Key(key) = event::read()? {
                if app.handle_key(key) {
                    break;
                }
            }
        }
    }

    restore_terminal(terminal)
}

fn init_terminal() -> io::Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    Terminal::new(CrosstermBackend::new(stdout))
}

fn restore_terminal(mut terminal: Terminal<CrosstermBackend<Stdout>>) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()
}

pub fn draw_app(frame: &mut Frame<'_>, app: &ExplorerApp) {
    let now = Instant::now();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(9),
            Constraint::Min(10),
            Constraint::Length(2),
        ])
        .split(frame.area());

    render_tabs(frame, app, chunks[0], now);
    render_summary(frame, app, chunks[1], now);

    match app.active_view {
        ActiveView::Overview => render_overview(frame, app, chunks[2], now),
        ActiveView::Checks => render_checks(frame, app, chunks[2], now),
        ActiveView::Diagnostics => render_diagnostics(frame, app, chunks[2], now),
        ActiveView::Plan => render_plan(frame, app, chunks[2], now),
    }

    render_footer(frame, app, chunks[3]);
}

fn render_tabs(frame: &mut Frame<'_>, app: &ExplorerApp, area: Rect, now: Instant) {
    let titles = app
        .views()
        .into_iter()
        .map(|view| Line::from(Span::raw(view_label(view))))
        .collect::<Vec<_>>();
    let selected = app
        .views()
        .iter()
        .position(|view| *view == app.active_view)
        .unwrap_or(0);
    let tabs = Tabs::new(titles)
        .select(selected)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(app.report.title)
                .border_style(accent_style(app.interaction_strength(now))),
        )
        .highlight_style(
            Style::default()
                .fg(accent_color(app.interaction_strength(now)))
                .add_modifier(Modifier::BOLD),
        )
        .style(Style::default().fg(Color::Gray));
    frame.render_widget(tabs, area);
}

fn render_summary(frame: &mut Frame<'_>, app: &ExplorerApp, area: Rect, now: Instant) {
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);
    let displayed_score = app.displayed_score(now);

    let left_lines = vec![
        Line::from(vec![
            Span::styled(
                app.report.project_name.clone(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(" ({})", app.report.project_summary)),
        ]),
        Line::from(format!("Target: {}", app.report.target)),
        Line::from(match app.report.previous {
            Some(previous) => format!(
                "Score: {} -> {} ({:+})",
                previous.score,
                displayed_score,
                displayed_score as i16 - previous.score as i16
            ),
            None => format!("Score: {}", displayed_score),
        }),
        Line::from(format!(
            "Tier: {}   Strict: {} / {}",
            app.report.current.readiness.as_str(),
            if app.report.current.strict_passed {
                "pass"
            } else {
                "fail"
            },
            app.report.current.minimum_score
        )),
    ];
    frame.render_widget(
        Paragraph::new(left_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Overview")
                    .border_style(accent_style(app.interaction_strength(now))),
            )
            .wrap(Wrap { trim: true }),
        horizontal[0],
    );

    let gauge = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Readiness")
                .border_style(accent_style(app.interaction_strength(now))),
        )
        .gauge_style(score_style(displayed_score, app.interaction_strength(now)))
        .label(format!("{displayed_score}/100"))
        .percent(displayed_score as u16);
    frame.render_widget(gauge, horizontal[1]);
}

fn render_overview(frame: &mut Frame<'_>, app: &ExplorerApp, area: Rect, now: Instant) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(columns[0]);
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints(if app.report.mode == UiMode::Plan {
            [Constraint::Percentage(45), Constraint::Percentage(55)]
        } else {
            [Constraint::Percentage(100), Constraint::Min(0)]
        })
        .split(columns[1]);

    let categories = app
        .report
        .categories
        .iter()
        .map(|category| {
            let animated = app.displayed_category_score(category.score, now);
            ListItem::new(format!(
                "{:<10} {} {:>3}% ({}/{})",
                category.label,
                progress_bar(animated, CATEGORY_BAR_WIDTH),
                category.score,
                category.earned,
                category.total
            ))
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        List::new(categories).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Categories")
                .border_style(accent_style(app.interaction_strength(now))),
        ),
        left[0],
    );

    let gaps = app
        .report
        .checks
        .iter()
        .filter(|check| check.status != CheckStatus::Strong)
        .take(6)
        .map(|check| {
            ListItem::new(format!(
                "{} | {}",
                check.label,
                check
                    .primary_cause
                    .clone()
                    .unwrap_or_else(|| check.message.clone())
            ))
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        List::new(gaps).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Priority Gaps")
                .border_style(accent_style(app.interaction_strength(now))),
        ),
        left[1],
    );

    let diagnostics = app
        .report
        .diagnostics
        .iter()
        .take(8)
        .map(|diagnostic| {
            ListItem::new(format!(
                "{} | {}",
                diagnostic.rule_label, diagnostic.message
            ))
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        List::new(diagnostics).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Diagnostics")
                .border_style(accent_style(app.interaction_strength(now))),
        ),
        right[0],
    );

    if app.report.mode == UiMode::Plan {
        let plan = app
            .report
            .files
            .iter()
            .map(|file| ListItem::new(format!("{:?} | {}", file.action, file.path)))
            .collect::<Vec<_>>();
        frame.render_widget(
            List::new(plan).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Plan Files")
                    .border_style(accent_style(app.interaction_strength(now))),
            ),
            right[1],
        );
    }
}

fn render_checks(frame: &mut Frame<'_>, app: &ExplorerApp, area: Rect, now: Instant) {
    let filtered = app.filtered_checks();
    let layout = split_detail(area, app.detail_open);
    let items = filtered
        .iter()
        .map(|index| {
            let check = &app.report.checks[*index];
            ListItem::new(format!(
                "{} | {} | {}",
                check.status.as_str(),
                check.category.as_str(),
                check.label
            ))
        })
        .collect::<Vec<_>>();

    let mut state = ListState::default().with_selected(if filtered.is_empty() {
        None
    } else {
        Some(app.selected_check.min(filtered.len().saturating_sub(1)))
    });
    frame.render_stateful_widget(
        List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Checks")
                    .border_style(accent_style(app.interaction_strength(now))),
            )
            .highlight_style(selection_style(app.interaction_strength(now))),
        layout.0,
        &mut state,
    );

    if let Some(detail_area) = layout.1 {
        frame.render_widget(Clear, detail_area);
        if let Some(index) = filtered.get(app.selected_check) {
            let check = &app.report.checks[*index];
            let lines = vec![
                Line::from(format!("Status: {}", check.status.as_str())),
                Line::from(format!("Category: {}", check.category.as_str())),
                Line::from(format!("Rule: {}", check.id)),
                Line::from(format!("Coverage: {}%", check.coverage)),
                Line::from(format!("Gap: -{}", check.gap)),
                Line::from(format!("Fixability: {}", check.fixability.as_str())),
                Line::from(format!(
                    "Blocking: {}",
                    if check.blocking { "yes" } else { "no" }
                )),
                Line::from(format!(
                    "Location: {}",
                    check
                        .closest_context
                        .clone()
                        .unwrap_or_else(|| String::from("n/a"))
                )),
                Line::from(String::new()),
                Line::from(check.message.clone()),
                Line::from(String::new()),
                Line::from(format!(
                    "Primary cause: {}",
                    check
                        .primary_cause
                        .clone()
                        .unwrap_or_else(|| String::from("n/a"))
                )),
                Line::from(format!(
                    "Why this failed: {}",
                    check
                        .primary_cause_detail
                        .clone()
                        .or_else(|| check.strongest_contradiction.clone())
                        .unwrap_or_else(|| String::from("No causal contradiction was isolated."))
                )),
                Line::from(format!(
                    "Strongest proof: {}",
                    check
                        .strongest_proof
                        .clone()
                        .unwrap_or_else(|| String::from(
                            "No strong supporting proof was captured."
                        ))
                )),
                Line::from(format!(
                    "Remediation: {}",
                    if check.fixable {
                        "ossify can scaffold part of this gap"
                    } else {
                        "manual improvement required"
                    }
                )),
                Line::from(format!("Hint: {}", check.hint)),
                Line::from(format!(
                    "Evidence: {}",
                    if check.evidence.is_empty() {
                        String::from("none")
                    } else {
                        check.evidence.join(", ")
                    }
                )),
                Line::from(format!(
                    "Retrieval scope: {}",
                    if check.retrieval_scope.is_empty() {
                        String::from("n/a")
                    } else {
                        check
                            .retrieval_scope
                            .iter()
                            .take(4)
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(", ")
                    }
                )),
                Line::from(format!(
                    "History refs: {}",
                    if check.history_refs.is_empty() {
                        String::from("n/a")
                    } else {
                        check
                            .history_refs
                            .iter()
                            .take(3)
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(", ")
                    }
                )),
            ];
            frame.render_widget(
                Paragraph::new(lines)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Detail")
                            .border_style(accent_style(app.interaction_strength(now))),
                    )
                    .wrap(Wrap { trim: true }),
                detail_area,
            );
        }
    }
}

fn render_diagnostics(frame: &mut Frame<'_>, app: &ExplorerApp, area: Rect, now: Instant) {
    let filtered = app.filtered_diagnostics();
    let layout = split_detail(area, app.detail_open);
    let items = filtered
        .iter()
        .map(|index| {
            let diagnostic = &app.report.diagnostics[*index];
            ListItem::new(format!(
                "{} | {} | {}",
                diagnostic.severity.as_str(),
                diagnostic.rule_label,
                diagnostic.message
            ))
        })
        .collect::<Vec<_>>();
    let mut state = ListState::default().with_selected(if filtered.is_empty() {
        None
    } else {
        Some(
            app.selected_diagnostic
                .min(filtered.len().saturating_sub(1)),
        )
    });
    frame.render_stateful_widget(
        List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Diagnostics")
                    .border_style(accent_style(app.interaction_strength(now))),
            )
            .highlight_style(selection_style(app.interaction_strength(now))),
        layout.0,
        &mut state,
    );

    if let Some(detail_area) = layout.1 {
        frame.render_widget(Clear, detail_area);
        if let Some(index) = filtered.get(app.selected_diagnostic) {
            let diagnostic = &app.report.diagnostics[*index];
            render_diagnostic_detail(
                frame,
                diagnostic,
                detail_area,
                app.interaction_strength(now),
            );
        }
    }
}

fn render_plan(frame: &mut Frame<'_>, app: &ExplorerApp, area: Rect, now: Instant) {
    let layout = split_detail(area, true);
    let items = app
        .report
        .files
        .iter()
        .map(|file| ListItem::new(format!("{:?} | {}", file.action, file.path)))
        .collect::<Vec<_>>();
    let mut state = ListState::default().with_selected(if app.report.files.is_empty() {
        None
    } else {
        Some(
            app.selected_file
                .min(app.report.files.len().saturating_sub(1)),
        )
    });
    frame.render_stateful_widget(
        List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Plan")
                    .border_style(accent_style(app.interaction_strength(now))),
            )
            .highlight_style(selection_style(app.interaction_strength(now))),
        layout.0,
        &mut state,
    );

    if let Some(detail_area) = layout.1 {
        frame.render_widget(Clear, detail_area);
        let file = app.report.files.get(app.selected_file);
        let lines = match file {
            Some(file) => vec![
                Line::from(format!("Action: {:?}", file.action)),
                Line::from(format!("Path: {}", file.path)),
                Line::from(format!(
                    "Reason: {}",
                    file.reason
                        .clone()
                        .unwrap_or_else(|| String::from("ready to apply"))
                )),
                Line::from(String::new()),
                Line::from(format!("Estimated score: {}", app.displayed_score(now))),
            ],
            None => vec![Line::from("No plan items.")],
        };
        frame.render_widget(
            Paragraph::new(lines)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Detail")
                        .border_style(accent_style(app.interaction_strength(now))),
                )
                .wrap(Wrap { trim: true }),
            detail_area,
        );
    }
}

fn render_diagnostic_detail(
    frame: &mut Frame<'_>,
    diagnostic: &UiDiagnostic,
    area: Rect,
    interaction_strength: f32,
) {
    let lines = vec![
        Line::from(format!("Severity: {}", diagnostic.severity.as_str())),
        Line::from(format!("Category: {}", diagnostic.category.as_str())),
        Line::from(format!("Rule: {}", diagnostic.rule_id)),
        Line::from(String::new()),
        Line::from(diagnostic.message.clone()),
        Line::from(String::new()),
        Line::from(format!("Help: {}", diagnostic.help)),
        Line::from(format!(
            "Primary cause: {}",
            diagnostic
                .primary_cause
                .clone()
                .unwrap_or_else(|| String::from("n/a"))
        )),
        Line::from(format!(
            "Strongest contradiction: {}",
            diagnostic
                .strongest_contradiction
                .clone()
                .unwrap_or_else(|| String::from("n/a"))
        )),
        Line::from(format!(
            "Strongest proof: {}",
            diagnostic
                .strongest_proof
                .clone()
                .unwrap_or_else(|| String::from("n/a"))
        )),
        Line::from(format!(
            "Evidence: {}",
            if diagnostic.evidence.is_empty() {
                String::from("none")
            } else {
                diagnostic.evidence.join(", ")
            }
        )),
        Line::from(format!(
            "Location: {}",
            diagnostic
                .closest_context
                .clone()
                .or_else(|| diagnostic.location.clone())
                .unwrap_or_else(|| String::from("n/a"))
        )),
        Line::from(format!("Impact: {:.2}", diagnostic.impact)),
        Line::from(format!(
            "Reported at: {}",
            diagnostic
                .location
                .clone()
                .unwrap_or_else(|| String::from("n/a"))
        )),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Detail")
                    .border_style(accent_style(interaction_strength)),
            )
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_footer(frame: &mut Frame<'_>, app: &ExplorerApp, area: Rect) {
    let footer = Paragraph::new(format!(
        "Tab views | Up/Down navigate | Enter detail | f status={} | c category={} | q quit",
        app.status_filter.label(),
        app.category_filter
            .map(|category| category.as_str())
            .unwrap_or("all")
    ))
    .style(Style::default().fg(Color::Gray))
    .wrap(Wrap { trim: true });
    frame.render_widget(footer, area);
}

fn split_detail(area: Rect, detail_open: bool) -> (Rect, Option<Rect>) {
    if !detail_open {
        return (area, None);
    }
    let split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(48), Constraint::Percentage(52)])
        .split(area);
    (split[0], Some(split[1]))
}

fn next_category(current: Option<RuleCategory>) -> Option<RuleCategory> {
    match current {
        None => Some(RuleCategory::Identity),
        Some(RuleCategory::Identity) => Some(RuleCategory::Docs),
        Some(RuleCategory::Docs) => Some(RuleCategory::Community),
        Some(RuleCategory::Community) => Some(RuleCategory::Automation),
        Some(RuleCategory::Automation) => Some(RuleCategory::Release),
        Some(RuleCategory::Release) => None,
    }
}

fn next_index(current: usize, len: usize, delta: isize) -> usize {
    if len == 0 {
        return 0;
    }
    if delta.is_negative() {
        current.saturating_sub(delta.unsigned_abs())
    } else {
        (current + delta as usize).min(len - 1)
    }
}

fn view_label(view: ActiveView) -> &'static str {
    match view {
        ActiveView::Overview => "Overview",
        ActiveView::Checks => "Checks",
        ActiveView::Diagnostics => "Diagnostics",
        ActiveView::Plan => "Plan",
    }
}

fn score_style(score: u8, interaction_strength: f32) -> Style {
    let color = if score >= 85 {
        if interaction_strength > 0.45 {
            Color::Cyan
        } else {
            Color::Green
        }
    } else if score >= 60 {
        if interaction_strength > 0.45 {
            Color::White
        } else {
            Color::Yellow
        }
    } else {
        Color::Red
    };

    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

fn accent_style(interaction_strength: f32) -> Style {
    Style::default()
        .fg(accent_color(interaction_strength))
        .add_modifier(Modifier::BOLD)
}

fn accent_color(interaction_strength: f32) -> Color {
    if interaction_strength > 0.65 {
        Color::White
    } else if interaction_strength > 0.2 {
        Color::Cyan
    } else {
        Color::Blue
    }
}

fn selection_style(interaction_strength: f32) -> Style {
    let background = if interaction_strength > 0.45 {
        Color::Blue
    } else {
        Color::DarkGray
    };

    Style::default()
        .bg(background)
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

fn progress_bar(score: u8, width: usize) -> String {
    let filled = ((score as usize * width) + 50) / 100;
    format!(
        "{}{}",
        "=".repeat(filled),
        ".".repeat(width.saturating_sub(filled))
    )
}

fn ease_out_cubic(progress: f32) -> f32 {
    1.0 - (1.0 - progress).powi(3)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::OssifyConfig;
    use crate::generator::{plan_fix_repository, InitOptions, LicenseKind};
    use crate::ui::model::UiReport;
    use ratatui::backend::TestBackend;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_TEMP_ID: AtomicUsize = AtomicUsize::new(0);

    fn temp_repo(name: &str) -> PathBuf {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("{name}-{id}"));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp directory");
        path
    }

    fn sample_plan_app() -> ExplorerApp {
        let root = temp_repo("ossify-ui-tui");
        fs::create_dir_all(root.join("src")).expect("create src");
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
        )
        .expect("write manifest");
        fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("write main");
        let plan = plan_fix_repository(
            &root,
            &InitOptions {
                overwrite: false,
                license: LicenseKind::Mit,
                owner: String::from("Open Source Maintainers"),
                funding: None,
            },
            &OssifyConfig::default(),
        )
        .expect("plan");
        let _ = fs::remove_dir_all(&root);
        ExplorerApp::new(UiReport::from_plan(&plan))
    }

    fn render_text(app: &ExplorerApp) -> String {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| draw_app(frame, app))
            .expect("draw app");
        let backend = terminal.backend();
        let buffer = backend.buffer();
        let mut out = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                out.push_str(buffer[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    fn key_event(code: KeyCode, kind: KeyEventKind) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: crossterm::event::KeyModifiers::NONE,
            kind,
            state: crossterm::event::KeyEventState::NONE,
        }
    }

    #[test]
    fn interactive_support_rejects_dumb_terminal() {
        assert!(!interactive_supported(true, Some(String::from("dumb"))));
        assert!(!interactive_supported(
            false,
            Some(String::from("xterm-256color"))
        ));
        assert!(interactive_supported(
            true,
            Some(String::from("xterm-256color"))
        ));
    }

    #[test]
    fn tab_cycles_views() {
        let mut app = sample_plan_app();
        assert_eq!(app.active_view(), ActiveView::Overview);
        assert!(!app.handle_key(KeyEvent::from(KeyCode::Tab)));
        assert_eq!(app.active_view(), ActiveView::Checks);
        assert!(!app.handle_key(KeyEvent::from(KeyCode::BackTab)));
        assert_eq!(app.active_view(), ActiveView::Overview);
    }

    #[test]
    fn filters_cycle() {
        let mut app = sample_plan_app();
        app.handle_key(KeyEvent::from(KeyCode::Char('f')));
        assert_eq!(app.status_filter(), StatusFilter::Strong);
        app.handle_key(KeyEvent::from(KeyCode::Char('c')));
        assert_eq!(app.category_filter(), Some(RuleCategory::Identity));
    }

    #[test]
    fn enter_toggles_detail_and_escape_quits() {
        let mut app = sample_plan_app();
        app.handle_key(KeyEvent::from(KeyCode::Enter));
        assert!(app.detail_open());
        assert!(app.handle_key(KeyEvent::from(KeyCode::Esc)));
    }

    #[test]
    fn repeated_navigation_is_throttled() {
        let mut app = sample_plan_app();
        let start = Instant::now();

        assert!(!app.handle_key_at(key_event(KeyCode::Tab, KeyEventKind::Press), start));
        assert_eq!(app.active_view(), ActiveView::Checks);

        assert!(!app.handle_key_at(key_event(KeyCode::Down, KeyEventKind::Press), start));
        assert_eq!(app.selected_check, 1);

        assert!(!app.handle_key_at(
            key_event(KeyCode::Down, KeyEventKind::Repeat),
            start + Duration::from_millis(30)
        ));
        assert_eq!(app.selected_check, 1);

        assert!(!app.handle_key_at(
            key_event(KeyCode::Down, KeyEventKind::Repeat),
            start + NAV_REPEAT_INTERVAL
        ));
        assert_eq!(app.selected_check, 2);
    }

    #[test]
    fn score_animation_eases_in() {
        let app = sample_plan_app();
        let early = app.displayed_score(app.started_at + Duration::from_millis(50));
        let settled = app.displayed_score(app.started_at + ENTRY_ANIMATION);

        assert!(early <= settled);
        assert_eq!(settled, app.report.current.score);
    }

    #[test]
    fn rendered_tui_contains_tabs_and_footer() {
        let app = sample_plan_app();
        let rendered = render_text(&app);
        assert!(rendered.contains("Overview"));
        assert!(rendered.contains("Checks"));
        assert!(rendered.contains("Diagnostics"));
        assert!(rendered.contains("Plan"));
        assert!(rendered.contains("Tab views"));
    }
}
