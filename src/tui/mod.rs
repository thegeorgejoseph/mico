use std::{
    cmp::Ordering,
    io::{self, Stdout},
    path::PathBuf,
    process::Command,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};
use uuid::Uuid;

use crate::{
    app::runtime::{LaunchMode, MicoRuntime},
    domain::model::{Workstream, WorkstreamAttention, WorkstreamRequest, WorkstreamStatus},
};

const MICO_BANNER: [&str; 4] = [
    "_ __ ___  _  ___ ___",
    "| '_ ` _ \\| |/ __/ _ \\",
    "| | | | | | | (_| (_) |",
    "|_| |_| |_|_|\\___\\___/",
];

const PURPLE_A: Color = Color::Rgb(204, 88, 255);
const PURPLE_B: Color = Color::Rgb(228, 72, 255);
const PURPLE_C: Color = Color::Rgb(250, 58, 255);
const PANEL_HIGHLIGHT: Color = Color::Rgb(22, 33, 46);
const SUCCESS_GREEN: Color = Color::Rgb(92, 214, 137);
const WARNING_AMBER: Color = Color::Rgb(255, 193, 94);
const ERROR_RED: Color = Color::Rgb(255, 107, 107);
const INFO_BLUE: Color = Color::Rgb(104, 180, 255);
const MUTED_TEXT: Color = Color::Rgb(145, 152, 168);

pub fn run_dashboard(runtime: MicoRuntime) -> anyhow::Result<()> {
    let mut terminal = initialize_terminal()?;
    let mut app = DashboardApp::new(runtime);

    let result = dashboard_loop(&mut terminal, &mut app);

    restore_terminal(&mut terminal)?;

    result
}

fn dashboard_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut DashboardApp,
) -> anyhow::Result<()> {
    loop {
        terminal.draw(|frame| app.render(frame))?;

        if event::poll(Duration::from_millis(250))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            match app.handle_key(key)? {
                DashboardAction::None => {}
                DashboardAction::Quit => return Ok(()),
                DashboardAction::Attach(workstream_id) => {
                    suspend_terminal(terminal)?;
                    let result = app.runtime.attach_workstream(workstream_id);
                    resume_terminal(terminal)?;
                    app.set_status_from_result(
                        result.map(|_| "Detached from workstream.".to_string()),
                    );
                }
            }
        }
    }
}

fn initialize_terminal() -> anyhow::Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;
    terminal.clear()?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> anyhow::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn suspend_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> anyhow::Result<()> {
    restore_terminal(terminal)
}

fn resume_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> anyhow::Result<()> {
    *terminal = initialize_terminal()?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusPane {
    Repos,
    Workstreams,
}

impl FocusPane {
    fn label(self) -> &'static str {
        match self {
            Self::Repos => "repos",
            Self::Workstreams => "workstreams",
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum DashboardAction {
    None,
    Quit,
    Attach(Uuid),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkstreamView {
    All,
    NeedsAttention,
    Running,
    Stopped,
    Pinned,
}

impl WorkstreamView {
    fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::NeedsAttention => "attention",
            Self::Running => "running",
            Self::Stopped => "stopped",
            Self::Pinned => "pinned",
        }
    }

    fn cycle(self, delta: isize) -> Self {
        let options = [
            Self::All,
            Self::NeedsAttention,
            Self::Running,
            Self::Stopped,
            Self::Pinned,
        ];
        let index = options
            .iter()
            .position(|candidate| *candidate == self)
            .unwrap_or(0);
        let len = options.len();
        let movement = delta.unsigned_abs() % len;
        let next = if delta.is_negative() {
            (index + len - movement) % len
        } else {
            (index + movement) % len
        };
        options[next]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkstreamSort {
    Attention,
    Recent,
}

impl WorkstreamSort {
    fn label(self) -> &'static str {
        match self {
            Self::Attention => "attention",
            Self::Recent => "recent",
        }
    }

    fn cycle(self) -> Self {
        match self {
            Self::Attention => Self::Recent,
            Self::Recent => Self::Attention,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum StatusTone {
    Neutral,
    Good,
    Error,
}

#[derive(Debug, Clone)]
struct StatusMessage {
    text: String,
    tone: StatusTone,
}

#[derive(Debug, Clone)]
struct AppVitals {
    pid: u32,
    cpu_pct: f32,
    rss_kb: u64,
    clock_label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuitKey {
    Esc,
    Q,
}

#[derive(Debug, Clone, Copy)]
enum PaletteCommand {
    AddRepo,
    CreateWorkstream,
    OpenInVscode,
    RefreshRepo,
    RemoveRepo,
    OpenWorkstream,
    AttachWorkstream,
    ResumeWorkstream,
    StopWorkstream,
    RemoveWorkstream,
    RefreshDoctor,
}

#[derive(Debug, Clone, Copy)]
struct PaletteEntry {
    command: PaletteCommand,
    title: &'static str,
    detail: &'static str,
}

const PALETTE_ENTRIES: [PaletteEntry; 11] = [
    PaletteEntry {
        command: PaletteCommand::AddRepo,
        title: "Add repo",
        detail: "Track another repository from a filesystem path.",
    },
    PaletteEntry {
        command: PaletteCommand::CreateWorkstream,
        title: "Create workstream",
        detail: "Use the selected repo to create a new or existing branch worktree.",
    },
    PaletteEntry {
        command: PaletteCommand::OpenInVscode,
        title: "Open selection in VS Code",
        detail: "Open the selected repo or workstream directory with `code`.",
    },
    PaletteEntry {
        command: PaletteCommand::RefreshRepo,
        title: "Refresh selected repo",
        detail: "Fetch the latest refs for the selected repository.",
    },
    PaletteEntry {
        command: PaletteCommand::RemoveRepo,
        title: "Remove selected repo",
        detail: "Untrack the selected repository after its workstreams are gone.",
    },
    PaletteEntry {
        command: PaletteCommand::OpenWorkstream,
        title: "Open selected workstream",
        detail: "Open it in this terminal. Detach with Ctrl-b d to return to mico.",
    },
    PaletteEntry {
        command: PaletteCommand::AttachWorkstream,
        title: "Open selected workstream in new tab",
        detail: "Open the selected workstream in a new iTerm tab.",
    },
    PaletteEntry {
        command: PaletteCommand::ResumeWorkstream,
        title: "Resume selected workstream",
        detail: "Recreate a tmux session in the saved worktree and mark it running again.",
    },
    PaletteEntry {
        command: PaletteCommand::StopWorkstream,
        title: "Stop selected workstream",
        detail: "Kill the tmux session but keep the worktree and local record.",
    },
    PaletteEntry {
        command: PaletteCommand::RemoveWorkstream,
        title: "Remove selected workstream",
        detail: "Remove the selected workstream. Managed worktrees are deleted; linked checkouts are untracked.",
    },
    PaletteEntry {
        command: PaletteCommand::RefreshDoctor,
        title: "Refresh doctor",
        detail: "Re-check tmux, iTerm, git, and local paths.",
    },
];

#[derive(Debug, Default, Clone)]
struct PaletteStateModel {
    query: String,
    selected: usize,
}

#[derive(Debug, Clone)]
enum Modal {
    AddRepo(AddRepoModal),
    Confirm(ConfirmModal),
    CreateWorkstream(CreateWorkstreamFlow),
    WorkstreamFilter(WorkstreamFilterModal),
    Triage(TriageModal),
}

#[derive(Debug, Clone)]
struct AddRepoModal {
    input: String,
}

#[derive(Debug, Clone)]
struct WorkstreamFilterModal {
    query: String,
    original_query: String,
}

#[derive(Debug, Clone)]
struct ConfirmModal {
    title: String,
    body: String,
    action: ConfirmAction,
}

#[derive(Debug, Clone)]
struct TriageModal {
    workstream_id: Uuid,
    branch: String,
    pinned: bool,
    selected: usize,
}

#[derive(Debug, Clone, Copy)]
enum TriageAction {
    ReviewNext,
    Blocked,
    TogglePin,
    Clear,
}

#[derive(Debug, Clone, Copy)]
enum ConfirmAction {
    RemoveRepo(Uuid),
    RemoveWorkstream(Uuid),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CreateStep {
    BranchMode,
    BaseBranch,
    ExistingBranch,
    NewBranch,
    Agent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CreateBranchKind {
    New,
    Existing,
}

#[derive(Debug, Clone)]
struct CreateWorkstreamFlow {
    repo_id: Uuid,
    repo_name: String,
    branches: Vec<String>,
    base_filter: String,
    base_selected: usize,
    branch_mode_selected: usize,
    existing_filter: String,
    existing_selected: usize,
    new_branch_input: String,
    agent_selected: usize,
    selected_base_branch: Option<String>,
    selected_existing_branch: Option<String>,
    branch_kind: Option<CreateBranchKind>,
    step: CreateStep,
}

impl CreateWorkstreamFlow {
    fn new(repo_id: Uuid, repo_name: String, branches: Vec<String>) -> Self {
        let sorted_branches = prioritize_branches(branches);
        let base_selected = preferred_branch_index(&sorted_branches);

        Self {
            repo_id,
            repo_name,
            branches: sorted_branches,
            base_filter: String::new(),
            base_selected,
            branch_mode_selected: 0,
            existing_filter: String::new(),
            existing_selected: 0,
            new_branch_input: String::new(),
            agent_selected: 0,
            selected_base_branch: None,
            selected_existing_branch: None,
            branch_kind: None,
            step: CreateStep::BranchMode,
        }
    }
}

struct DashboardApp {
    runtime: MicoRuntime,
    focus: FocusPane,
    repo_state: ListState,
    workstream_state: ListState,
    workstream_view: WorkstreamView,
    workstream_sort: WorkstreamSort,
    workstream_filter: String,
    palette: Option<PaletteStateModel>,
    modal: Option<Modal>,
    status: Option<StatusMessage>,
    pending_quit: Option<(QuitKey, Instant)>,
    app_vitals: Option<AppVitals>,
    last_vitals_refresh: Option<Instant>,
    dashboard_started_at: Instant,
    recent_output_workstream_id: Option<Uuid>,
    recent_output_lines: Vec<String>,
    last_output_refresh: Option<Instant>,
}

impl DashboardApp {
    fn new(mut runtime: MicoRuntime) -> Self {
        let _ = runtime.refresh_doctor();
        let mut repo_state = ListState::default();
        let mut workstream_state = ListState::default();

        if !runtime.state.repos.is_empty() {
            repo_state.select(Some(0));
        }

        if !runtime.state.workstreams.is_empty() {
            workstream_state.select(Some(0));
        }

        Self {
            runtime,
            focus: FocusPane::Repos,
            repo_state,
            workstream_state,
            workstream_view: WorkstreamView::All,
            workstream_sort: WorkstreamSort::Attention,
            workstream_filter: String::new(),
            palette: None,
            modal: None,
            status: None,
            pending_quit: None,
            app_vitals: None,
            last_vitals_refresh: None,
            dashboard_started_at: Instant::now(),
            recent_output_workstream_id: None,
            recent_output_lines: Vec::new(),
            last_output_refresh: None,
        }
    }

    fn render(&mut self, frame: &mut Frame<'_>) {
        self.refresh_app_vitals();
        self.refresh_recent_output();
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(9),
                Constraint::Min(8),
                Constraint::Length(7),
            ])
            .split(frame.area());

        self.render_header(frame, layout[0]);
        self.render_body(frame, layout[1]);
        self.render_footer(frame, layout[2]);

        if let Some(palette) = &self.palette {
            self.render_palette(frame, palette);
        }

        if let Some(modal) = &self.modal {
            self.render_modal(frame, modal);
        }
    }

    fn render_header(&self, frame: &mut Frame<'_>, area: Rect) {
        let header = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(34), Constraint::Min(30)])
            .split(area);

        let banner_block = Block::default()
            .borders(Borders::ALL)
            .title("Mission Control");
        let banner_inner = banner_block.inner(header[0]);
        let banner_lines = MICO_BANNER
            .iter()
            .enumerate()
            .map(|(index, line)| {
                let color = match index {
                    0 => PURPLE_A,
                    1 => PURPLE_B,
                    2 => PURPLE_C,
                    _ => PURPLE_B,
                };
                Line::styled(
                    *line,
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                )
            })
            .collect::<Vec<_>>();
        let banner_height = u16::try_from(banner_lines.len()).unwrap_or(u16::MAX);
        let banner_area = Rect {
            x: banner_inner.x,
            y: banner_inner
                .y
                .saturating_add(banner_inner.height.saturating_sub(banner_height) / 2),
            width: banner_inner.width,
            height: banner_height.min(banner_inner.height),
        };
        let banner = Paragraph::new(Text::from(banner_lines)).alignment(Alignment::Center);

        let dependencies = self
            .runtime
            .report
            .dependencies
            .iter()
            .map(|item| {
                let marker = if item.found { "ok" } else { "missing" };
                format!("{}: {}", item.name, marker)
            })
            .collect::<Vec<_>>()
            .join("   ");
        let attention_count = self
            .runtime
            .state
            .workstreams
            .iter()
            .filter(|workstream| {
                workstream.pinned
                    || !matches!(workstream.attention, WorkstreamAttention::None)
                    || matches!(workstream.status, WorkstreamStatus::Stopped)
            })
            .count();
        let running_count = self
            .runtime
            .state
            .workstreams
            .iter()
            .filter(|workstream| matches!(workstream.status, WorkstreamStatus::Running))
            .count();
        let footer = Paragraph::new(self.flight_deck_line(attention_count, running_count))
            .wrap(Wrap { trim: false })
            .alignment(Alignment::Left);

        let flight_block = Block::default().borders(Borders::ALL).title("Flight Deck");
        let flight_inner = flight_block.inner(header[1]);
        let flight_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(1)])
            .split(flight_inner);
        let title = Paragraph::new(Text::from(vec![
            Line::styled(
                "Mission Control for parallel agents",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Line::from("Fast path: add a repo, hit Enter, pick a branch, launch an agent."),
            Line::from(
                "tmux sessions keep running when you quit the dashboard; mico is just the control plane.",
            ),
            Line::styled(dependencies, Style::default().fg(Color::Gray)),
        ]))
        .wrap(Wrap { trim: false });
        frame.render_widget(banner_block, header[0]);
        frame.render_widget(banner, banner_area);
        frame.render_widget(flight_block, header[1]);
        frame.render_widget(title, flight_chunks[0]);
        frame.render_widget(footer, flight_chunks[1]);
    }

    fn render_body(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(area);

        let repo_items: Vec<ListItem<'_>> = if self.runtime.state.repos.is_empty() {
            vec![ListItem::new(
                "No repos tracked yet. Press : and choose Add repo.",
            )]
        } else {
            self.runtime
                .state
                .repos
                .iter()
                .map(|repo| {
                    ListItem::new(vec![
                        Line::styled(
                            repo.display_name.clone(),
                            Style::default().add_modifier(Modifier::BOLD),
                        ),
                        Line::from(repo.path.display().to_string()),
                    ])
                })
                .collect()
        };

        let repo_block = Block::default()
            .borders(Borders::ALL)
            .title("Repos")
            .border_style(if self.focus == FocusPane::Repos {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            });

        let repo_list = List::new(repo_items)
            .block(repo_block)
            .highlight_style(
                Style::default()
                    .bg(PANEL_HIGHLIGHT)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");
        frame.render_stateful_widget(repo_list, chunks[0], &mut self.repo_state);

        let visible_workstream_ids = self.visible_workstream_ids();
        let selected_workstream_id = self.selected_workstream_id();
        let workstream_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(8), Constraint::Min(6)])
            .split(chunks[1]);

        let workstream_items: Vec<ListItem<'_>> = if visible_workstream_ids.is_empty() {
            vec![ListItem::new(match self.workstream_view {
                WorkstreamView::All => {
                    "No workstreams yet. Select a repo, open the palette, and create one."
                }
                _ => "No workstreams matched the current view.",
            })]
        } else {
            visible_workstream_ids
                .iter()
                .filter_map(|workstream_id| self.runtime.workstream_by_id(*workstream_id).ok())
                .map(|workstream| {
                    let repo_name = self
                        .runtime
                        .state
                        .repos
                        .iter()
                        .find(|repo| repo.id == workstream.repo_id)
                        .map(|repo| repo.display_name.as_str())
                        .unwrap_or("<missing repo>");
                    let last_seen = workstream
                        .last_attached_at_epoch_secs
                        .or(workstream.last_opened_at_epoch_secs);

                    let mut header = vec![Span::styled(
                        workstream.branch.clone(),
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    )];
                    for chip in workstream_chips(workstream) {
                        header.push(Span::raw(" "));
                        header.push(chip);
                    }

                    let mut lines = vec![
                        Line::from(header),
                        line_from_stat_pairs(&[
                            ("repo".to_string(), repo_name.to_string()),
                            ("agent".to_string(), workstream.agent_preset.clone()),
                            (
                                "created".to_string(),
                                elapsed_label(workstream.created_at_epoch_secs),
                            ),
                            (
                                "status".to_string(),
                                elapsed_label(workstream.status_changed_at_epoch_secs),
                            ),
                        ]),
                        line_from_stat_pairs(&[
                            (
                                "last open/attach".to_string(),
                                option_elapsed_label(last_seen),
                            ),
                            ("session".to_string(), workstream.session_name.clone()),
                        ]),
                    ];

                    if selected_workstream_id == Some(workstream.id) {
                        let hint = match workstream.status {
                            WorkstreamStatus::Running => line_from_pairs(&[
                                ("Enter/o", "open here"),
                                ("a", "new tab"),
                                ("t", "triage"),
                                ("x", "stop"),
                            ]),
                            WorkstreamStatus::Stopped => line_from_pairs(&[
                                ("Enter/o", "resume here"),
                                ("a", "resume in new tab"),
                                ("t", "triage"),
                            ]),
                        };
                        lines.push(hint);
                    }

                    ListItem::new(lines)
                })
                .collect()
        };

        let workstream_title = format!(
            "Workstreams  view:{}  sort:{}{}",
            self.workstream_view.label(),
            self.workstream_sort.label(),
            if self.workstream_filter.trim().is_empty() {
                String::new()
            } else {
                format!("  filter:{}", self.workstream_filter)
            }
        );
        let workstream_block = Block::default()
            .borders(Borders::ALL)
            .title(workstream_title)
            .border_style(if self.focus == FocusPane::Workstreams {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            });

        let workstream_list = List::new(workstream_items)
            .block(workstream_block)
            .highlight_style(
                Style::default()
                    .bg(PANEL_HIGHLIGHT)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");
        self.render_workstream_signal(frame, workstream_chunks[0], &visible_workstream_ids);
        frame.render_stateful_widget(
            workstream_list,
            workstream_chunks[1],
            &mut self.workstream_state,
        );
    }

    fn render_workstream_signal(
        &self,
        frame: &mut Frame<'_>,
        area: Rect,
        visible_workstream_ids: &[Uuid],
    ) {
        let workstreams = visible_workstream_ids
            .iter()
            .filter_map(|workstream_id| self.runtime.workstream_by_id(*workstream_id).ok())
            .collect::<Vec<_>>();

        let blocked = workstreams
            .iter()
            .filter(|workstream| matches!(workstream.attention, WorkstreamAttention::Blocked))
            .count();
        let review = workstreams
            .iter()
            .filter(|workstream| matches!(workstream.attention, WorkstreamAttention::ReviewNext))
            .count();
        let stopped = workstreams
            .iter()
            .filter(|workstream| matches!(workstream.status, WorkstreamStatus::Stopped))
            .count();
        let running = workstreams.len().saturating_sub(stopped);
        let selected_line = self
            .selected_workstream()
            .map(|workstream| {
                format!(
                    "selected {}   state {}   last touch {}",
                    workstream.branch,
                    workstream_state_label(workstream),
                    option_elapsed_label(
                        workstream
                            .last_attached_at_epoch_secs
                            .or(workstream.last_opened_at_epoch_secs)
                    )
                )
            })
            .unwrap_or_else(|| "select a workstream to inspect its current signal.".to_string());
        let block = Block::default().borders(Borders::ALL).title("Pulse");
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.height == 0 {
            return;
        }

        let summary_height = inner.height.min(2);
        let output_height = inner.height.saturating_sub(summary_height);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(summary_height),
                Constraint::Min(output_height),
            ])
            .split(inner);

        let summary_lines = if workstreams.is_empty() {
            vec![
                Line::styled(
                    "No workstreams in this view yet.",
                    Style::default().fg(MUTED_TEXT),
                ),
                Line::from(""),
            ]
        } else {
            vec![
                Line::from(format!(
                    "blocked {}   review {}   running {}   stopped {}",
                    blocked, review, running, stopped
                )),
                Line::styled(selected_line, Style::default().fg(MUTED_TEXT)),
            ]
        };
        frame.render_widget(
            Paragraph::new(Text::from(summary_lines)).wrap(Wrap { trim: false }),
            chunks[0],
        );

        let output_lines = if self.recent_output_lines.is_empty() {
            vec![Line::styled(
                "out waiting for pane output",
                Style::default().fg(MUTED_TEXT),
            )]
        } else {
            self.recent_output_lines
                .iter()
                .map(|line| Line::styled(format!("out {line}"), Style::default().fg(MUTED_TEXT)))
                .collect::<Vec<_>>()
        };
        frame.render_widget(
            Paragraph::new(Text::from(output_lines)).wrap(Wrap { trim: false }),
            chunks[1],
        );
    }

    fn flight_deck_line(&self, attention_count: usize, running_count: usize) -> Line<'static> {
        let orbit = ["north", "east", "south", "west"];
        let orbit_len = u64::try_from(orbit.len()).unwrap_or(1);
        let orbit_index =
            usize::try_from(self.dashboard_started_at.elapsed().as_secs() / 4 % orbit_len)
                .unwrap_or(0);
        let dependencies_ok = self
            .runtime
            .report
            .dependencies
            .iter()
            .filter(|dependency| dependency.found)
            .count();

        let clock = self
            .app_vitals
            .as_ref()
            .map(|vitals| vitals.clock_label.clone())
            .unwrap_or_else(|| "?".to_string());
        let mut pairs = vec![("time".to_string(), clock)];

        match self.dashboard_started_at.elapsed().as_secs() / 4 % 4 {
            0 => {
                let cpu = self
                    .app_vitals
                    .as_ref()
                    .map(|vitals| format!("{:.1}%", vitals.cpu_pct))
                    .unwrap_or_else(|| "?".to_string());
                let mem = self
                    .app_vitals
                    .as_ref()
                    .map(|vitals| format_kb(vitals.rss_kb))
                    .unwrap_or_else(|| "?".to_string());
                let pid = self
                    .app_vitals
                    .as_ref()
                    .map(|vitals| vitals.pid.to_string())
                    .unwrap_or_else(|| "?".to_string());
                pairs.push(("cpu".to_string(), cpu));
                pairs.push(("mem".to_string(), mem));
                pairs.push(("pid".to_string(), pid));
            }
            1 => {
                pairs.push((
                    "repos".to_string(),
                    self.runtime.state.repos.len().to_string(),
                ));
                pairs.push((
                    "workstreams".to_string(),
                    self.runtime.state.workstreams.len().to_string(),
                ));
                pairs.push(("running".to_string(), running_count.to_string()));
                pairs.push(("attention".to_string(), attention_count.to_string()));
            }
            2 => {
                if let Some(workstream) = self.selected_workstream() {
                    pairs.push(("selected".to_string(), workstream.branch.clone()));
                    pairs.push((
                        "state".to_string(),
                        workstream_state_label(workstream).to_string(),
                    ));
                    pairs.push((
                        "touch".to_string(),
                        option_elapsed_label(
                            workstream
                                .last_attached_at_epoch_secs
                                .or(workstream.last_opened_at_epoch_secs),
                        ),
                    ));
                } else {
                    pairs.push(("focus".to_string(), self.focus.label().to_string()));
                    pairs.push(("view".to_string(), self.workstream_view.label().to_string()));
                    pairs.push(("sort".to_string(), self.workstream_sort.label().to_string()));
                }
            }
            _ => {
                pairs.push(("orbit".to_string(), orbit[orbit_index].to_string()));
                pairs.push((
                    "uptime".to_string(),
                    format_duration(self.dashboard_started_at.elapsed().as_secs()),
                ));
                pairs.push((
                    "doctor".to_string(),
                    format!(
                        "{}/{}",
                        dependencies_ok,
                        self.runtime.report.dependencies.len()
                    ),
                ));
            }
        }

        line_from_stat_pairs(&pairs)
    }

    fn render_footer(&self, frame: &mut Frame<'_>, area: Rect) {
        let selected_repo = self.selected_repo().map(|repo| {
            format!(
                "selected repo: {}  ({})",
                repo.display_name,
                repo.path.display()
            )
        });
        let selected_workstream = self.selected_workstream().map(|workstream| {
            format!(
                "selected workstream: {}  -> {}",
                workstream.branch,
                workstream.worktree_path.display()
            )
        });
        let status = self.status.clone().unwrap_or(StatusMessage {
            text: "Ready.".to_string(),
            tone: StatusTone::Neutral,
        });
        let context_line = match self.focus {
            FocusPane::Repos => line_from_pairs(&[
                ("Enter", "create workstream"),
                ("v", "open in code"),
                (":", "commands"),
            ]),
            FocusPane::Workstreams => line_from_pairs(&[
                ("Enter", "open here"),
                ("o", "open here"),
                ("a", "open new tab"),
                ("v", "open in code"),
                ("/", "filter"),
                ("t", "triage"),
                ("[/]", "views"),
                ("s", "sort"),
                ("x", "stop"),
                (":", "commands"),
            ]),
        };
        let global_line = line_from_pairs(&[
            ("Tab", "switch panes"),
            ("j/k", "move"),
            ("Esc Esc", "quit dashboard"),
            ("q q", "quit dashboard"),
            (":", "command palette"),
        ]);

        let info = Paragraph::new(Text::from(vec![
            Line::styled(
                format!("status: {}", status.text),
                status_style(status.tone),
            ),
            Line::from(selected_repo.unwrap_or_else(|| "selected repo: none".to_string())),
            Line::from(
                selected_workstream.unwrap_or_else(|| "selected workstream: none".to_string()),
            ),
            context_line,
            global_line,
        ]))
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Command Deck")
                .border_style(Style::default().fg(PURPLE_B)),
        );

        frame.render_widget(info, area);
    }

    fn render_palette(&self, frame: &mut Frame<'_>, palette: &PaletteStateModel) {
        let area = centered_rect(72, 42, frame.area());
        frame.render_widget(Clear, area);

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4),
                Constraint::Length(3),
                Constraint::Min(8),
            ])
            .split(area);

        let help = Paragraph::new(Text::from(vec![
            Line::from("Command palette"),
            line_from_pairs(&[
                ("Type", "filter"),
                ("Enter", "run command"),
                ("Esc", "close palette"),
                ("↑↓", "move"),
            ]),
            Line::from(
                "Commands operate on your current repo or workstream selection when relevant.",
            ),
        ]))
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title("Palette"));

        let input = Paragraph::new(format!("> {}", palette.query))
            .block(Block::default().borders(Borders::ALL).title("Search"));

        let filtered = self.filtered_palette_entries(palette);
        let items: Vec<ListItem<'_>> = if filtered.is_empty() {
            vec![ListItem::new("No commands matched your search.")]
        } else {
            filtered
                .iter()
                .map(|entry| {
                    ListItem::new(vec![
                        Line::styled(entry.title, Style::default().add_modifier(Modifier::BOLD)),
                        Line::from(entry.detail),
                    ])
                })
                .collect()
        };

        let mut list_state = ListState::default();
        if !filtered.is_empty() {
            list_state.select(Some(palette.selected.min(filtered.len().saturating_sub(1))));
        }

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Commands"))
            .highlight_style(
                Style::default()
                    .bg(PANEL_HIGHLIGHT)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

        frame.render_widget(help, layout[0]);
        frame.render_widget(input, layout[1]);
        frame.render_stateful_widget(list, layout[2], &mut list_state);
    }

    fn render_modal(&self, frame: &mut Frame<'_>, modal: &Modal) {
        match modal {
            Modal::AddRepo(model) => self.render_add_repo_modal(frame, model),
            Modal::Confirm(model) => self.render_confirm_modal(frame, model),
            Modal::CreateWorkstream(model) => self.render_create_workstream_modal(frame, model),
            Modal::WorkstreamFilter(model) => self.render_workstream_filter_modal(frame, model),
            Modal::Triage(model) => self.render_triage_modal(frame, model),
        }
    }

    fn render_add_repo_modal(&self, frame: &mut Frame<'_>, modal: &AddRepoModal) {
        let area = centered_rect(68, 26, frame.area());
        frame.render_widget(Clear, area);
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4),
                Constraint::Length(3),
                Constraint::Min(3),
            ])
            .split(area);

        let help = Paragraph::new(Text::from(vec![
            Line::from("Add repo"),
            line_from_pairs(&[
                ("Type", "edit path"),
                ("Enter", "track repo"),
                ("Esc", "cancel"),
                ("Backspace", "delete"),
            ]),
        ]))
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title("Repo"));
        let input = Paragraph::new(modal.input.clone())
            .block(Block::default().borders(Borders::ALL).title("Path"));
        let note = Paragraph::new(
            "Tip: the input starts with your current working directory so you can edit from there.",
        )
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title("Hint"));

        frame.render_widget(help, layout[0]);
        frame.render_widget(input, layout[1]);
        frame.render_widget(note, layout[2]);
    }

    fn render_confirm_modal(&self, frame: &mut Frame<'_>, modal: &ConfirmModal) {
        let area = centered_rect(58, 22, frame.area());
        frame.render_widget(Clear, area);

        let body = Paragraph::new(Text::from(vec![
            Line::from(modal.body.clone()),
            Line::from(""),
            line_from_pairs(&[("Enter", "confirm"), ("Esc", "cancel")]),
        ]))
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(modal.title.clone()),
        );

        frame.render_widget(body, area);
    }

    fn render_workstream_filter_modal(&self, frame: &mut Frame<'_>, modal: &WorkstreamFilterModal) {
        let area = centered_rect(58, 22, frame.area());
        frame.render_widget(Clear, area);

        let body = Paragraph::new(Text::from(vec![
            Line::from(format!("filter > {}", modal.query)),
            Line::from(""),
            line_from_pairs(&[
                ("Type", "edit filter"),
                ("Enter", "apply"),
                ("Esc", "cancel"),
                ("Backspace", "delete"),
            ]),
        ]))
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Filter Workstreams"),
        );

        frame.render_widget(body, area);
    }

    fn render_triage_modal(&self, frame: &mut Frame<'_>, modal: &TriageModal) {
        let area = centered_rect(54, 34, frame.area());
        frame.render_widget(Clear, area);
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(5), Constraint::Min(6)])
            .split(area);

        let help = Paragraph::new(Text::from(vec![
            Line::from(format!("Triage `{}`", modal.branch)),
            line_from_pairs(&[
                ("r", "review"),
                ("b", "blocked"),
                ("p", "pin"),
                ("c", "clear"),
                ("Esc", "cancel"),
            ]),
        ]))
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title("Triage"));

        let options = triage_actions(modal)
            .into_iter()
            .map(|action| ListItem::new(triage_action_label(action, modal)))
            .collect::<Vec<_>>();
        let mut state = ListState::default();
        state.select(Some(modal.selected.min(options.len().saturating_sub(1))));

        let list = List::new(options)
            .block(Block::default().borders(Borders::ALL).title("Actions"))
            .highlight_style(
                Style::default()
                    .bg(PANEL_HIGHLIGHT)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

        frame.render_widget(help, layout[0]);
        frame.render_stateful_widget(list, layout[1], &mut state);
    }

    fn render_create_workstream_modal(&self, frame: &mut Frame<'_>, flow: &CreateWorkstreamFlow) {
        let area = centered_rect(74, 58, frame.area());
        frame.render_widget(Clear, area);
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(5),
                Constraint::Length(3),
                Constraint::Min(10),
            ])
            .split(area);

        let summary = format!(
            "Create workstream for {}\nChoose whether to create a new branch from a base branch or use an existing branch directly.",
            flow.repo_name
        );

        let help = Paragraph::new(Text::from(vec![
            Line::from(summary),
            line_from_pairs(&[
                ("↑↓", "move"),
                ("Enter", "confirm step"),
                ("←", "go back"),
                ("Esc", "cancel"),
            ]),
        ]))
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title("Workstream"));

        let step_label = match flow.step {
            CreateStep::BranchMode => "Branch Strategy: new branch or existing branch".to_string(),
            CreateStep::BaseBranch => format!("Base Branch Filter: {}", flow.base_filter),
            CreateStep::ExistingBranch => {
                format!("Existing Branch Filter: {}", flow.existing_filter)
            }
            CreateStep::NewBranch => format!("New Branch Name: {}", flow.new_branch_input),
            CreateStep::Agent => format!(
                "Agent: {}",
                self.runtime
                    .config
                    .agent_presets
                    .get(flow.agent_selected)
                    .map(|preset| preset.name.as_str())
                    .unwrap_or("n/a")
            ),
        };
        let step_bar = Paragraph::new(step_label)
            .block(Block::default().borders(Borders::ALL).title("Current Step"));

        frame.render_widget(help, layout[0]);
        frame.render_widget(step_bar, layout[1]);

        match flow.step {
            CreateStep::BranchMode => {
                let options = vec![
                    "Create a new branch from a base branch".to_string(),
                    "Use an existing branch as the workstream".to_string(),
                ];
                self.render_picker_list(
                    frame,
                    layout[2],
                    "Branch Strategy",
                    &options,
                    flow.branch_mode_selected,
                );
            }
            CreateStep::BaseBranch => {
                self.render_picker_list(
                    frame,
                    layout[2],
                    "Base Branches",
                    &filtered_strings(&flow.branches, &flow.base_filter),
                    flow.base_selected,
                );
            }
            CreateStep::ExistingBranch => {
                self.render_picker_list(
                    frame,
                    layout[2],
                    "Existing Branches",
                    &filtered_strings(&flow.branches, &flow.existing_filter),
                    flow.existing_selected,
                );
            }
            CreateStep::NewBranch => {
                let input = Paragraph::new(flow.new_branch_input.clone())
                    .block(Block::default().borders(Borders::ALL).title("Branch Name"));
                frame.render_widget(input, layout[2]);
            }
            CreateStep::Agent => {
                let options = self
                    .runtime
                    .config
                    .agent_presets
                    .iter()
                    .map(|preset| {
                        if preset.command.trim().is_empty() {
                            format!("{} -> shell", preset.name)
                        } else {
                            format!("{} -> {}", preset.name, preset.command)
                        }
                    })
                    .collect::<Vec<_>>();
                self.render_picker_list(frame, layout[2], "Agents", &options, flow.agent_selected);
            }
        }
    }

    fn render_picker_list(
        &self,
        frame: &mut Frame<'_>,
        area: Rect,
        title: &str,
        items: &[String],
        selected: usize,
    ) {
        let entries: Vec<ListItem<'_>> = if items.is_empty() {
            vec![ListItem::new("No items available.")]
        } else {
            items
                .iter()
                .map(|item| ListItem::new(item.clone()))
                .collect()
        };

        let mut state = ListState::default();
        if !items.is_empty() {
            state.select(Some(selected.min(items.len().saturating_sub(1))));
        }

        let list = List::new(entries)
            .block(Block::default().borders(Borders::ALL).title(title))
            .highlight_style(
                Style::default()
                    .bg(PANEL_HIGHLIGHT)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

        frame.render_stateful_widget(list, area, &mut state);
    }

    fn handle_key(&mut self, key: KeyEvent) -> anyhow::Result<DashboardAction> {
        if self.palette.is_some() {
            self.pending_quit = None;
            return self.handle_palette_key(key);
        }

        if self.modal.is_some() {
            self.pending_quit = None;
            return self.handle_modal_key(key);
        }

        self.handle_dashboard_key(key)
    }

    fn handle_dashboard_key(&mut self, key: KeyEvent) -> anyhow::Result<DashboardAction> {
        if !matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) {
            self.pending_quit = None;
        }

        match key.code {
            KeyCode::Esc => self.confirm_quit(QuitKey::Esc, "Esc"),
            KeyCode::Char('q') => self.confirm_quit(QuitKey::Q, "q"),
            KeyCode::Enter => match self.focus {
                FocusPane::Repos => self.activate_palette_command(PaletteCommand::CreateWorkstream),
                FocusPane::Workstreams => {
                    self.activate_palette_command(PaletteCommand::OpenWorkstream)
                }
            },
            KeyCode::Tab => {
                self.pending_quit = None;
                self.focus = if self.focus == FocusPane::Repos {
                    FocusPane::Workstreams
                } else {
                    FocusPane::Repos
                };
                Ok(DashboardAction::None)
            }
            KeyCode::Char(':') => {
                self.pending_quit = None;
                self.palette = Some(PaletteStateModel::default());
                Ok(DashboardAction::None)
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.pending_quit = None;
                self.move_selection(1);
                Ok(DashboardAction::None)
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.pending_quit = None;
                self.move_selection(-1);
                Ok(DashboardAction::None)
            }
            KeyCode::Char('[') if self.focus == FocusPane::Workstreams => {
                let selected = self.selected_workstream_id();
                self.workstream_view = self.workstream_view.cycle(-1);
                self.sync_workstream_selection();
                if let Some(workstream_id) = selected {
                    self.select_workstream_by_id(workstream_id);
                }
                self.set_status(format!(
                    "Workstream view: {}.",
                    self.workstream_view.label()
                ));
                Ok(DashboardAction::None)
            }
            KeyCode::Char(']') if self.focus == FocusPane::Workstreams => {
                let selected = self.selected_workstream_id();
                self.workstream_view = self.workstream_view.cycle(1);
                self.sync_workstream_selection();
                if let Some(workstream_id) = selected {
                    self.select_workstream_by_id(workstream_id);
                }
                self.set_status(format!(
                    "Workstream view: {}.",
                    self.workstream_view.label()
                ));
                Ok(DashboardAction::None)
            }
            KeyCode::Char('s') if self.focus == FocusPane::Workstreams => {
                let selected = self.selected_workstream_id();
                self.workstream_sort = self.workstream_sort.cycle();
                self.sync_workstream_selection();
                if let Some(workstream_id) = selected {
                    self.select_workstream_by_id(workstream_id);
                }
                self.set_status(format!(
                    "Workstream sort: {}.",
                    self.workstream_sort.label()
                ));
                Ok(DashboardAction::None)
            }
            KeyCode::Char('/') if self.focus == FocusPane::Workstreams => {
                self.modal = Some(Modal::WorkstreamFilter(WorkstreamFilterModal {
                    query: self.workstream_filter.clone(),
                    original_query: self.workstream_filter.clone(),
                }));
                Ok(DashboardAction::None)
            }
            KeyCode::Char('o') => {
                if let Some(id) = self.selected_workstream_id() {
                    return Ok(DashboardAction::Attach(id));
                } else {
                    self.set_status("Select a workstream first.".to_string());
                }
                Ok(DashboardAction::None)
            }
            KeyCode::Char('a') => {
                if let Some(id) = self.selected_workstream_id() {
                    let result = self
                        .runtime
                        .open_workstream(id)
                        .map(|_| "Opened workstream in a new tab.".to_string());
                    self.set_status_from_result(result);
                    self.select_workstream_by_id(id);
                    Ok(DashboardAction::None)
                } else {
                    self.set_status("Select a workstream first.".to_string());
                    Ok(DashboardAction::None)
                }
            }
            KeyCode::Char('v') => {
                let result = match self.focus {
                    FocusPane::Repos => self
                        .selected_repo_id()
                        .ok_or_else(|| anyhow::anyhow!("Select a repo first."))
                        .and_then(|repo_id| self.runtime.open_repo_in_vscode(repo_id))
                        .map(|_| "Opened repo in VS Code.".to_string()),
                    FocusPane::Workstreams => self
                        .selected_workstream_id()
                        .ok_or_else(|| anyhow::anyhow!("Select a workstream first."))
                        .and_then(|workstream_id| {
                            self.runtime.open_workstream_in_vscode(workstream_id)
                        })
                        .map(|_| "Opened workstream in VS Code.".to_string()),
                };
                self.set_status_from_result(result);
                Ok(DashboardAction::None)
            }
            KeyCode::Char('x') => {
                if let Some(id) = self.selected_workstream_id() {
                    let result = self
                        .runtime
                        .stop_workstream(id)
                        .map(|branch| format!("Stopped `{branch}`."));
                    self.set_status_from_result(result);
                    self.select_workstream_by_id(id);
                } else {
                    self.set_status("Select a workstream first.".to_string());
                }
                Ok(DashboardAction::None)
            }
            KeyCode::Char('t') if self.focus == FocusPane::Workstreams => {
                self.open_selected_workstream_triage()
            }
            _ => Ok(DashboardAction::None),
        }
    }

    fn handle_palette_key(&mut self, key: KeyEvent) -> anyhow::Result<DashboardAction> {
        let mut palette = self.palette.clone().unwrap_or_default();
        let filtered_len = self.filtered_palette_entries(&palette).len();

        match key.code {
            KeyCode::Esc => {
                self.palette = None;
            }
            KeyCode::Backspace => {
                palette.query.pop();
                palette.selected = 0;
                self.palette = Some(palette);
            }
            KeyCode::Down => {
                if filtered_len > 0 {
                    palette.selected = (palette.selected + 1).min(filtered_len - 1);
                }
                self.palette = Some(palette);
            }
            KeyCode::Up => {
                if filtered_len > 0 {
                    palette.selected = palette.selected.saturating_sub(1);
                }
                self.palette = Some(palette);
            }
            KeyCode::Enter => {
                if let Some(entry) = self
                    .filtered_palette_entries(&palette)
                    .get(palette.selected)
                    .copied()
                {
                    self.palette = None;
                    return self.activate_palette_command(entry.command);
                }
            }
            KeyCode::Char(ch) => {
                palette.query.push(ch);
                palette.selected = 0;
                self.palette = Some(palette);
            }
            _ => {
                self.palette = Some(palette);
            }
        }

        Ok(DashboardAction::None)
    }

    fn handle_modal_key(&mut self, key: KeyEvent) -> anyhow::Result<DashboardAction> {
        let Some(modal) = self.modal.clone() else {
            return Ok(DashboardAction::None);
        };

        match modal {
            Modal::AddRepo(mut model) => {
                match key.code {
                    KeyCode::Esc => self.modal = None,
                    KeyCode::Backspace => {
                        model.input.pop();
                        self.modal = Some(Modal::AddRepo(model));
                    }
                    KeyCode::Enter => {
                        let path = model.input.trim();
                        let result = self.runtime.add_repo(
                            if path.is_empty() {
                                None
                            } else {
                                Some(PathBuf::from(path))
                            },
                            None,
                        );
                        match result {
                            Ok(repo) => {
                                self.modal = None;
                                self.select_repo_by_id(repo.id);
                                self.set_status(format!("Added repo `{}`.", repo.display_name));
                            }
                            Err(error) => {
                                self.set_status(error.to_string());
                                self.modal = Some(Modal::AddRepo(model));
                            }
                        }
                    }
                    KeyCode::Char(ch) => {
                        model.input.push(ch);
                        self.modal = Some(Modal::AddRepo(model));
                    }
                    _ => self.modal = Some(Modal::AddRepo(model)),
                }
                Ok(DashboardAction::None)
            }
            Modal::Confirm(model) => {
                match key.code {
                    KeyCode::Esc => self.modal = None,
                    KeyCode::Enter => {
                        self.modal = None;
                        match model.action {
                            ConfirmAction::RemoveRepo(repo_id) => {
                                let result = self
                                    .runtime
                                    .remove_repo(repo_id)
                                    .map(|name| format!("Removed repo `{name}`."));
                                self.set_status_from_result(result);
                                self.sync_repo_selection();
                            }
                            ConfirmAction::RemoveWorkstream(workstream_id) => {
                                let result = self
                                    .runtime
                                    .remove_workstream(workstream_id)
                                    .map(|branch| format!("Removed workstream `{branch}`."));
                                self.set_status_from_result(result);
                                self.sync_workstream_selection();
                            }
                        }
                    }
                    _ => self.modal = Some(Modal::Confirm(model)),
                }
                Ok(DashboardAction::None)
            }
            Modal::CreateWorkstream(mut flow) => match key.code {
                KeyCode::Esc => {
                    self.modal = None;
                    Ok(DashboardAction::None)
                }
                KeyCode::Left => {
                    self.step_back(&mut flow);
                    self.modal = Some(Modal::CreateWorkstream(flow));
                    Ok(DashboardAction::None)
                }
                _ => {
                    let keep_open = self.handle_create_workstream_key(&mut flow, key)?;
                    if keep_open {
                        self.modal = Some(Modal::CreateWorkstream(flow));
                    }
                    Ok(DashboardAction::None)
                }
            },
            Modal::WorkstreamFilter(mut model) => {
                match key.code {
                    KeyCode::Esc => {
                        self.workstream_filter = model.original_query;
                        self.modal = None;
                        self.sync_workstream_selection();
                    }
                    KeyCode::Enter => {
                        self.modal = None;
                        self.sync_workstream_selection();
                    }
                    KeyCode::Backspace => {
                        model.query.pop();
                        self.workstream_filter = model.query.clone();
                        self.workstream_state.select(normalize_selection(
                            Some(0),
                            self.visible_workstream_ids().len(),
                        ));
                        self.modal = Some(Modal::WorkstreamFilter(model));
                    }
                    KeyCode::Char(ch) => {
                        model.query.push(ch);
                        self.workstream_filter = model.query.clone();
                        self.workstream_state.select(normalize_selection(
                            Some(0),
                            self.visible_workstream_ids().len(),
                        ));
                        self.modal = Some(Modal::WorkstreamFilter(model));
                    }
                    _ => {
                        self.modal = Some(Modal::WorkstreamFilter(model));
                    }
                }
                Ok(DashboardAction::None)
            }
            Modal::Triage(mut model) => {
                let actions = triage_actions(&model);
                match key.code {
                    KeyCode::Esc => self.modal = None,
                    KeyCode::Char('r') => {
                        self.modal = None;
                        return self
                            .apply_triage_action(model.workstream_id, TriageAction::ReviewNext);
                    }
                    KeyCode::Char('b') => {
                        self.modal = None;
                        return self
                            .apply_triage_action(model.workstream_id, TriageAction::Blocked);
                    }
                    KeyCode::Char('p') => {
                        self.modal = None;
                        return self
                            .apply_triage_action(model.workstream_id, TriageAction::TogglePin);
                    }
                    KeyCode::Char('c') => {
                        self.modal = None;
                        return self.apply_triage_action(model.workstream_id, TriageAction::Clear);
                    }
                    KeyCode::Down => {
                        if !actions.is_empty() {
                            model.selected = (model.selected + 1).min(actions.len() - 1);
                        }
                        self.modal = Some(Modal::Triage(model));
                    }
                    KeyCode::Up => {
                        model.selected = model.selected.saturating_sub(1);
                        self.modal = Some(Modal::Triage(model));
                    }
                    KeyCode::Enter => {
                        self.modal = None;
                        if let Some(action) = actions.get(model.selected).copied() {
                            return self.apply_triage_action(model.workstream_id, action);
                        }
                    }
                    _ => self.modal = Some(Modal::Triage(model)),
                }
                Ok(DashboardAction::None)
            }
        }
    }

    fn handle_create_workstream_key(
        &mut self,
        flow: &mut CreateWorkstreamFlow,
        key: KeyEvent,
    ) -> anyhow::Result<bool> {
        match flow.step {
            CreateStep::BranchMode => match key.code {
                KeyCode::Down => {
                    flow.branch_mode_selected = (flow.branch_mode_selected + 1).min(1);
                }
                KeyCode::Up => {
                    flow.branch_mode_selected = flow.branch_mode_selected.saturating_sub(1);
                }
                KeyCode::Enter => {
                    flow.branch_kind = Some(if flow.branch_mode_selected == 0 {
                        CreateBranchKind::New
                    } else {
                        CreateBranchKind::Existing
                    });
                    flow.step = if flow.branch_mode_selected == 0 {
                        CreateStep::BaseBranch
                    } else {
                        CreateStep::ExistingBranch
                    };
                }
                _ => {}
            },
            CreateStep::BaseBranch => match key.code {
                KeyCode::Backspace => {
                    flow.base_filter.pop();
                    flow.base_selected = 0;
                }
                KeyCode::Down => {
                    let items = filtered_strings(&flow.branches, &flow.base_filter);
                    if !items.is_empty() {
                        flow.base_selected = (flow.base_selected + 1).min(items.len() - 1);
                    }
                }
                KeyCode::Up => {
                    flow.base_selected = flow.base_selected.saturating_sub(1);
                }
                KeyCode::Enter => {
                    let items = filtered_strings(&flow.branches, &flow.base_filter);
                    if let Some(branch) = items.get(flow.base_selected).cloned() {
                        flow.selected_base_branch = Some(branch);
                        flow.step = CreateStep::NewBranch;
                    } else {
                        self.set_status("Choose a base branch first.".to_string());
                    }
                }
                KeyCode::Char(ch) => {
                    flow.base_filter.push(ch);
                    flow.base_selected = 0;
                }
                _ => {}
            },
            CreateStep::ExistingBranch => match key.code {
                KeyCode::Backspace => {
                    flow.existing_filter.pop();
                    flow.existing_selected = 0;
                }
                KeyCode::Down => {
                    let items = filtered_strings(&flow.branches, &flow.existing_filter);
                    if !items.is_empty() {
                        flow.existing_selected =
                            (flow.existing_selected + 1).min(items.len().saturating_sub(1));
                    }
                }
                KeyCode::Up => {
                    flow.existing_selected = flow.existing_selected.saturating_sub(1);
                }
                KeyCode::Enter => {
                    let items = filtered_strings(&flow.branches, &flow.existing_filter);
                    if let Some(branch) = items.get(flow.existing_selected).cloned() {
                        flow.selected_existing_branch = Some(branch);
                        flow.step = CreateStep::Agent;
                    } else {
                        self.set_status("Choose an existing branch first.".to_string());
                    }
                }
                KeyCode::Char(ch) => {
                    flow.existing_filter.push(ch);
                    flow.existing_selected = 0;
                }
                _ => {}
            },
            CreateStep::NewBranch => match key.code {
                KeyCode::Backspace => {
                    flow.new_branch_input.pop();
                }
                KeyCode::Enter => {
                    if flow.new_branch_input.trim().is_empty() {
                        self.set_status("Type a new branch name first.".to_string());
                    } else {
                        flow.step = CreateStep::Agent;
                    }
                }
                KeyCode::Char(ch) => {
                    flow.new_branch_input.push(ch);
                }
                _ => {}
            },
            CreateStep::Agent => match key.code {
                KeyCode::Down => {
                    let len = self.runtime.config.agent_presets.len();
                    if len > 0 {
                        flow.agent_selected = (flow.agent_selected + 1).min(len - 1);
                    }
                }
                KeyCode::Up => {
                    flow.agent_selected = flow.agent_selected.saturating_sub(1);
                }
                KeyCode::Enter => {
                    let Some(agent) = self
                        .runtime
                        .config
                        .agent_presets
                        .get(flow.agent_selected)
                        .map(|preset| preset.name.clone())
                    else {
                        self.set_status("No agent presets configured.".to_string());
                        return Ok(true);
                    };

                    let result = match flow.branch_kind {
                        Some(CreateBranchKind::New) => self.runtime.create_workstream(
                            flow.repo_id,
                            WorkstreamRequest::New {
                                branch: flow.new_branch_input.trim().to_string(),
                                base_branch: flow
                                    .selected_base_branch
                                    .clone()
                                    .unwrap_or_else(|| "main".to_string()),
                            },
                            &agent,
                            LaunchMode::Stay,
                        ),
                        Some(CreateBranchKind::Existing) => self.runtime.create_workstream(
                            flow.repo_id,
                            WorkstreamRequest::Existing {
                                branch: flow.selected_existing_branch.clone().unwrap_or_default(),
                            },
                            &agent,
                            LaunchMode::Stay,
                        ),
                        None => {
                            self.set_status("Choose a branch strategy first.".to_string());
                            return Ok(true);
                        }
                    };

                    match result {
                        Ok(workstream) => {
                            self.select_workstream_by_id(workstream.id);
                            self.set_status(format!(
                                "Created workstream `{}` for `{}`.",
                                workstream.branch, flow.repo_name
                            ));
                            return Ok(false);
                        }
                        Err(error) => {
                            self.set_status(error.to_string());
                        }
                    }
                }
                _ => {}
            },
        }

        Ok(true)
    }

    fn activate_palette_command(
        &mut self,
        command: PaletteCommand,
    ) -> anyhow::Result<DashboardAction> {
        match command {
            PaletteCommand::AddRepo => {
                let default_path = std::env::current_dir()
                    .ok()
                    .map(|path| path.display().to_string())
                    .unwrap_or_default();
                self.modal = Some(Modal::AddRepo(AddRepoModal {
                    input: default_path,
                }));
            }
            PaletteCommand::CreateWorkstream => {
                let Some(repo_id) = self.selected_repo_id() else {
                    self.set_status("Select a repo first.".to_string());
                    return Ok(DashboardAction::None);
                };
                let repo_name = self.runtime.repo_by_id(repo_id)?.display_name.clone();
                match self.runtime.branches_for_repo(repo_id) {
                    Ok(branches) => {
                        self.modal = Some(Modal::CreateWorkstream(CreateWorkstreamFlow::new(
                            repo_id, repo_name, branches,
                        )));
                    }
                    Err(error) => self.set_status(error.to_string()),
                }
            }
            PaletteCommand::OpenInVscode => {
                let result = match self.focus {
                    FocusPane::Repos => self
                        .selected_repo_id()
                        .ok_or_else(|| anyhow::anyhow!("Select a repo first."))
                        .and_then(|repo_id| self.runtime.open_repo_in_vscode(repo_id))
                        .map(|_| "Opened repo in VS Code.".to_string()),
                    FocusPane::Workstreams => self
                        .selected_workstream_id()
                        .ok_or_else(|| anyhow::anyhow!("Select a workstream first."))
                        .and_then(|workstream_id| {
                            self.runtime.open_workstream_in_vscode(workstream_id)
                        })
                        .map(|_| "Opened workstream in VS Code.".to_string()),
                };
                self.set_status_from_result(result);
            }
            PaletteCommand::RefreshRepo => {
                let Some(repo_id) = self.selected_repo_id() else {
                    self.set_status("Select a repo first.".to_string());
                    return Ok(DashboardAction::None);
                };
                let result = self
                    .runtime
                    .refresh_repo(repo_id)
                    .map(|_| "Fetched latest refs.".to_string());
                self.set_status_from_result(result);
            }
            PaletteCommand::RemoveRepo => {
                let Some(repo_id) = self.selected_repo_id() else {
                    self.set_status("Select a repo first.".to_string());
                    return Ok(DashboardAction::None);
                };
                let repo = self.runtime.repo_by_id(repo_id)?.clone();
                self.modal = Some(Modal::Confirm(ConfirmModal {
                    title: "Remove Repo".to_string(),
                    body: format!(
                        "Untrack `{}`?\nThis only removes it from mico. Workstreams must already be removed.",
                        repo.display_name
                    ),
                    action: ConfirmAction::RemoveRepo(repo_id),
                }));
            }
            PaletteCommand::OpenWorkstream => {
                let Some(workstream_id) = self.selected_workstream_id() else {
                    self.set_status("Select a workstream first.".to_string());
                    return Ok(DashboardAction::None);
                };
                return Ok(DashboardAction::Attach(workstream_id));
            }
            PaletteCommand::AttachWorkstream => {
                let Some(workstream_id) = self.selected_workstream_id() else {
                    self.set_status("Select a workstream first.".to_string());
                    return Ok(DashboardAction::None);
                };
                let result = self
                    .runtime
                    .open_workstream(workstream_id)
                    .map(|_| "Opened workstream in a new tab.".to_string());
                self.set_status_from_result(result);
            }
            PaletteCommand::ResumeWorkstream => {
                let Some(workstream_id) = self.selected_workstream_id() else {
                    self.set_status("Select a workstream first.".to_string());
                    return Ok(DashboardAction::None);
                };
                let result = self
                    .runtime
                    .resume_workstream(workstream_id, LaunchMode::Stay)
                    .map(|workstream| format!("Resumed `{}`.", workstream.branch));
                self.set_status_from_result(result);
            }
            PaletteCommand::StopWorkstream => {
                let Some(workstream_id) = self.selected_workstream_id() else {
                    self.set_status("Select a workstream first.".to_string());
                    return Ok(DashboardAction::None);
                };
                let result = self
                    .runtime
                    .stop_workstream(workstream_id)
                    .map(|branch| format!("Stopped `{branch}`."));
                self.set_status_from_result(result);
            }
            PaletteCommand::RemoveWorkstream => {
                let Some(workstream_id) = self.selected_workstream_id() else {
                    self.set_status("Select a workstream first.".to_string());
                    return Ok(DashboardAction::None);
                };
                let workstream = self.runtime.workstream_by_id(workstream_id)?.clone();
                let body = if matches!(
                    workstream.worktree_ownership,
                    crate::domain::model::WorktreeOwnership::External
                ) {
                    format!(
                        "Remove `{}`?\nThis keeps the existing checkout on disk and only untracks it from mico.",
                        workstream.branch
                    )
                } else {
                    format!(
                        "Remove `{}`?\nThis deletes the managed worktree directory and stops its tmux session.",
                        workstream.branch
                    )
                };
                self.modal = Some(Modal::Confirm(ConfirmModal {
                    title: "Remove Workstream".to_string(),
                    body,
                    action: ConfirmAction::RemoveWorkstream(workstream_id),
                }));
            }
            PaletteCommand::RefreshDoctor => {
                let result = self
                    .runtime
                    .refresh_doctor()
                    .map(|_| "Refreshed dependency checks.".to_string());
                self.set_status_from_result(result);
            }
        }

        self.sync_repo_selection();
        self.sync_workstream_selection();
        Ok(DashboardAction::None)
    }

    fn move_selection(&mut self, delta: isize) {
        match self.focus {
            FocusPane::Repos => {
                let len = self.runtime.state.repos.len();
                let next = next_index(self.repo_state.selected(), len, delta);
                self.repo_state.select(next);
            }
            FocusPane::Workstreams => {
                let len = self.visible_workstream_ids().len();
                let next = next_index(self.workstream_state.selected(), len, delta);
                self.workstream_state.select(next);
            }
        }
    }

    fn selected_repo_id(&self) -> Option<Uuid> {
        self.repo_state
            .selected()
            .and_then(|index| self.runtime.state.repos.get(index))
            .map(|repo| repo.id)
    }

    fn selected_workstream_id(&self) -> Option<Uuid> {
        let visible_ids = self.visible_workstream_ids();
        self.workstream_state
            .selected()
            .and_then(|index| visible_ids.get(index))
            .copied()
    }

    fn selected_repo(&self) -> Option<&crate::domain::model::RepoTarget> {
        self.repo_state
            .selected()
            .and_then(|index| self.runtime.state.repos.get(index))
    }

    fn selected_workstream(&self) -> Option<&crate::domain::model::Workstream> {
        self.selected_workstream_id()
            .and_then(|workstream_id| self.runtime.workstream_by_id(workstream_id).ok())
    }

    fn select_repo_by_id(&mut self, repo_id: Uuid) {
        if let Some(index) = self
            .runtime
            .state
            .repos
            .iter()
            .position(|repo| repo.id == repo_id)
        {
            self.repo_state.select(Some(index));
            self.focus = FocusPane::Repos;
        }
    }

    fn select_workstream_by_id(&mut self, workstream_id: Uuid) {
        if let Some(index) = self
            .visible_workstream_ids()
            .iter()
            .position(|candidate| *candidate == workstream_id)
        {
            self.workstream_state.select(Some(index));
            self.focus = FocusPane::Workstreams;
        }
    }

    fn sync_repo_selection(&mut self) {
        let len = self.runtime.state.repos.len();
        self.repo_state
            .select(normalize_selection(self.repo_state.selected(), len));
    }

    fn sync_workstream_selection(&mut self) {
        let len = self.visible_workstream_ids().len();
        self.workstream_state
            .select(normalize_selection(self.workstream_state.selected(), len));
    }

    fn visible_workstream_ids(&self) -> Vec<Uuid> {
        let query = self.workstream_filter.trim().to_lowercase();
        let mut workstreams = self
            .runtime
            .state
            .workstreams
            .iter()
            .filter(|workstream| match self.workstream_view {
                WorkstreamView::All => true,
                WorkstreamView::NeedsAttention => {
                    workstream.pinned
                        || !matches!(workstream.attention, WorkstreamAttention::None)
                        || matches!(workstream.status, WorkstreamStatus::Stopped)
                }
                WorkstreamView::Running => matches!(workstream.status, WorkstreamStatus::Running),
                WorkstreamView::Stopped => matches!(workstream.status, WorkstreamStatus::Stopped),
                WorkstreamView::Pinned => workstream.pinned,
            })
            .filter(|workstream| {
                if query.is_empty() {
                    return true;
                }

                let repo_name = self
                    .runtime
                    .state
                    .repos
                    .iter()
                    .find(|repo| repo.id == workstream.repo_id)
                    .map(|repo| repo.display_name.as_str())
                    .unwrap_or_default();

                workstream.branch.to_lowercase().contains(&query)
                    || workstream.agent_preset.to_lowercase().contains(&query)
                    || workstream.session_name.to_lowercase().contains(&query)
                    || repo_name.to_lowercase().contains(&query)
                    || workstream
                        .worktree_path
                        .display()
                        .to_string()
                        .to_lowercase()
                        .contains(&query)
            })
            .collect::<Vec<_>>();

        workstreams.sort_by(|left, right| compare_workstreams(left, right, self.workstream_sort));
        workstreams
            .into_iter()
            .map(|workstream| workstream.id)
            .collect()
    }

    fn open_selected_workstream_triage(&mut self) -> anyhow::Result<DashboardAction> {
        let Some(workstream_id) = self.selected_workstream_id() else {
            self.set_status("Select a workstream first.".to_string());
            return Ok(DashboardAction::None);
        };

        let workstream = self.runtime.workstream_by_id(workstream_id)?.clone();
        self.modal = Some(Modal::Triage(TriageModal {
            workstream_id,
            branch: workstream.branch,
            pinned: workstream.pinned,
            selected: 0,
        }));
        Ok(DashboardAction::None)
    }

    fn apply_triage_action(
        &mut self,
        workstream_id: Uuid,
        action: TriageAction,
    ) -> anyhow::Result<DashboardAction> {
        match action {
            TriageAction::ReviewNext => {
                self.toggle_selected_workstream_attention(WorkstreamAttention::ReviewNext)
            }
            TriageAction::Blocked => {
                self.toggle_selected_workstream_attention(WorkstreamAttention::Blocked)
            }
            TriageAction::TogglePin => self.toggle_selected_workstream_pin(),
            TriageAction::Clear => self.clear_selected_workstream_triage(),
        }?;
        self.select_workstream_by_id(workstream_id);
        Ok(DashboardAction::None)
    }

    fn toggle_selected_workstream_attention(
        &mut self,
        attention: WorkstreamAttention,
    ) -> anyhow::Result<DashboardAction> {
        let Some(workstream_id) = self.selected_workstream_id() else {
            self.set_status("Select a workstream first.".to_string());
            return Ok(DashboardAction::None);
        };

        let current = self
            .runtime
            .workstream_by_id(workstream_id)?
            .attention
            .clone();
        let result = if current == attention {
            self.runtime
                .clear_workstream_attention(workstream_id)
                .map(|branch| format!("Cleared attention for `{branch}`."))
        } else {
            let label = attention_label(&attention).to_lowercase();
            self.runtime
                .set_workstream_attention(workstream_id, attention)
                .map(|branch| format!("Marked `{branch}` as {label}."))
        };

        self.set_status_from_result(result);
        self.select_workstream_by_id(workstream_id);
        Ok(DashboardAction::None)
    }

    fn toggle_selected_workstream_pin(&mut self) -> anyhow::Result<DashboardAction> {
        let Some(workstream_id) = self.selected_workstream_id() else {
            self.set_status("Select a workstream first.".to_string());
            return Ok(DashboardAction::None);
        };

        let result =
            self.runtime
                .toggle_workstream_pinned(workstream_id)
                .map(|(branch, pinned)| {
                    if pinned {
                        format!("Pinned `{branch}`.")
                    } else {
                        format!("Unpinned `{branch}`.")
                    }
                });
        self.set_status_from_result(result);
        self.select_workstream_by_id(workstream_id);
        Ok(DashboardAction::None)
    }

    fn clear_selected_workstream_triage(&mut self) -> anyhow::Result<DashboardAction> {
        let Some(workstream_id) = self.selected_workstream_id() else {
            self.set_status("Select a workstream first.".to_string());
            return Ok(DashboardAction::None);
        };

        let branch = self.runtime.workstream_by_id(workstream_id)?.branch.clone();
        self.runtime.clear_workstream_attention(workstream_id)?;
        self.runtime.set_workstream_pinned(workstream_id, false)?;
        self.set_status_from_result(Ok(format!("Cleared triage for `{branch}`.")));
        self.select_workstream_by_id(workstream_id);
        Ok(DashboardAction::None)
    }

    fn filtered_palette_entries(&self, palette: &PaletteStateModel) -> Vec<PaletteEntry> {
        let query = palette.query.trim().to_lowercase();
        PALETTE_ENTRIES
            .iter()
            .copied()
            .filter(|entry| {
                query.is_empty()
                    || entry.title.to_lowercase().contains(&query)
                    || entry.detail.to_lowercase().contains(&query)
            })
            .collect()
    }

    fn step_back(&mut self, flow: &mut CreateWorkstreamFlow) {
        flow.step = match flow.step {
            CreateStep::BranchMode => CreateStep::BranchMode,
            CreateStep::BaseBranch => CreateStep::BranchMode,
            CreateStep::ExistingBranch => CreateStep::BranchMode,
            CreateStep::NewBranch => CreateStep::BaseBranch,
            CreateStep::Agent => match flow.branch_kind {
                Some(CreateBranchKind::New) => CreateStep::NewBranch,
                Some(CreateBranchKind::Existing) => CreateStep::ExistingBranch,
                None => CreateStep::BranchMode,
            },
        };
    }

    fn set_status(&mut self, message: String) {
        self.status = Some(StatusMessage {
            text: message,
            tone: StatusTone::Neutral,
        });
    }

    fn set_status_from_result(&mut self, result: anyhow::Result<String>) {
        match result {
            Ok(message) => {
                self.status = Some(StatusMessage {
                    text: message,
                    tone: StatusTone::Good,
                });
            }
            Err(error) => {
                self.status = Some(StatusMessage {
                    text: error.to_string(),
                    tone: StatusTone::Error,
                });
            }
        }
        self.sync_repo_selection();
        self.sync_workstream_selection();
    }

    fn refresh_app_vitals(&mut self) {
        let now = Instant::now();
        if let Some(last_refresh) = self.last_vitals_refresh
            && now.duration_since(last_refresh) < Duration::from_secs(1)
        {
            return;
        }

        self.last_vitals_refresh = Some(now);
        self.app_vitals = sample_app_vitals();
    }

    fn refresh_recent_output(&mut self) {
        let selected_workstream_id = self.selected_workstream_id();

        if selected_workstream_id != self.recent_output_workstream_id {
            self.recent_output_workstream_id = selected_workstream_id;
            self.recent_output_lines.clear();
            self.last_output_refresh = None;
        }

        let Some(workstream_id) = selected_workstream_id else {
            return;
        };

        let now = Instant::now();
        if let Some(last_refresh) = self.last_output_refresh
            && now.duration_since(last_refresh) < Duration::from_millis(800)
        {
            return;
        }

        self.last_output_refresh = Some(now);
        self.recent_output_lines = self
            .runtime
            .recent_workstream_output(workstream_id, 5)
            .unwrap_or_else(|error| vec![format!("output unavailable: {error}")]);
    }

    fn confirm_quit(&mut self, key: QuitKey, label: &str) -> anyhow::Result<DashboardAction> {
        let now = Instant::now();

        if let Some((pending_key, pending_at)) = self.pending_quit
            && pending_key == key
            && now.duration_since(pending_at) <= Duration::from_secs(2)
        {
            self.pending_quit = None;
            return Ok(DashboardAction::Quit);
        }

        self.pending_quit = Some((key, now));
        self.set_status(format!("Press {label} again to quit."));
        Ok(DashboardAction::None)
    }
}

fn filtered_strings(items: &[String], filter: &str) -> Vec<String> {
    let query = filter.trim().to_lowercase();
    items
        .iter()
        .filter(|item| query.is_empty() || item.to_lowercase().contains(&query))
        .cloned()
        .collect()
}

fn line_from_pairs(items: &[(&str, &str)]) -> Line<'static> {
    let mut spans = Vec::new();

    for (index, (key, label)) in items.iter().enumerate() {
        if index > 0 {
            spans.push(Span::raw("  "));
        }
        spans.push(key_chip(key));
        spans.push(Span::raw(format!(" {label}")));
    }

    Line::from(spans)
}

fn line_from_stat_pairs(items: &[(String, String)]) -> Line<'static> {
    let mut spans = Vec::new();

    for (index, (key, value)) in items.iter().enumerate() {
        if index > 0 {
            spans.push(Span::raw("   "));
        }
        spans.push(Span::styled(
            format!("{key} "),
            Style::default().fg(INFO_BLUE).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            value.clone(),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ));
    }

    Line::from(spans)
}

fn sample_app_vitals() -> Option<AppVitals> {
    let pid = std::process::id();
    let output = Command::new("ps")
        .args([
            "-p",
            &pid.to_string(),
            "-o",
            "pid=",
            "-o",
            "%cpu=",
            "-o",
            "rss=",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let line = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()?
        .trim()
        .to_string();
    let mut parts = line.split_whitespace();
    let pid = parts.next()?.parse().ok()?;
    let cpu_pct = parts.next()?.parse().ok()?;
    let rss_kb = parts.next()?.parse().ok()?;
    let clock_label = sample_clock_label()?;

    Some(AppVitals {
        pid,
        cpu_pct,
        rss_kb,
        clock_label,
    })
}

fn sample_clock_label() -> Option<String> {
    let output = Command::new("date")
        .args(["+%I:%M:%S %p %Z"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn key_chip(key: &str) -> Span<'static> {
    Span::styled(
        format!("[{key}]"),
        Style::default().fg(PURPLE_B).add_modifier(Modifier::BOLD),
    )
}

fn triage_actions(_modal: &TriageModal) -> Vec<TriageAction> {
    vec![
        TriageAction::ReviewNext,
        TriageAction::Blocked,
        TriageAction::TogglePin,
        TriageAction::Clear,
    ]
}

fn triage_action_label(action: TriageAction, modal: &TriageModal) -> String {
    match action {
        TriageAction::ReviewNext => "Mark review next".to_string(),
        TriageAction::Blocked => "Mark blocked".to_string(),
        TriageAction::TogglePin => {
            if modal.pinned {
                "Unpin workstream".to_string()
            } else {
                "Pin workstream".to_string()
            }
        }
        TriageAction::Clear => "Clear triage".to_string(),
    }
}

fn prioritize_branches(mut branches: Vec<String>) -> Vec<String> {
    branches.sort_by_key(|branch| {
        let rank = match branch.as_str() {
            "main" => 0,
            "master" => 1,
            "trunk" => 2,
            "develop" => 3,
            _ => 10,
        };
        (rank, branch.clone())
    });
    branches
}

fn preferred_branch_index(branches: &[String]) -> usize {
    ["main", "master", "trunk", "develop"]
        .iter()
        .find_map(|preferred| branches.iter().position(|branch| branch == preferred))
        .unwrap_or(0)
}

fn compare_workstreams(left: &Workstream, right: &Workstream, sort: WorkstreamSort) -> Ordering {
    let pinned_cmp = right.pinned.cmp(&left.pinned);
    if pinned_cmp != Ordering::Equal {
        return pinned_cmp;
    }

    match sort {
        WorkstreamSort::Attention => {
            let rank_cmp = workstream_attention_rank(left).cmp(&workstream_attention_rank(right));
            if rank_cmp != Ordering::Equal {
                return rank_cmp;
            }

            let age_cmp = left
                .status_changed_at_epoch_secs
                .cmp(&right.status_changed_at_epoch_secs);
            if age_cmp != Ordering::Equal {
                return age_cmp;
            }

            left.branch.cmp(&right.branch)
        }
        WorkstreamSort::Recent => {
            let recent_cmp =
                latest_activity_epoch_secs(right).cmp(&latest_activity_epoch_secs(left));
            if recent_cmp != Ordering::Equal {
                return recent_cmp;
            }

            let rank_cmp = workstream_attention_rank(left).cmp(&workstream_attention_rank(right));
            if rank_cmp != Ordering::Equal {
                return rank_cmp;
            }

            left.branch.cmp(&right.branch)
        }
    }
}

fn workstream_attention_rank(workstream: &Workstream) -> u8 {
    match workstream.attention {
        WorkstreamAttention::Blocked => 0,
        WorkstreamAttention::ReviewNext => 1,
        WorkstreamAttention::None => match workstream.status {
            WorkstreamStatus::Stopped => 2,
            WorkstreamStatus::Running => 3,
        },
    }
}

fn latest_activity_epoch_secs(workstream: &Workstream) -> u64 {
    workstream
        .last_attached_at_epoch_secs
        .into_iter()
        .chain(workstream.last_opened_at_epoch_secs)
        .max()
        .unwrap_or(workstream.created_at_epoch_secs)
}

fn workstream_state_label(workstream: &Workstream) -> &'static str {
    match workstream.attention {
        WorkstreamAttention::Blocked => "blocked",
        WorkstreamAttention::ReviewNext => "review next",
        WorkstreamAttention::None => match workstream.status {
            WorkstreamStatus::Running => "running",
            WorkstreamStatus::Stopped => "stopped",
        },
    }
}

fn workstream_chips(workstream: &Workstream) -> Vec<Span<'static>> {
    let mut chips = Vec::new();

    if matches!(
        workstream.worktree_ownership,
        crate::domain::model::WorktreeOwnership::External
    ) {
        chips.push(label_chip("LINKED", INFO_BLUE));
    }

    if workstream.pinned {
        chips.push(label_chip("PIN", INFO_BLUE));
    }

    match workstream.attention {
        WorkstreamAttention::None => {}
        WorkstreamAttention::ReviewNext => chips.push(label_chip("REVIEW", WARNING_AMBER)),
        WorkstreamAttention::Blocked => chips.push(label_chip("BLOCKED", ERROR_RED)),
    }

    match workstream.status {
        WorkstreamStatus::Running => chips.push(label_chip("RUNNING", SUCCESS_GREEN)),
        WorkstreamStatus::Stopped => chips.push(label_chip("STOPPED", WARNING_AMBER)),
    }

    chips
}

fn label_chip(label: &str, color: Color) -> Span<'static> {
    Span::styled(
        format!("[{label}]"),
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    )
}

fn attention_label(attention: &WorkstreamAttention) -> &'static str {
    match attention {
        WorkstreamAttention::None => "none",
        WorkstreamAttention::ReviewNext => "review next",
        WorkstreamAttention::Blocked => "blocked",
    }
}

fn status_style(tone: StatusTone) -> Style {
    match tone {
        StatusTone::Neutral => Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
        StatusTone::Good => Style::default()
            .fg(SUCCESS_GREEN)
            .add_modifier(Modifier::BOLD),
        StatusTone::Error => Style::default().fg(ERROR_RED).add_modifier(Modifier::BOLD),
    }
}

fn elapsed_label(epoch_secs: u64) -> String {
    if epoch_secs == 0 {
        return "?".to_string();
    }

    let now = now_epoch_secs();
    if now <= epoch_secs {
        return "now".to_string();
    }

    format_duration(now - epoch_secs)
}

fn option_elapsed_label(epoch_secs: Option<u64>) -> String {
    epoch_secs
        .map(elapsed_label)
        .unwrap_or_else(|| "never".to_string())
}

fn format_kb(kb: u64) -> String {
    if kb < 1024 {
        format!("{kb} KB")
    } else {
        let whole_mb = kb / 1024;
        let tenth_mb = (kb % 1024) * 10 / 1024;
        format!("{whole_mb}.{tenth_mb} MB")
    }
}

fn format_duration(duration_secs: u64) -> String {
    if duration_secs < 60 {
        format!("{duration_secs}s")
    } else if duration_secs < 60 * 60 {
        format!("{}m", duration_secs / 60)
    } else if duration_secs < 60 * 60 * 24 {
        format!("{}h", duration_secs / (60 * 60))
    } else {
        format!("{}d", duration_secs / (60 * 60 * 24))
    }
}

fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn next_index(current: Option<usize>, len: usize, delta: isize) -> Option<usize> {
    if len == 0 {
        return None;
    }

    let current = current.unwrap_or(0).min(len - 1);
    let movement = delta.unsigned_abs();
    let next = if delta.is_negative() {
        current.saturating_sub(movement)
    } else {
        current.saturating_add(movement).min(len - 1)
    };
    Some(next)
}

fn normalize_selection(current: Option<usize>, len: usize) -> Option<usize> {
    if len == 0 {
        None
    } else {
        Some(current.unwrap_or(0).min(len - 1))
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}
