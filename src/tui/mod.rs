use std::{
    io::{self, Stdout},
    path::PathBuf,
};

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};
use uuid::Uuid;

use crate::{
    app::runtime::{LaunchMode, MicoRuntime},
    domain::model::WorkstreamStatus,
};

const MICO_BANNER: [&str; 6] = [
    "                              ",
    " _ __ ___  _  ___ ___         ",
    "| '_ ` _ \\| |/ __/ _ \\        ",
    "| | | | | | | (_| (_) |       ",
    "|_| |_| |_|_|\\___\\___/        ",
    "                              ",
];

const PURPLE_A: Color = Color::Rgb(204, 88, 255);
const PURPLE_B: Color = Color::Rgb(228, 72, 255);
const PURPLE_C: Color = Color::Rgb(250, 58, 255);
const PANEL_HIGHLIGHT: Color = Color::Rgb(22, 33, 46);

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

        if let Event::Key(key) = event::read()?
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

#[derive(Debug, Clone, Copy)]
enum DashboardAction {
    None,
    Quit,
    Attach(Uuid),
}

#[derive(Debug, Clone, Copy)]
enum PaletteCommand {
    AddRepo,
    CreateWorkstream,
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

const PALETTE_ENTRIES: [PaletteEntry; 10] = [
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
        detail: "Open the selected workstream in iTerm.",
    },
    PaletteEntry {
        command: PaletteCommand::AttachWorkstream,
        title: "Attach selected workstream",
        detail: "Attach it in this terminal. Detach with Ctrl-b d to return to mico.",
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
        detail: "Delete the selected worktree, tmux session, and local record.",
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
}

#[derive(Debug, Clone)]
struct AddRepoModal {
    input: String,
}

#[derive(Debug, Clone)]
struct ConfirmModal {
    title: String,
    body: String,
    action: ConfirmAction,
}

#[derive(Debug, Clone, Copy)]
enum ConfirmAction {
    RemoveRepo(Uuid),
    RemoveWorkstream(Uuid),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CreateStep {
    BaseBranch,
    BranchMode,
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
            step: CreateStep::BaseBranch,
        }
    }
}

struct DashboardApp {
    runtime: MicoRuntime,
    focus: FocusPane,
    repo_state: ListState,
    workstream_state: ListState,
    palette: Option<PaletteStateModel>,
    modal: Option<Modal>,
    status: Option<String>,
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
            palette: None,
            modal: None,
            status: None,
        }
    }

    fn render(&mut self, frame: &mut Frame<'_>) {
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

        let banner_lines = MICO_BANNER
            .iter()
            .enumerate()
            .map(|(index, line)| {
                let color = match index {
                    0 => PURPLE_A,
                    1 => PURPLE_B,
                    2 => PURPLE_C,
                    3 => PURPLE_B,
                    _ => PURPLE_A,
                };
                Line::styled(
                    *line,
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                )
            })
            .collect::<Vec<_>>();

        let banner = Paragraph::new(Text::from(banner_lines)).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Mission Control"),
        );

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
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title("Flight Deck"));

        frame.render_widget(banner, header[0]);
        frame.render_widget(title, header[1]);
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
            .title("Repos [Enter] [A] [g] [D]")
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

        let workstream_items: Vec<ListItem<'_>> = if self.runtime.state.workstreams.is_empty() {
            vec![ListItem::new(
                "No workstreams yet. Select a repo, open the palette, and create one.",
            )]
        } else {
            let selected_index = self.workstream_state.selected();
            self.runtime
                .state
                .workstreams
                .iter()
                .enumerate()
                .map(|(index, workstream)| {
                    let status = match workstream.status {
                        WorkstreamStatus::Running => "running",
                        WorkstreamStatus::Stopped => "stopped",
                    };
                    let repo_name = self
                        .runtime
                        .state
                        .repos
                        .iter()
                        .find(|repo| repo.id == workstream.repo_id)
                        .map(|repo| repo.display_name.as_str())
                        .unwrap_or("<missing repo>");

                    let mut lines = vec![
                        Line::styled(
                            format!("{} [{}]", workstream.branch, status),
                            Style::default().add_modifier(Modifier::BOLD),
                        ),
                        Line::from(format!(
                            "repo: {}   agent: {}",
                            repo_name, workstream.agent_preset
                        )),
                        Line::from(format!("session: {}", workstream.session_name)),
                    ];

                    if selected_index == Some(index) {
                        let hint = match workstream.status {
                            WorkstreamStatus::Running => Line::styled(
                                "a attach here   o open in iTerm   Ctrl-b d return to mico",
                                Style::default().fg(PURPLE_B),
                            ),
                            WorkstreamStatus::Stopped => Line::styled(
                                "r resume session   a resume+attach   o resume+open",
                                Style::default().fg(PURPLE_B),
                            ),
                        };
                        lines.push(hint);
                    }

                    ListItem::new(lines)
                })
                .collect()
        };

        let workstream_block = Block::default()
            .borders(Borders::ALL)
            .title("Workstreams [Enter] [a] [r] [x] [D]")
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
        frame.render_stateful_widget(workstream_list, chunks[1], &mut self.workstream_state);
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
        let status = self.status.clone().unwrap_or_else(|| "Ready.".to_string());
        let context_line = match self.focus {
            FocusPane::Repos => line_from_pairs(&[
                ("Tab", "switch to workstreams"),
                ("Enter", "create workstream from selected repo"),
                ("A", "add repo"),
                ("g", "refresh selected repo"),
                ("Shift+D", "remove repo"),
                (":", "all commands"),
            ]),
            FocusPane::Workstreams => line_from_pairs(&[
                ("Tab", "switch to repos"),
                ("Enter", "open selected workstream"),
                ("a", "attach here"),
                ("r", "resume session"),
                ("Ctrl-b d", "return"),
                ("x", "stop"),
                ("Shift+D", "remove"),
                ("o", "open in iTerm"),
            ]),
        };
        let global_line = line_from_pairs(&[
            ("j/k", "move"),
            ("Esc", "back or quit"),
            ("q", "quit dashboard"),
            ("n", "new workstream"),
            (":", "command palette"),
        ]);

        let info = Paragraph::new(Text::from(vec![
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
                .title(format!("Status: {status}")),
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
            "Create workstream for {}\nChoose a base branch, then either create a new branch or pick an existing one.",
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
            CreateStep::BaseBranch => format!("Base Branch Filter: {}", flow.base_filter),
            CreateStep::BranchMode => "Branch Strategy: new branch or existing branch".to_string(),
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
            CreateStep::BaseBranch => {
                self.render_picker_list(
                    frame,
                    layout[2],
                    "Base Branches",
                    &filtered_strings(&flow.branches, &flow.base_filter),
                    flow.base_selected,
                );
            }
            CreateStep::BranchMode => {
                let options = vec![
                    "New branch from selected base".to_string(),
                    "Use an existing branch".to_string(),
                ];
                self.render_picker_list(
                    frame,
                    layout[2],
                    "Branch Strategy",
                    &options,
                    flow.branch_mode_selected,
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
                    .map(|preset| format!("{} -> {}", preset.name, preset.command))
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
            return self.handle_palette_key(key);
        }

        if self.modal.is_some() {
            return self.handle_modal_key(key);
        }

        self.handle_dashboard_key(key)
    }

    fn handle_dashboard_key(&mut self, key: KeyEvent) -> anyhow::Result<DashboardAction> {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => Ok(DashboardAction::Quit),
            KeyCode::Enter => match self.focus {
                FocusPane::Repos => self.activate_palette_command(PaletteCommand::CreateWorkstream),
                FocusPane::Workstreams => {
                    self.activate_palette_command(PaletteCommand::OpenWorkstream)
                }
            },
            KeyCode::Tab => {
                self.focus = if self.focus == FocusPane::Repos {
                    FocusPane::Workstreams
                } else {
                    FocusPane::Repos
                };
                Ok(DashboardAction::None)
            }
            KeyCode::Char(':') => {
                self.palette = Some(PaletteStateModel::default());
                Ok(DashboardAction::None)
            }
            KeyCode::Char('n') => self.activate_palette_command(PaletteCommand::CreateWorkstream),
            KeyCode::Char('A') => self.activate_palette_command(PaletteCommand::AddRepo),
            KeyCode::Char('g') => self.activate_palette_command(PaletteCommand::RefreshRepo),
            KeyCode::Char('r') => {
                if self.focus == FocusPane::Workstreams {
                    self.activate_palette_command(PaletteCommand::ResumeWorkstream)
                } else {
                    Ok(DashboardAction::None)
                }
            }
            KeyCode::Char('D') => match self.focus {
                FocusPane::Repos => self.activate_palette_command(PaletteCommand::RemoveRepo),
                FocusPane::Workstreams => {
                    self.activate_palette_command(PaletteCommand::RemoveWorkstream)
                }
            },
            KeyCode::Down | KeyCode::Char('j') => {
                self.move_selection(1);
                Ok(DashboardAction::None)
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.move_selection(-1);
                Ok(DashboardAction::None)
            }
            KeyCode::Char('o') => {
                if let Some(id) = self.selected_workstream_id() {
                    let result = self
                        .runtime
                        .open_workstream(id)
                        .map(|_| "Opened workstream in iTerm.".to_string());
                    self.set_status_from_result(result);
                } else {
                    self.set_status("Select a workstream first.".to_string());
                }
                Ok(DashboardAction::None)
            }
            KeyCode::Char('a') => {
                if let Some(id) = self.selected_workstream_id() {
                    Ok(DashboardAction::Attach(id))
                } else {
                    self.set_status("Select a workstream first.".to_string());
                    Ok(DashboardAction::None)
                }
            }
            KeyCode::Char('x') => {
                if let Some(id) = self.selected_workstream_id() {
                    let result = self
                        .runtime
                        .stop_workstream(id)
                        .map(|branch| format!("Stopped `{branch}`."));
                    self.set_status_from_result(result);
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
        }
    }

    fn handle_create_workstream_key(
        &mut self,
        flow: &mut CreateWorkstreamFlow,
        key: KeyEvent,
    ) -> anyhow::Result<bool> {
        match flow.step {
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
                        flow.step = CreateStep::BranchMode;
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
                        CreateStep::NewBranch
                    } else {
                        CreateStep::ExistingBranch
                    };
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
                        Some(CreateBranchKind::New) => self.runtime.create_workstream_new(
                            flow.repo_id,
                            flow.selected_base_branch.as_deref().unwrap_or("main"),
                            flow.new_branch_input.trim(),
                            &agent,
                            LaunchMode::Stay,
                        ),
                        Some(CreateBranchKind::Existing) => {
                            self.runtime.create_workstream_existing(
                                flow.repo_id,
                                flow.selected_existing_branch.as_deref().unwrap_or(""),
                                &agent,
                                LaunchMode::Stay,
                            )
                        }
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
                let result = self
                    .runtime
                    .open_workstream(workstream_id)
                    .map(|_| "Opened workstream in iTerm.".to_string());
                self.set_status_from_result(result);
            }
            PaletteCommand::AttachWorkstream => {
                let Some(workstream_id) = self.selected_workstream_id() else {
                    self.set_status("Select a workstream first.".to_string());
                    return Ok(DashboardAction::None);
                };
                return Ok(DashboardAction::Attach(workstream_id));
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
                self.modal = Some(Modal::Confirm(ConfirmModal {
                    title: "Remove Workstream".to_string(),
                    body: format!(
                        "Remove `{}`?\nThis deletes the worktree directory and stops its tmux session.",
                        workstream.branch
                    ),
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
                let len = self.runtime.state.workstreams.len();
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
        self.workstream_state
            .selected()
            .and_then(|index| self.runtime.state.workstreams.get(index))
            .map(|workstream| workstream.id)
    }

    fn selected_repo(&self) -> Option<&crate::domain::model::RepoTarget> {
        self.repo_state
            .selected()
            .and_then(|index| self.runtime.state.repos.get(index))
    }

    fn selected_workstream(&self) -> Option<&crate::domain::model::Workstream> {
        self.workstream_state
            .selected()
            .and_then(|index| self.runtime.state.workstreams.get(index))
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
            .runtime
            .state
            .workstreams
            .iter()
            .position(|workstream| workstream.id == workstream_id)
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
        let len = self.runtime.state.workstreams.len();
        self.workstream_state
            .select(normalize_selection(self.workstream_state.selected(), len));
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
            CreateStep::BaseBranch => CreateStep::BaseBranch,
            CreateStep::BranchMode => CreateStep::BaseBranch,
            CreateStep::ExistingBranch => CreateStep::BranchMode,
            CreateStep::NewBranch => CreateStep::BranchMode,
            CreateStep::Agent => match flow.branch_kind {
                Some(CreateBranchKind::New) => CreateStep::NewBranch,
                Some(CreateBranchKind::Existing) => CreateStep::ExistingBranch,
                None => CreateStep::BranchMode,
            },
        };
    }

    fn set_status(&mut self, message: String) {
        self.status = Some(message);
    }

    fn set_status_from_result(&mut self, result: anyhow::Result<String>) {
        match result {
            Ok(message) => self.set_status(message),
            Err(error) => self.set_status(error.to_string()),
        }
        self.sync_repo_selection();
        self.sync_workstream_selection();
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

fn key_chip(key: &str) -> Span<'static> {
    Span::styled(
        format!(" {key} "),
        Style::default()
            .fg(Color::Black)
            .bg(PURPLE_B)
            .add_modifier(Modifier::BOLD),
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
