use std::{
    cmp::Ordering,
    collections::VecDeque,
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
    app::{
        background::{
            BackgroundTaskManager, SessionLaunchTarget, TaskLock, TaskRequest, TaskSuccess,
            TrackedWorkstreamPath,
        },
        runtime::{LaunchMode, MicoRuntime},
    },
    domain::model::{
        AttentionReason, Workstream, WorkstreamRequest, WorkstreamSession, WorkstreamStatus,
    },
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
const LIVE_OUTPUT: Color = Color::Rgb(255, 244, 161);

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
        app.poll_background_tasks();
        app.background_tasks
            .refresh_config(app.runtime.config_snapshot());
        app.process_deferred_actions(terminal)?;
        terminal.draw(|frame| app.render(frame))?;

        if event::poll(Duration::from_millis(250))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            match app.handle_key(key)? {
                DashboardAction::None => {}
                DashboardAction::Quit => return Ok(()),
                DashboardAction::Attach(workstream_id, session_id) => {
                    suspend_terminal(terminal)?;
                    let result = app
                        .runtime
                        .attach_workstream_session(workstream_id, session_id);
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
            Self::Repos => "missions",
            Self::Workstreams => "launches",
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum DashboardAction {
    None,
    Quit,
    Attach(Uuid, Uuid),
}

#[derive(Debug, Clone, Copy)]
enum DeferredAction {
    Open(Uuid, Uuid),
    Attach(Uuid, Uuid),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkstreamView {
    All,
    NeedsYou,
    Running,
    Stopped,
}

impl WorkstreamView {
    fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::NeedsYou => "needs-you",
            Self::Running => "running",
            Self::Stopped => "stopped",
        }
    }

    fn cycle(self, delta: isize) -> Self {
        let options = [Self::All, Self::NeedsYou, Self::Running, Self::Stopped];
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
    LaunchWorkstreamSession,
    RunWorkstreamOneOff,
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

const PALETTE_ENTRIES: [PaletteEntry; 13] = [
    PaletteEntry {
        command: PaletteCommand::AddRepo,
        title: "Add mission",
        detail: "Track another repository from a filesystem path.",
    },
    PaletteEntry {
        command: PaletteCommand::CreateWorkstream,
        title: "Create launch",
        detail: "Use the selected mission to create a new or existing branch worktree.",
    },
    PaletteEntry {
        command: PaletteCommand::LaunchWorkstreamSession,
        title: "Launch another session",
        detail: "Start an additional claude, codex, opencode, or terminal session in the selected launch.",
    },
    PaletteEntry {
        command: PaletteCommand::RunWorkstreamOneOff,
        title: "Run one-off agent command",
        detail: "Open a drawer for a non-interactive agent run in the selected launch.",
    },
    PaletteEntry {
        command: PaletteCommand::OpenInVscode,
        title: "Open selection in VS Code",
        detail: "Open the selected mission or launch directory with `code`.",
    },
    PaletteEntry {
        command: PaletteCommand::RefreshRepo,
        title: "Refresh selected mission",
        detail: "Fetch the latest refs for the selected repository.",
    },
    PaletteEntry {
        command: PaletteCommand::RemoveRepo,
        title: "Remove selected mission",
        detail: "Untrack the selected mission after its launches are gone.",
    },
    PaletteEntry {
        command: PaletteCommand::OpenWorkstream,
        title: "Open selected launch",
        detail: "Open it in this terminal. Detach with Ctrl-b d to return to mico.",
    },
    PaletteEntry {
        command: PaletteCommand::AttachWorkstream,
        title: "Open selected launch in new tab",
        detail: "Open the selected launch in a new iTerm tab.",
    },
    PaletteEntry {
        command: PaletteCommand::ResumeWorkstream,
        title: "Resume selected launch",
        detail: "Recreate a tmux session in the saved worktree and mark it running again.",
    },
    PaletteEntry {
        command: PaletteCommand::StopWorkstream,
        title: "Stop selected launch",
        detail: "Kill the tmux session but keep the worktree and local record.",
    },
    PaletteEntry {
        command: PaletteCommand::RemoveWorkstream,
        title: "Remove selected launch",
        detail: "Remove the selected launch. Managed worktrees are deleted; linked checkouts are untracked.",
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
    LaunchSession(LaunchSessionModal),
    OneOff(OneOffModal),
    SessionPicker(SessionPickerModal),
    WorkstreamFilter(WorkstreamFilterModal),
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
struct LaunchSessionModal {
    workstream_id: Uuid,
    branch: String,
    launch_mode: LaunchMode,
    selected: usize,
}

#[derive(Debug, Clone)]
struct OneOffModal {
    workstream_id: Uuid,
    branch: String,
    prompt: String,
    output: Option<String>,
    selected: usize,
}

#[derive(Debug, Clone)]
struct SessionPickerModal {
    workstream_id: Uuid,
    branch: String,
    launch_mode: LaunchMode,
    selected: usize,
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
    last_attention_refresh: Option<Instant>,
    background_tasks: BackgroundTaskManager,
    deferred_actions: VecDeque<DeferredAction>,
}

impl DashboardApp {
    fn new(mut runtime: MicoRuntime) -> Self {
        let _ = runtime.refresh_doctor();
        let background_tasks = BackgroundTaskManager::new(
            runtime.paths().clone(),
            runtime.config_snapshot(),
            runtime.completion_store(),
        );
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
            last_attention_refresh: None,
            background_tasks,
            deferred_actions: VecDeque::new(),
        }
    }

    fn render(&mut self, frame: &mut Frame<'_>) {
        self.refresh_app_vitals();
        self.refresh_attention_signals();
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
        let needs_you_count = self
            .runtime
            .state
            .workstreams
            .iter()
            .filter(|workstream| workstream.has_unread_attention())
            .count();
        let running_count = self
            .runtime
            .state
            .workstreams
            .iter()
            .filter(|workstream| matches!(workstream.status, WorkstreamStatus::Running))
            .count();
        let footer = Paragraph::new(self.flight_deck_line(needs_you_count, running_count))
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
            Line::from("Fast path: add a mission, hit Enter, pick a branch, launch an agent."),
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
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(area);

        let repo_items: Vec<ListItem<'_>> = if self.runtime.state.repos.is_empty() {
            vec![ListItem::new(
                "No missions tracked yet. Press : and choose Add mission.",
            )]
        } else {
            self.runtime
                .state
                .repos
                .iter()
                .map(|repo| {
                    let mut header = vec![Span::styled(
                        repo.display_name.clone(),
                        Style::default().add_modifier(Modifier::BOLD),
                    )];
                    if self.background_tasks.is_repo_mutating(repo.id) {
                        header.push(Span::raw(" "));
                        header.push(label_chip("BUSY", WARNING_AMBER));
                    }
                    ListItem::new(vec![
                        Line::from(header),
                        Line::from(repo.path.display().to_string()),
                    ])
                })
                .collect()
        };

        let repo_block = Block::default()
            .borders(Borders::ALL)
            .title("Missions")
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
                    "No launches yet. Select a mission, open the palette, and create one."
                }
                _ => "No launches matched the current view.",
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
                    if self.background_tasks.is_workstream_locked(workstream.id) {
                        header.push(Span::raw(" "));
                        header.push(label_chip("BUSY", WARNING_AMBER));
                    }
                    for chip in workstream_chips(workstream) {
                        header.push(Span::raw(" "));
                        header.push(chip);
                    }

                    let mut lines = vec![
                        Line::from(header),
                        line_from_stat_pairs(&[
                            ("repo".to_string(), repo_name.to_string()),
                            (
                                "agent".to_string(),
                                workstream
                                    .preferred_session()
                                    .map(|session| session.agent_preset.clone())
                                    .unwrap_or_else(|| workstream.agent_preset.clone()),
                            ),
                            (
                                "sessions".to_string(),
                                format!(
                                    "{}/{} live",
                                    workstream.running_session_count(),
                                    workstream.session_count()
                                ),
                            ),
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
                        let hint = match (workstream.status.clone(), workstream.session_count()) {
                            (WorkstreamStatus::Running, 0 | 1) => line_from_pairs(&[
                                ("Enter/o", "open here"),
                                ("a", "new tab"),
                                ("n", "new session"),
                                ("x", "stop"),
                            ]),
                            (WorkstreamStatus::Running, _) => line_from_pairs(&[
                                ("Enter/o", "pick session"),
                                ("a", "pick new tab"),
                                ("n", "new session"),
                                ("x", "stop"),
                            ]),
                            (WorkstreamStatus::Stopped, 0 | 1) => line_from_pairs(&[
                                ("Enter/o", "resume here"),
                                ("a", "resume in new tab"),
                                ("n", "new session"),
                            ]),
                            (WorkstreamStatus::Stopped, _) => line_from_pairs(&[
                                ("Enter/o", "pick session"),
                                ("a", "pick new tab"),
                                ("n", "new session"),
                            ]),
                        };
                        lines.push(hint);
                    }

                    lines.push(Line::from(""));

                    ListItem::new(lines)
                })
                .collect()
        };

        let workstream_title = format!(
            "Launches  view:{}  sort:{}{}",
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

        let needs_you = workstreams
            .iter()
            .filter(|workstream| workstream.has_unread_attention())
            .count();
        let failed = workstreams
            .iter()
            .filter(|workstream| has_unread_reason(workstream, AttentionReason::TaskFailed))
            .count();
        let done = workstreams
            .iter()
            .filter(|workstream| has_unread_reason(workstream, AttentionReason::OneOffCompleted))
            .count();
        let idle = workstreams
            .iter()
            .filter(|workstream| has_unread_reason(workstream, AttentionReason::IdleOutput))
            .count();
        let drift = workstreams
            .iter()
            .filter(|workstream| has_unread_reason(workstream, AttentionReason::BranchChanged))
            .count();
        let stopped = workstreams
            .iter()
            .filter(|workstream| matches!(workstream.status, WorkstreamStatus::Stopped))
            .count();
        let running = workstreams.len().saturating_sub(stopped);
        let selected_line = self
            .selected_workstream()
            .map(|workstream| {
                let session_label = workstream
                    .preferred_session()
                    .map(|session| session.agent_preset.clone())
                    .unwrap_or_else(|| workstream.agent_preset.clone());
                format!(
                    "selected {}   session {}   live {}/{}   last touch {}",
                    workstream.branch,
                    session_label,
                    workstream.running_session_count(),
                    workstream.session_count(),
                    option_elapsed_label(
                        workstream
                            .last_attached_at_epoch_secs
                            .or(workstream.last_opened_at_epoch_secs)
                    ),
                )
            })
            .unwrap_or_else(|| "select a workstream to inspect its current signal.".to_string());
        let selected_line = self
            .selected_workstream()
            .and_then(|workstream| {
                workstream
                    .latest_unread_attention_event()
                    .or_else(|| workstream.latest_attention_event())
                    .map(|event| format!("{selected_line}   signal {}", event.summary))
            })
            .unwrap_or_else(|| selected_line.replace("workstream", "launch"));
        let block = Block::default()
            .borders(Borders::ALL)
            .title("Attention Inbox");
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
                    "No launches in this view yet.",
                    Style::default().fg(MUTED_TEXT),
                ),
                Line::from(""),
            ]
        } else {
            vec![
                Line::from(format!(
                    "needs you {}   failed {}   idle {}   done {}   drift {}   running {}   stopped {}",
                    needs_you, failed, idle, done, drift, running, stopped
                )),
                Line::styled(selected_line, Style::default().fg(MUTED_TEXT)),
            ]
        };
        frame.render_widget(
            Paragraph::new(Text::from(summary_lines)).wrap(Wrap { trim: false }),
            chunks[0],
        );

        let mut inbox_entries = workstreams
            .iter()
            .filter_map(|workstream| {
                workstream
                    .latest_unread_attention_event()
                    .map(|event| (workstream, event))
            })
            .collect::<Vec<_>>();
        inbox_entries.sort_by(|(_, left), (_, right)| {
            right
                .created_at_epoch_secs
                .cmp(&left.created_at_epoch_secs)
                .then_with(|| left.summary.cmp(&right.summary))
        });

        let selected_attention_detail = self
            .selected_workstream_id()
            .and_then(|workstream_id| self.runtime.latest_attention_detail(workstream_id).ok())
            .flatten();

        let output_lines = if let Some(detail) = selected_attention_detail {
            detail
                .lines()
                .map(|line| Line::styled(line.to_string(), Style::default().fg(LIVE_OUTPUT)))
                .collect::<Vec<_>>()
        } else if !inbox_entries.is_empty() {
            inbox_entries
                .into_iter()
                .map(|(workstream, event)| {
                    Line::from(vec![
                        Span::styled(
                            format!("[{}] ", attention_reason_label(&event.reason)),
                            Style::default()
                                .fg(attention_reason_color(&event.reason))
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            format!("{} ", workstream.branch),
                            Style::default()
                                .fg(Color::White)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(event.summary.clone(), Style::default().fg(MUTED_TEXT)),
                    ])
                })
                .collect::<Vec<_>>()
        } else if self.recent_output_lines.is_empty() {
            vec![Line::styled(
                "live waiting for pane output",
                Style::default().fg(LIVE_OUTPUT),
            )]
        } else {
            self.recent_output_lines
                .iter()
                .map(|line| Line::styled(format!("live {line}"), Style::default().fg(LIVE_OUTPUT)))
                .collect::<Vec<_>>()
        };
        frame.render_widget(
            Paragraph::new(Text::from(output_lines)).wrap(Wrap { trim: false }),
            chunks[1],
        );
    }

    fn flight_deck_line(&self, needs_you_count: usize, running_count: usize) -> Line<'static> {
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
                    "missions".to_string(),
                    self.runtime.state.repos.len().to_string(),
                ));
                pairs.push((
                    "launches".to_string(),
                    self.runtime.state.workstreams.len().to_string(),
                ));
                pairs.push(("running".to_string(), running_count.to_string()));
                pairs.push(("needs-you".to_string(), needs_you_count.to_string()));
                pairs.push((
                    "jobs".to_string(),
                    self.background_tasks.active_tasks().len().to_string(),
                ));
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
                "selected mission: {}  ({})",
                repo.display_name,
                repo.path.display()
            )
        });
        let selected_workstream = self.selected_workstream().map(|workstream| {
            let session_summary = workstream
                .preferred_session()
                .map(|session| format!("{} ({})", session.agent_preset, session.session_name))
                .unwrap_or_else(|| "no session".to_string());
            format!(
                "selected launch: {}  -> {}  [{}]",
                workstream.branch,
                workstream.worktree_path.display(),
                session_summary
            )
        });
        let status = self.status.clone().unwrap_or(StatusMessage {
            text: "Ready.".to_string(),
            tone: StatusTone::Neutral,
        });
        let context_line = match self.focus {
            FocusPane::Repos => line_from_pairs(&[
                ("Enter", "create launch"),
                ("v", "open in code"),
                (":", "commands"),
            ]),
            FocusPane::Workstreams => line_from_pairs(&[
                ("Enter", "open here"),
                ("o", "open here"),
                ("a", "open new tab"),
                ("n", "new session"),
                ("!", "one-off"),
                ("v", "open in code"),
                ("/", "filter"),
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
        let jobs_line = if self.background_tasks.active_tasks().is_empty() {
            Line::from("jobs: none")
        } else {
            Line::from(format!(
                "jobs: {}",
                self.background_tasks.active_labels().join("  |  ")
            ))
        };

        let info = Paragraph::new(Text::from(vec![
            Line::styled(
                format!("status: {}", status.text),
                status_style(status.tone),
            ),
            Line::from(selected_repo.unwrap_or_else(|| "selected mission: none".to_string())),
            Line::from(selected_workstream.unwrap_or_else(|| "selected launch: none".to_string())),
            jobs_line,
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
            Modal::LaunchSession(model) => self.render_launch_session_modal(frame, model),
            Modal::OneOff(model) => self.render_one_off_modal(frame, model),
            Modal::SessionPicker(model) => self.render_session_picker_modal(frame, model),
            Modal::WorkstreamFilter(model) => self.render_workstream_filter_modal(frame, model),
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
            Line::from("Add mission"),
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
                .title("Filter Launches"),
        );

        frame.render_widget(body, area);
    }

    fn render_session_picker_modal(&self, frame: &mut Frame<'_>, modal: &SessionPickerModal) {
        let area = centered_rect(66, 42, frame.area());
        frame.render_widget(Clear, area);
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(5), Constraint::Min(8)])
            .split(area);

        let help = Paragraph::new(Text::from(vec![
            Line::from(format!("Choose a session for `{}`", modal.branch)),
            line_from_pairs(&[
                ("↑↓", "move"),
                ("Enter", "open session"),
                ("n", "new session"),
                ("Esc", "cancel"),
            ]),
        ]))
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title("Sessions"));

        let sessions = self
            .runtime
            .workstream_sessions(modal.workstream_id)
            .unwrap_or_default();
        let items = sessions
            .iter()
            .map(session_picker_item)
            .collect::<Vec<ListItem<'_>>>();
        let mut state = ListState::default();
        if !items.is_empty() {
            state.select(Some(modal.selected.min(items.len().saturating_sub(1))));
        }

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Available"))
            .highlight_style(
                Style::default()
                    .bg(PANEL_HIGHLIGHT)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

        frame.render_widget(help, layout[0]);
        frame.render_stateful_widget(list, layout[1], &mut state);
    }

    fn render_launch_session_modal(&self, frame: &mut Frame<'_>, modal: &LaunchSessionModal) {
        let area = centered_rect(58, 30, frame.area());
        frame.render_widget(Clear, area);
        let help = Paragraph::new(Text::from(vec![
            Line::from(format!(
                "Launch another session for launch `{}`",
                modal.branch
            )),
            line_from_pairs(&[("↑↓", "move"), ("Enter", "launch"), ("Esc", "cancel")]),
        ]))
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title("New Session"));

        let presets = self
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

        let mut state = ListState::default();
        if !presets.is_empty() {
            state.select(Some(modal.selected.min(presets.len().saturating_sub(1))));
        }

        let list = List::new(
            presets
                .into_iter()
                .map(ListItem::new)
                .collect::<Vec<ListItem<'_>>>(),
        )
        .block(Block::default().borders(Borders::ALL).title("Agents"))
        .highlight_style(
            Style::default()
                .bg(PANEL_HIGHLIGHT)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(5), Constraint::Min(8)])
            .split(area);
        frame.render_widget(help, layout[0]);
        frame.render_stateful_widget(list, layout[1], &mut state);
    }

    fn render_one_off_modal(&self, frame: &mut Frame<'_>, modal: &OneOffModal) {
        let area = centered_rect(76, 60, frame.area());
        frame.render_widget(Clear, area);
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(5),
                Constraint::Length(3),
                Constraint::Length(4),
                Constraint::Min(10),
            ])
            .split(area);

        let help = Paragraph::new(Text::from(vec![
            Line::from(format!("One-off agent run in `{}`", modal.branch)),
            line_from_pairs(&[
                ("↑↓", "change agent"),
                ("Type", "edit prompt"),
                ("Enter", "run"),
                ("Esc", "close"),
            ]),
        ]))
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title("One-Off"));

        let presets = one_off_agent_names(&self.runtime);
        let selected_agent = presets
            .get(modal.selected)
            .cloned()
            .unwrap_or_else(|| "n/a".to_string());
        let agent = Paragraph::new(selected_agent)
            .block(Block::default().borders(Borders::ALL).title("Agent"));
        let prompt = Paragraph::new(modal.prompt.clone())
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title("Prompt"));
        let output = Paragraph::new(modal.output.clone().unwrap_or_else(|| {
            "Run a one-off command and the response will land here.".to_string()
        }))
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title("Output"));

        frame.render_widget(help, layout[0]);
        frame.render_widget(agent, layout[1]);
        frame.render_widget(prompt, layout[2]);
        frame.render_widget(output, layout[3]);
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
            "Create launch for {}\nChoose whether to create a new branch from a base branch or use an existing branch directly.",
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
                FocusPane::Workstreams => self.open_or_pick_selected_workstream(LaunchMode::Attach),
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
                    return self.open_or_pick_workstream(id, LaunchMode::Attach);
                } else {
                    self.set_status("Select a workstream first.".to_string());
                }
                Ok(DashboardAction::None)
            }
            KeyCode::Char('a') => {
                if let Some(id) = self.selected_workstream_id() {
                    self.open_or_pick_workstream(id, LaunchMode::Open)
                } else {
                    self.set_status("Select a workstream first.".to_string());
                    Ok(DashboardAction::None)
                }
            }
            KeyCode::Char('n') if self.focus == FocusPane::Workstreams => {
                let Some(workstream_id) = self.selected_workstream_id() else {
                    self.set_status("Select a workstream first.".to_string());
                    return Ok(DashboardAction::None);
                };
                self.open_launch_session_modal(workstream_id, LaunchMode::Stay)
            }
            KeyCode::Char('!') if self.focus == FocusPane::Workstreams => {
                let Some(workstream_id) = self.selected_workstream_id() else {
                    self.set_status("Select a workstream first.".to_string());
                    return Ok(DashboardAction::None);
                };
                self.open_one_off_modal(workstream_id)
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
                    self.start_stop_workstream(id)?;
                } else {
                    self.set_status("Select a workstream first.".to_string());
                }
                Ok(DashboardAction::None)
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
                                self.start_remove_workstream(workstream_id)?;
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
                    } else {
                        self.modal = None;
                    }
                    Ok(DashboardAction::None)
                }
            },
            Modal::LaunchSession(mut model) => {
                let len = self.runtime.config.agent_presets.len();
                match key.code {
                    KeyCode::Esc => self.modal = None,
                    KeyCode::Down => {
                        if len > 0 {
                            model.selected = (model.selected + 1).min(len.saturating_sub(1));
                        }
                        self.modal = Some(Modal::LaunchSession(model));
                    }
                    KeyCode::Up => {
                        model.selected = model.selected.saturating_sub(1);
                        self.modal = Some(Modal::LaunchSession(model));
                    }
                    KeyCode::Enter => {
                        let Some(agent) = self
                            .runtime
                            .config
                            .agent_presets
                            .get(model.selected)
                            .map(|preset| preset.name.clone())
                        else {
                            self.set_status("No agent presets configured.".to_string());
                            self.modal = Some(Modal::LaunchSession(model));
                            return Ok(DashboardAction::None);
                        };

                        self.modal = None;
                        let runtime_launch_mode = if matches!(model.launch_mode, LaunchMode::Attach)
                        {
                            LaunchMode::Stay
                        } else {
                            model.launch_mode
                        };
                        let result = self.start_create_workstream_session(
                            model.workstream_id,
                            &agent,
                            runtime_launch_mode,
                            matches!(model.launch_mode, LaunchMode::Attach),
                        );
                        match result {
                            Ok(()) => {
                                if matches!(model.launch_mode, LaunchMode::Attach) {
                                    self.set_status(
                                        "Launching session in the background. Attaching when it is ready."
                                            .to_string(),
                                    );
                                }
                            }
                            Err(error) => self.set_status(error.to_string()),
                        }
                    }
                    _ => {
                        self.modal = Some(Modal::LaunchSession(model));
                    }
                }
                Ok(DashboardAction::None)
            }
            Modal::OneOff(mut model) => {
                let presets = one_off_agent_names(&self.runtime);
                match key.code {
                    KeyCode::Esc => self.modal = None,
                    KeyCode::Backspace => {
                        model.prompt.pop();
                        self.modal = Some(Modal::OneOff(model));
                    }
                    KeyCode::Down => {
                        if !presets.is_empty() {
                            model.selected =
                                (model.selected + 1).min(presets.len().saturating_sub(1));
                        }
                        self.modal = Some(Modal::OneOff(model));
                    }
                    KeyCode::Up => {
                        model.selected = model.selected.saturating_sub(1);
                        self.modal = Some(Modal::OneOff(model));
                    }
                    KeyCode::Enter => {
                        let Some(agent) = presets.get(model.selected) else {
                            self.set_status("No one-off agents are configured.".to_string());
                            self.modal = Some(Modal::OneOff(model));
                            return Ok(DashboardAction::None);
                        };
                        if model.prompt.trim().is_empty() {
                            self.set_status("Type a one-off prompt first.".to_string());
                            self.modal = Some(Modal::OneOff(model));
                            return Ok(DashboardAction::None);
                        }
                        match self.start_one_off_task(
                            model.workstream_id,
                            agent,
                            model.prompt.trim(),
                        ) {
                            Ok(()) => {
                                model.output = Some("Running one-off command...".to_string());
                                self.modal = Some(Modal::OneOff(model));
                            }
                            Err(error) => {
                                model.output = Some(error.to_string());
                                self.modal = Some(Modal::OneOff(model));
                            }
                        }
                    }
                    KeyCode::Char(ch) => {
                        model.prompt.push(ch);
                        self.modal = Some(Modal::OneOff(model));
                    }
                    _ => self.modal = Some(Modal::OneOff(model)),
                }
                Ok(DashboardAction::None)
            }
            Modal::SessionPicker(mut model) => {
                let sessions = self
                    .runtime
                    .workstream_sessions(model.workstream_id)
                    .unwrap_or_default();
                match key.code {
                    KeyCode::Esc => self.modal = None,
                    KeyCode::Char('n') => {
                        self.modal = None;
                        return self
                            .open_launch_session_modal(model.workstream_id, model.launch_mode);
                    }
                    KeyCode::Down => {
                        if !sessions.is_empty() {
                            model.selected =
                                (model.selected + 1).min(sessions.len().saturating_sub(1));
                        }
                        self.modal = Some(Modal::SessionPicker(model));
                    }
                    KeyCode::Up => {
                        model.selected = model.selected.saturating_sub(1);
                        self.modal = Some(Modal::SessionPicker(model));
                    }
                    KeyCode::Enter => {
                        let Some(session) = sessions.get(model.selected).cloned() else {
                            self.set_status("No sessions are available.".to_string());
                            self.modal = Some(Modal::SessionPicker(model));
                            return Ok(DashboardAction::None);
                        };

                        self.modal = None;
                        self.runtime
                            .set_preferred_session(model.workstream_id, session.id)?;
                        self.select_workstream_by_id(model.workstream_id);

                        if matches!(model.launch_mode, LaunchMode::Attach) {
                            return Ok(DashboardAction::Attach(model.workstream_id, session.id));
                        }

                        let result = if self.ensure_workstream_available(model.workstream_id) {
                            self.runtime
                                .open_workstream_session(model.workstream_id, session.id)
                                .map(|_| format!("Opened {} in a new tab.", session.agent_preset))
                        } else {
                            Err(anyhow::anyhow!("That workstream is busy right now."))
                        };
                        self.set_status_from_result(result);
                    }
                    _ => self.modal = Some(Modal::SessionPicker(model)),
                }
                Ok(DashboardAction::None)
            }
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
                        Some(CreateBranchKind::New) => self.start_create_workstream(
                            flow.repo_id,
                            WorkstreamRequest::New {
                                branch: flow.new_branch_input.trim().to_string(),
                                base_branch: flow
                                    .selected_base_branch
                                    .clone()
                                    .unwrap_or_else(|| "main".to_string()),
                            },
                            &agent,
                        ),
                        Some(CreateBranchKind::Existing) => self.start_create_workstream(
                            flow.repo_id,
                            WorkstreamRequest::Existing {
                                branch: flow.selected_existing_branch.clone().unwrap_or_default(),
                            },
                            &agent,
                        ),
                        None => {
                            self.set_status("Choose a branch strategy first.".to_string());
                            return Ok(true);
                        }
                    };

                    match result {
                        Ok(()) => {
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
                self.start_load_branches(repo_id)?;
            }
            PaletteCommand::LaunchWorkstreamSession => {
                let Some(workstream_id) = self.selected_workstream_id() else {
                    self.set_status("Select a workstream first.".to_string());
                    return Ok(DashboardAction::None);
                };
                return self.open_launch_session_modal(workstream_id, LaunchMode::Stay);
            }
            PaletteCommand::RunWorkstreamOneOff => {
                let Some(workstream_id) = self.selected_workstream_id() else {
                    self.set_status("Select a workstream first.".to_string());
                    return Ok(DashboardAction::None);
                };
                return self.open_one_off_modal(workstream_id);
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
                self.start_refresh_repo(repo_id)?;
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
                        "Untrack `{}`?\nThis only removes it from mico. Launches must already be removed.",
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
                return self.open_or_pick_workstream(workstream_id, LaunchMode::Attach);
            }
            PaletteCommand::AttachWorkstream => {
                let Some(workstream_id) = self.selected_workstream_id() else {
                    self.set_status("Select a workstream first.".to_string());
                    return Ok(DashboardAction::None);
                };
                return self.open_or_pick_workstream(workstream_id, LaunchMode::Open);
            }
            PaletteCommand::ResumeWorkstream => {
                let Some(workstream_id) = self.selected_workstream_id() else {
                    self.set_status("Select a workstream first.".to_string());
                    return Ok(DashboardAction::None);
                };
                self.start_resume_workstream(workstream_id)?;
            }
            PaletteCommand::StopWorkstream => {
                let Some(workstream_id) = self.selected_workstream_id() else {
                    self.set_status("Select a workstream first.".to_string());
                    return Ok(DashboardAction::None);
                };
                self.start_stop_workstream(workstream_id)?;
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

    fn open_or_pick_selected_workstream(
        &mut self,
        launch_mode: LaunchMode,
    ) -> anyhow::Result<DashboardAction> {
        let Some(workstream_id) = self.selected_workstream_id() else {
            self.set_status("Select a workstream first.".to_string());
            return Ok(DashboardAction::None);
        };
        self.open_or_pick_workstream(workstream_id, launch_mode)
    }

    fn open_or_pick_workstream(
        &mut self,
        workstream_id: Uuid,
        launch_mode: LaunchMode,
    ) -> anyhow::Result<DashboardAction> {
        if !self.ensure_workstream_available(workstream_id) {
            return Ok(DashboardAction::None);
        }
        let workstream = self.runtime.workstream_by_id(workstream_id)?.clone();
        let Some(session) = workstream.preferred_session().cloned() else {
            self.set_status("This workstream does not have a session yet.".to_string());
            return Ok(DashboardAction::None);
        };

        if workstream.session_count() > 1 {
            self.modal = Some(Modal::SessionPicker(SessionPickerModal {
                workstream_id,
                branch: workstream.branch,
                launch_mode,
                selected: workstream
                    .sessions
                    .iter()
                    .position(|candidate| candidate.id == session.id)
                    .unwrap_or(0),
            }));
            return Ok(DashboardAction::None);
        }

        self.runtime
            .set_preferred_session(workstream_id, session.id)?;
        self.select_workstream_by_id(workstream_id);

        match launch_mode {
            LaunchMode::Attach => Ok(DashboardAction::Attach(workstream_id, session.id)),
            LaunchMode::Open => {
                let result = self
                    .runtime
                    .open_workstream_session(workstream_id, session.id)
                    .map(|_| "Opened workstream in a new tab.".to_string());
                self.set_status_from_result(result);
                Ok(DashboardAction::None)
            }
            LaunchMode::Stay => Ok(DashboardAction::None),
        }
    }

    fn open_launch_session_modal(
        &mut self,
        workstream_id: Uuid,
        launch_mode: LaunchMode,
    ) -> anyhow::Result<DashboardAction> {
        if !self.ensure_workstream_available(workstream_id) {
            return Ok(DashboardAction::None);
        }
        let workstream = self.runtime.workstream_by_id(workstream_id)?.clone();
        self.modal = Some(Modal::LaunchSession(LaunchSessionModal {
            workstream_id,
            branch: workstream.branch,
            launch_mode,
            selected: self
                .runtime
                .config
                .agent_presets
                .iter()
                .position(|preset| preset.name == workstream.agent_preset)
                .unwrap_or(0),
        }));
        Ok(DashboardAction::None)
    }

    fn open_one_off_modal(&mut self, workstream_id: Uuid) -> anyhow::Result<DashboardAction> {
        if !self.ensure_workstream_available(workstream_id) {
            return Ok(DashboardAction::None);
        }
        let workstream = self.runtime.workstream_by_id(workstream_id)?.clone();
        let agents = one_off_agent_names(&self.runtime);
        if agents.is_empty() {
            self.set_status("No one-off agents are configured.".to_string());
            return Ok(DashboardAction::None);
        }
        let selected = workstream
            .preferred_session()
            .and_then(|session| {
                agents
                    .iter()
                    .position(|agent| agent == &session.agent_preset)
            })
            .unwrap_or(0);
        let output = self.runtime.latest_one_off_detail(workstream_id)?;
        self.modal = Some(Modal::OneOff(OneOffModal {
            workstream_id,
            branch: workstream.branch,
            prompt: String::new(),
            output,
            selected,
        }));
        Ok(DashboardAction::None)
    }

    fn ensure_workstream_available(&mut self, workstream_id: Uuid) -> bool {
        if self.background_tasks.is_workstream_locked(workstream_id) {
            let label = self
                .runtime
                .workstream_by_id(workstream_id)
                .map(|workstream| workstream.branch.clone())
                .unwrap_or_else(|_| "workstream".to_string());
            self.set_status(format!("`{label}` is already busy with another action."));
            false
        } else {
            true
        }
    }

    fn ensure_repo_available(&mut self, repo_id: Uuid) -> bool {
        if self.background_tasks.is_repo_mutating(repo_id) {
            let label = self
                .runtime
                .repo_by_id(repo_id)
                .map(|repo| repo.display_name.clone())
                .unwrap_or_else(|_| "repo".to_string());
            self.set_status(format!(
                "`{label}` is already busy with another repo action."
            ));
            false
        } else {
            true
        }
    }

    fn start_load_branches(&mut self, repo_id: Uuid) -> anyhow::Result<()> {
        if !self.ensure_repo_available(repo_id) {
            return Ok(());
        }
        let repo = self.runtime.repo_by_id(repo_id)?.clone();
        self.background_tasks
            .submit(TaskRequest::LoadBranches { repo: repo.clone() })?;
        self.set_status(format!(
            "Loading branches for `{}` in the background.",
            repo.display_name
        ));
        Ok(())
    }

    fn start_refresh_repo(&mut self, repo_id: Uuid) -> anyhow::Result<()> {
        if !self.ensure_repo_available(repo_id) {
            return Ok(());
        }
        let repo = self.runtime.repo_by_id(repo_id)?.clone();
        let tracked_workstreams = self
            .runtime
            .state
            .workstreams
            .iter()
            .filter(|workstream| workstream.repo_id == repo_id)
            .map(|workstream| TrackedWorkstreamPath {
                workstream_id: workstream.id,
                worktree_path: workstream.worktree_path.clone(),
            })
            .collect::<Vec<_>>();
        self.background_tasks.submit(TaskRequest::RefreshRepo {
            repo: repo.clone(),
            tracked_workstreams,
        })?;
        self.set_status(format!(
            "Refreshing `{}` in the background.",
            repo.display_name
        ));
        Ok(())
    }

    fn start_create_workstream(
        &mut self,
        repo_id: Uuid,
        request: WorkstreamRequest,
        agent: &str,
    ) -> anyhow::Result<()> {
        if !self.ensure_repo_available(repo_id) {
            return Ok(());
        }
        let repo = self.runtime.repo_by_id(repo_id)?.clone();
        let tracked_worktree_paths = self
            .runtime
            .state
            .workstreams
            .iter()
            .filter(|workstream| workstream.repo_id == repo_id)
            .map(|workstream| workstream.worktree_path.clone())
            .collect::<Vec<_>>();
        self.background_tasks
            .submit(TaskRequest::CreateWorkstream {
                repo: repo.clone(),
                request,
                agent: agent.to_string(),
                tracked_worktree_paths,
            })?;
        self.set_status(format!(
            "Creating a workstream for `{}` in the background.",
            repo.display_name
        ));
        Ok(())
    }

    fn start_create_workstream_session(
        &mut self,
        workstream_id: Uuid,
        agent: &str,
        launch_mode: LaunchMode,
        attach_after: bool,
    ) -> anyhow::Result<()> {
        if !self.ensure_workstream_available(workstream_id) {
            return Ok(());
        }
        let workstream = self.runtime.workstream_by_id(workstream_id)?.clone();
        let repo = self.runtime.repo_by_id(workstream.repo_id)?.clone();
        let launch_target = if attach_after {
            SessionLaunchTarget::Attach
        } else {
            session_launch_target(launch_mode)
        };
        self.background_tasks
            .submit(TaskRequest::CreateWorkstreamSession {
                repo,
                workstream: workstream.clone(),
                agent: agent.to_string(),
                launch_target,
            })?;
        self.set_status(format!(
            "Launching a new session for `{}` in the background.",
            workstream.branch
        ));
        Ok(())
    }

    fn start_resume_workstream(&mut self, workstream_id: Uuid) -> anyhow::Result<()> {
        if !self.ensure_workstream_available(workstream_id) {
            return Ok(());
        }
        let workstream = self.runtime.workstream_by_id(workstream_id)?.clone();
        let session_id = self.runtime.preferred_session_id(workstream_id)?;
        let repo = self.runtime.repo_by_id(workstream.repo_id)?.clone();
        self.background_tasks
            .submit(TaskRequest::ResumeWorkstreamSession {
                repo,
                workstream: workstream.clone(),
                session_id,
                launch_target: SessionLaunchTarget::Stay,
            })?;
        self.set_status(format!(
            "Resuming `{}` in the background.",
            workstream.branch
        ));
        Ok(())
    }

    fn start_stop_workstream(&mut self, workstream_id: Uuid) -> anyhow::Result<()> {
        if !self.ensure_workstream_available(workstream_id) {
            return Ok(());
        }
        let workstream = self.runtime.workstream_by_id(workstream_id)?.clone();
        self.background_tasks.submit(TaskRequest::StopWorkstream {
            workstream: workstream.clone(),
        })?;
        self.set_status(format!(
            "Stopping `{}` in the background.",
            workstream.branch
        ));
        Ok(())
    }

    fn start_remove_workstream(&mut self, workstream_id: Uuid) -> anyhow::Result<()> {
        if !self.ensure_workstream_available(workstream_id) {
            return Ok(());
        }
        let workstream = self.runtime.workstream_by_id(workstream_id)?.clone();
        let repo = self.runtime.repo_by_id(workstream.repo_id)?.clone();
        if !self.ensure_repo_available(repo.id) {
            return Ok(());
        }
        self.background_tasks
            .submit(TaskRequest::RemoveWorkstream {
                repo,
                workstream: workstream.clone(),
            })?;
        self.set_status(format!(
            "Removing `{}` in the background.",
            workstream.branch
        ));
        Ok(())
    }

    fn start_one_off_task(
        &mut self,
        workstream_id: Uuid,
        agent: &str,
        prompt: &str,
    ) -> anyhow::Result<()> {
        if !self.ensure_workstream_available(workstream_id) {
            return Ok(());
        }
        let workstream = self.runtime.workstream_by_id(workstream_id)?.clone();
        self.background_tasks.submit(TaskRequest::RunOneOff {
            workstream: workstream.clone(),
            agent: agent.to_string(),
            prompt: prompt.to_string(),
        })?;
        self.set_status(format!(
            "Running a one-off {} command for `{}` in the background.",
            agent, workstream.branch
        ));
        Ok(())
    }

    fn process_deferred_actions(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> anyhow::Result<()> {
        while let Some(action) = self.deferred_actions.pop_front() {
            match action {
                DeferredAction::Open(workstream_id, session_id) => {
                    let result = self
                        .runtime
                        .open_workstream_session(workstream_id, session_id)
                        .map(|_| "Opened workstream in a new tab.".to_string());
                    self.set_status_from_result(result);
                }
                DeferredAction::Attach(workstream_id, session_id) => {
                    suspend_terminal(terminal)?;
                    let result = self
                        .runtime
                        .attach_workstream_session(workstream_id, session_id);
                    resume_terminal(terminal)?;
                    self.set_status_from_result(
                        result.map(|_| "Detached from workstream.".to_string()),
                    );
                }
            }
        }
        Ok(())
    }

    fn poll_background_tasks(&mut self) {
        for update in self.background_tasks.drain_updates() {
            match update.result {
                Ok(success) => {
                    self.apply_background_success(success, update.persisted_completion_id)
                }
                Err(error) => {
                    let error_text = error.to_string();
                    if let Some(Modal::OneOff(model)) = self.modal.as_mut() {
                        let matches_workstream = update.locks.iter().any(
                            |lock| matches!(lock, TaskLock::Workstream(id) if *id == model.workstream_id),
                        );
                        if matches_workstream {
                            model.output = Some(error_text.clone());
                        }
                    }
                    if let Some(workstream_id) = update.locks.iter().find_map(|lock| match lock {
                        TaskLock::Workstream(id) => Some(*id),
                        TaskLock::RepoMutation(_) => None,
                    }) {
                        let summary = format!("{} failed: {error_text}", update.label);
                        if let Err(record_error) = self.runtime.record_attention(
                            workstream_id,
                            AttentionReason::TaskFailed,
                            summary.clone(),
                            true,
                        ) {
                            self.status = Some(StatusMessage {
                                text: format!(
                                    "{summary} (and failed to record attention: {record_error})"
                                ),
                                tone: StatusTone::Error,
                            });
                            continue;
                        }
                    }
                    self.status = Some(StatusMessage {
                        text: format!("{} failed: {error_text}", update.label),
                        tone: StatusTone::Error,
                    });
                }
            }
        }

        self.sync_repo_selection();
        self.sync_workstream_selection();
    }

    fn apply_background_success(
        &mut self,
        success: TaskSuccess,
        persisted_completion_id: Option<Uuid>,
    ) {
        let result = match success {
            TaskSuccess::BranchesLoaded {
                repo_id,
                repo_name,
                branches,
            } => {
                self.select_repo_by_id(repo_id);
                self.modal = Some(Modal::CreateWorkstream(CreateWorkstreamFlow::new(
                    repo_id,
                    repo_name.clone(),
                    branches,
                )));
                Ok(format!("Loaded branches for `{repo_name}`."))
            }
            TaskSuccess::RepoRefreshed {
                repo_id,
                branch_updates,
            } => {
                let repo_name = self
                    .runtime
                    .repo_by_id(repo_id)
                    .map(|repo| repo.display_name.clone())
                    .unwrap_or_else(|_| "repo".to_string());
                self.runtime
                    .apply_branch_updates(branch_updates)
                    .map(|_| format!("Refreshed `{repo_name}`."))
            }
            TaskSuccess::WorkstreamCreated { workstream } => {
                let branch = workstream.branch.clone();
                let workstream_id = workstream.id;
                self.runtime.apply_created_workstream(workstream).map(|_| {
                    self.select_workstream_by_id(workstream_id);
                    format!("Created workstream `{branch}`.")
                })
            }
            TaskSuccess::WorkstreamSessionCreated {
                workstream_id,
                session,
                launch_target,
            } => {
                let branch = self
                    .runtime
                    .workstream_by_id(workstream_id)
                    .map(|workstream| workstream.branch.clone())
                    .unwrap_or_else(|_| "workstream".to_string());
                let agent = session.agent_preset.clone();
                let session_id = session.id;
                self.runtime
                    .apply_created_workstream_session(workstream_id, session)
                    .map(|_| {
                        self.schedule_launch_action(launch_target, workstream_id, session_id);
                        format!("Launched {agent} for `{branch}`.")
                    })
            }
            TaskSuccess::WorkstreamSessionResumed {
                workstream_id,
                session_id,
                launch_target,
            } => {
                let branch = self
                    .runtime
                    .workstream_by_id(workstream_id)
                    .map(|workstream| workstream.branch.clone())
                    .unwrap_or_else(|_| "workstream".to_string());
                self.runtime
                    .apply_resumed_workstream_session(workstream_id, session_id)
                    .map(|_| {
                        self.schedule_launch_action(launch_target, workstream_id, session_id);
                        format!("Resumed `{branch}`.")
                    })
            }
            TaskSuccess::WorkstreamStopped { workstream_id } => self
                .runtime
                .apply_stopped_workstream(workstream_id)
                .map(|branch| format!("Stopped `{branch}`.")),
            TaskSuccess::WorkstreamRemoved { workstream_id } => self
                .runtime
                .apply_removed_workstream(workstream_id)
                .map(|branch| format!("Removed `{branch}`.")),
            TaskSuccess::OneOffCompleted {
                workstream_id,
                output,
            } => {
                if let Some(Modal::OneOff(model)) = self.modal.as_mut()
                    && model.workstream_id == workstream_id
                {
                    model.output = Some(output.clone());
                }
                let branch = self
                    .runtime
                    .workstream_by_id(workstream_id)
                    .map(|workstream| workstream.branch.clone())
                    .unwrap_or_else(|_| "workstream".to_string());
                self.runtime
                    .record_attention_with_detail(
                        workstream_id,
                        AttentionReason::OneOffCompleted,
                        format!("One-off completed for `{branch}`."),
                        Some(output.clone()),
                        true,
                    )
                    .map(|_| {
                        format!(
                            "One-off completed for `{branch}`. Select the launch to inspect the result."
                        )
                    })
            }
        };

        match result {
            Ok(message) => {
                if let Some(completion_id) = persisted_completion_id
                    && let Err(error) = self.runtime.completion_store().remove(completion_id)
                {
                    self.status = Some(StatusMessage {
                        text: format!(
                            "Applied background task but failed to clear completion record: {error}"
                        ),
                        tone: StatusTone::Error,
                    });
                    return;
                }
                self.set_status_good(message)
            }
            Err(error) => {
                self.status = Some(StatusMessage {
                    text: error.to_string(),
                    tone: StatusTone::Error,
                });
            }
        }
    }

    fn schedule_launch_action(
        &mut self,
        launch_target: SessionLaunchTarget,
        workstream_id: Uuid,
        session_id: Uuid,
    ) {
        match launch_target {
            SessionLaunchTarget::Stay => {}
            SessionLaunchTarget::Open => self
                .deferred_actions
                .push_back(DeferredAction::Open(workstream_id, session_id)),
            SessionLaunchTarget::Attach => self
                .deferred_actions
                .push_back(DeferredAction::Attach(workstream_id, session_id)),
        }
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
                WorkstreamView::NeedsYou => workstream.has_unread_attention(),
                WorkstreamView::Running => matches!(workstream.status, WorkstreamStatus::Running),
                WorkstreamView::Stopped => matches!(workstream.status, WorkstreamStatus::Stopped),
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

    fn set_status_good(&mut self, message: String) {
        self.status = Some(StatusMessage {
            text: message,
            tone: StatusTone::Good,
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

    fn refresh_attention_signals(&mut self) {
        let now = Instant::now();
        if let Some(last_refresh) = self.last_attention_refresh
            && now.duration_since(last_refresh) < Duration::from_secs(15)
        {
            return;
        }

        self.last_attention_refresh = Some(now);
        if let Err(error) = self.runtime.reconcile_workstream_output_activity() {
            self.status = Some(StatusMessage {
                text: format!("Failed to refresh launch attention: {error}"),
                tone: StatusTone::Error,
            });
        }
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

fn one_off_agent_names(runtime: &MicoRuntime) -> Vec<String> {
    runtime
        .config
        .agent_presets
        .iter()
        .filter(|preset| preset.one_off_command.is_some())
        .map(|preset| preset.name.clone())
        .collect()
}

fn session_launch_target(launch_mode: LaunchMode) -> SessionLaunchTarget {
    match launch_mode {
        LaunchMode::Stay => SessionLaunchTarget::Stay,
        LaunchMode::Open => SessionLaunchTarget::Open,
        LaunchMode::Attach => SessionLaunchTarget::Attach,
    }
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

fn session_picker_item(session: &WorkstreamSession) -> ListItem<'static> {
    let status = match session.status {
        WorkstreamStatus::Running => "running",
        WorkstreamStatus::Stopped => "stopped",
    };
    let status_color = if matches!(session.status, WorkstreamStatus::Running) {
        SUCCESS_GREEN
    } else {
        WARNING_AMBER
    };
    ListItem::new(vec![
        Line::from(vec![
            Span::styled(
                session.agent_preset.clone(),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                format!("[{}]", status.to_uppercase()),
                Style::default()
                    .fg(status_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        line_from_stat_pairs(&[
            ("session".to_string(), session.session_name.clone()),
            (
                "last touch".to_string(),
                option_elapsed_label(
                    session
                        .last_attached_at_epoch_secs
                        .or(session.last_opened_at_epoch_secs),
                ),
            ),
        ]),
        Line::from(""),
    ])
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
    match sort {
        WorkstreamSort::Attention => {
            let rank_cmp = workstream_attention_rank(left).cmp(&workstream_attention_rank(right));
            if rank_cmp != Ordering::Equal {
                return rank_cmp;
            }

            latest_activity_epoch_secs(right)
                .cmp(&latest_activity_epoch_secs(left))
                .then_with(|| left.branch.cmp(&right.branch))
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
    if let Some(event) = workstream.latest_unread_attention_event() {
        return attention_reason_rank(&event.reason);
    }

    match workstream.status {
        WorkstreamStatus::Running => 4,
        WorkstreamStatus::Stopped => 5,
    }
}

fn latest_activity_epoch_secs(workstream: &Workstream) -> u64 {
    workstream.latest_activity_epoch_secs()
}

fn workstream_state_label(workstream: &Workstream) -> &'static str {
    if let Some(event) = workstream.latest_unread_attention_event() {
        return attention_reason_state_label(&event.reason);
    }

    match workstream.status {
        WorkstreamStatus::Running => "running",
        WorkstreamStatus::Stopped => "stopped",
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

    if let Some(event) = workstream.latest_unread_attention_event() {
        chips.push(label_chip(
            attention_reason_chip(&event.reason),
            attention_reason_color(&event.reason),
        ));
        if workstream.unread_attention_count() > 1 {
            chips.push(dynamic_label_chip(
                format!("NEW {}", workstream.unread_attention_count()),
                INFO_BLUE,
            ));
        }
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

fn dynamic_label_chip(label: String, color: Color) -> Span<'static> {
    Span::styled(
        format!("[{label}]"),
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    )
}

fn has_unread_reason(workstream: &Workstream, reason: AttentionReason) -> bool {
    workstream
        .attention_events
        .iter()
        .any(|event| !event.seen && event.reason == reason)
}

fn attention_reason_rank(reason: &AttentionReason) -> u8 {
    match reason {
        AttentionReason::TaskFailed => 0,
        AttentionReason::SessionStopped => 1,
        AttentionReason::IdleOutput => 2,
        AttentionReason::BranchChanged => 3,
        AttentionReason::OneOffCompleted => 4,
    }
}

fn attention_reason_state_label(reason: &AttentionReason) -> &'static str {
    match reason {
        AttentionReason::TaskFailed => "failed",
        AttentionReason::OneOffCompleted => "done",
        AttentionReason::SessionStopped => "waiting",
        AttentionReason::IdleOutput => "idle",
        AttentionReason::BranchChanged => "drifted",
    }
}

fn attention_reason_label(reason: &AttentionReason) -> &'static str {
    match reason {
        AttentionReason::TaskFailed => "FAILED",
        AttentionReason::OneOffCompleted => "DONE",
        AttentionReason::SessionStopped => "WAITING",
        AttentionReason::IdleOutput => "IDLE",
        AttentionReason::BranchChanged => "DRIFT",
    }
}

fn attention_reason_chip(reason: &AttentionReason) -> &'static str {
    attention_reason_label(reason)
}

fn attention_reason_color(reason: &AttentionReason) -> Color {
    match reason {
        AttentionReason::TaskFailed => ERROR_RED,
        AttentionReason::OneOffCompleted => SUCCESS_GREEN,
        AttentionReason::SessionStopped => WARNING_AMBER,
        AttentionReason::IdleOutput => WARNING_AMBER,
        AttentionReason::BranchChanged => INFO_BLUE,
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
