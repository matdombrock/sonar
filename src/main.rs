use color_eyre::eyre::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use ratatui::{
    Frame, Terminal,
    layout::{Constraint, Rect},
    prelude::Backend,
    text::Line,
    widgets::{Clear, Wrap},
};
use ratatui::{backend::CrosstermBackend, layout::Layout};
use ratatui::{
    layout::Direction,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use ratatui::style::{Color, Style};
use ratatui::text::{Span, Text};
use std::os::unix::fs::PermissionsExt;

const DIR_PRETTY_LIMIT: usize = 1000;
const SEARCH_LIMIT: usize = 1000;

const NF_MAG: &str = "󰍉";
const NF_LOOK: &str = "";
const NF_SEL: &str = ""; //➤
const NF_DIR: &str = "";
const NF_DIRO: &str = "󰉒";
const NF_FILE: &str = "";
const NF_CMD: &str = "";
const NF_INFO: &str = "";
const NF_CHECK: &str = "";

// Shortcut strings
const SC_DIR_UP: &str = " .. up";
const SC_EXIT: &str = " exit";
const SC_HOME: &str = "~ home";
const SC_DIR_BACK: &str = " back";
const SC_EXP: &str = " explode";

mod cmd_name {
    pub const EXIT: &str = ":exit";
    pub const HOME: &str = ":home";
    pub const SEL_UP: &str = ":sel-up";
    pub const SEL_DOWN: &str = ":sel-down";
    pub const DIR_UP: &str = ":dir-up";
    pub const DIR_BACK: &str = ":dir-back";
    pub const EXPLODE: &str = ":explode";
    pub const SELECT: &str = ":select";
    pub const CMD_TOGGLE: &str = ":cmd-toggle";
    pub const MULTI_SEL: &str = ":multi-sel";
    pub const MULTI_CLEAR: &str = ":multi-clear";
}

#[derive(Clone)]
struct ItemInfo {
    name: String,
    is_sc: bool,
    metadata: fs::Metadata,
}

enum LoopReturn {
    Continue,
    Break,
    Ok,
}

struct App<'a> {
    input: String,
    dir_listing: Vec<ItemInfo>,
    results: Vec<ItemInfo>,
    selection: String,
    selection_index: i32,
    multi_selection: Vec<PathBuf>,
    preview_content: Text<'a>,
    cwd: PathBuf,
    lwd: PathBuf,
    mode_explode: bool,
    command_window_open: bool,
    command_input: String,
}
impl<'a> App<'a> {
    fn new() -> Self {
        Self {
            input: String::new(),
            dir_listing: Vec::new(),
            results: Vec::new(),
            selection: String::new(),
            selection_index: 0,
            multi_selection: Vec::new(),
            preview_content: Default::default(),
            cwd: env::current_dir().unwrap(),
            lwd: env::current_dir().unwrap(),
            mode_explode: false,
            command_window_open: false,
            command_input: String::new(),
        }
    }

    fn set_cwd(&mut self, path: &PathBuf) {
        let new_path = if path.to_str().unwrap() == ".." {
            self.cwd.parent().unwrap_or(&self.cwd).to_path_buf()
        } else {
            let mut temp_path = self.cwd.clone();
            temp_path.push(path);
            temp_path
        };
        self.lwd = self.cwd.clone();
        self.cwd = new_path;
    }

    fn get_directory_listing(&self, path: &PathBuf) -> Vec<ItemInfo> {
        let mut entries = Vec::new();

        match fs::read_dir(path) {
            Ok(read_dir) => {
                for entry_result in read_dir {
                    if let Ok(entry) = entry_result {
                        let file_name = entry.file_name();
                        let file_name_str = file_name.to_string_lossy();
                        match entry.metadata() {
                            Ok(metadata) => {
                                if !self.mode_explode {
                                    entries.push(ItemInfo {
                                        name: file_name_str.to_string(),
                                        is_sc: false,
                                        metadata,
                                    });
                                } else {
                                    let sub_path = entry.path();
                                    if metadata.is_dir() {
                                        // Recursively collect files from subdirectory
                                        let sub_entries = self.get_directory_listing(&sub_path);
                                        entries.extend(sub_entries);
                                    } else {
                                        entries.push(ItemInfo {
                                            name: sub_path.to_str().unwrap().to_string(),
                                            is_sc: false,
                                            metadata,
                                        });
                                    }
                                }
                            }
                            Err(_) => {
                                // Optionally log or handle metadata errors
                                continue;
                            }
                        }
                    }
                }
            }
            Err(_) => {
                // Optionally log or handle directory read errors
            }
        }
        entries
    }

    fn fmtln_info(label: &str, value: &str) -> Line<'a> {
        Line::styled(
            format!("{} {}: {}", NF_INFO, label, value),
            Style::default().fg(Color::Yellow),
        )
    }
    fn fmtln_path(path: &PathBuf) -> Line<'a> {
        Line::styled(
            format!("{} {}", NF_DIRO, path.to_str().unwrap()),
            Style::default().fg(Color::Blue),
        )
    }
    fn fmtln_sc(description: &str) -> Line<'a> {
        Line::styled(
            format!("{} {}", NF_CMD, description),
            Style::default().fg(Color::Green),
        )
    }

    fn dir_list_pretty(&self, list: &Vec<ItemInfo>) -> Text<'a> {
        let mut text = Text::default();
        for item in list.iter().take(DIR_PRETTY_LIMIT) {
            // Check if this item is part of the multi selection
            let mut ms = "";
            let mut is_multi_selected = false;
            let mut selected_path = self.cwd.clone();
            selected_path.push(&item.name);
            for ms_item in self.multi_selection.iter() {
                if *ms_item == selected_path {
                    is_multi_selected = true;
                    break;
                }
            }
            let ms_on = format!("{} ", NF_CHECK);
            if is_multi_selected {
                ms = &ms_on;
            }
            // Limit for performance
            let line = if item.is_sc {
                Line::styled(
                    format!("{}{} {}", ms, NF_CMD, item.name),
                    Style::default().fg(Color::Yellow),
                )
            } else if item.metadata.is_dir() {
                Line::styled(
                    format!("{}{} {}/", ms, NF_DIR, item.name),
                    Style::default().fg(Color::Green),
                )
            } else {
                // When exploded the item name is the full path
                // Remove the cwd prefix for better readability
                let name = if item.name.starts_with(self.cwd.to_str().unwrap()) {
                    item.name[self.cwd.to_str().unwrap().len()..].to_string()
                } else {
                    item.name.clone()
                };
                Line::styled(
                    format!("{}{} {}", ms, NF_FILE, name),
                    Style::default().fg(Color::Cyan),
                )
            };
            text.lines.push(Line::from(line));
        }
        text
    }

    fn preview_dir(&mut self, selected_path: &PathBuf) {
        let path_line = App::fmtln_path(&selected_path);
        self.preview_content += path_line;
        // Get the file metadata
        let metadata = fs::metadata(&selected_path);
        if let Ok(meta) = metadata {
            // Get permissions
            let permissions = meta.permissions();
            let perm_line = App::fmtln_info("permissions", &format!("{:o}", permissions.mode()));
            self.preview_content += perm_line;
        }
        let listing = self.get_directory_listing(&selected_path);
        let count_line = App::fmtln_info("count", &listing.len().to_string());
        self.preview_content += count_line;
        self.preview_content += Line::from("-------");
        let pretty_listing = self.dir_list_pretty(&listing);
        for line in pretty_listing.lines.iter().take(20) {
            self.preview_content += Line::from(line.clone());
        }
    }

    fn preview_file(&mut self, selected_path: &PathBuf) {
        let path_line = App::fmtln_path(&selected_path);
        self.preview_content += path_line;
        // Get the file metadata
        let metadata = fs::metadata(&selected_path);
        if let Ok(meta) = metadata {
            // Get permissions
            let permissions = meta.permissions();
            let perm_line = App::fmtln_info("permissions", &format!("{:o}", permissions.mode()));
            self.preview_content += perm_line;
            // Get mime type
            if meta.file_type().is_file() {
                // Get mimetype using mime_guess
                let mime = mime_guess::from_path(&selected_path).first_or_octet_stream();
                let mime_line = App::fmtln_info("mime", &mime.to_string());
                self.preview_content += mime_line;
            }
        }
        self.preview_content += Line::from("-------");
        // Read file content (first 100 lines)
        if let Ok(content) = fs::read_to_string(&selected_path) {
            for line in content.lines().take(100) {
                self.preview_content += Line::from(line.to_string());
            }
        } else {
            self.preview_content += Line::from("Unable to read file content.");
        }
    }

    fn update_directory_listing(&mut self) {
        let mut listing = self.get_directory_listing(&self.cwd.clone());
        let empty_metadata = fs::metadata(&self.cwd).unwrap();
        listing.insert(
            0,
            ItemInfo {
                name: SC_HOME.to_string(),
                is_sc: true,
                metadata: empty_metadata.clone(),
            },
        );
        listing.insert(
            0,
            ItemInfo {
                name: SC_DIR_BACK.to_string(),
                is_sc: true,
                metadata: empty_metadata.clone(),
            },
        );
        listing.insert(
            0,
            ItemInfo {
                name: SC_DIR_UP.to_string(),
                is_sc: true,
                metadata: empty_metadata.clone(),
            },
        );
        listing.insert(
            0,
            ItemInfo {
                name: SC_EXP.to_string(),
                is_sc: true,
                metadata: empty_metadata.clone(),
            },
        );
        listing.insert(
            0,
            ItemInfo {
                name: SC_EXIT.to_string(),
                is_sc: true,
                metadata: empty_metadata.clone(),
            },
        );
        self.dir_listing = listing;
    }

    fn update_results(&mut self) {
        let matcher = SkimMatcherV2::default();
        let mut scored: Vec<_> = self
            .dir_listing
            .iter()
            .take(SEARCH_LIMIT) // Limit for performance
            .filter_map(|item| {
                matcher
                    .fuzzy_match(&item.name, &self.input)
                    .map(|score| (score, item.clone()))
            })
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0));
        self.results = scored.into_iter().map(|(_, item)| item).collect();
    }

    fn update_selection(&mut self) {
        if self.selection_index < self.results.len() as i32 {
            self.selection = self.results[self.selection_index as usize].name.clone();
        } else if !self.results.is_empty() {
            self.selection_index = 0;
            self.selection = String::new();
        } else {
            self.selection_index = 0;
            self.selection = String::new();
        }
        // Remove icon prefix from selection
        if let Some(pos) = self.selection.find("| ") {
            self.selection = self.selection[(pos + 2)..].to_string();
        }
    }

    fn update_preview(&mut self) {
        self.preview_content = Default::default();
        match self.selection.as_str() {
            SC_EXIT => {
                self.preview_content += App::fmtln_sc("Exit the application");
            }
            SC_HOME => {
                self.preview_content += App::fmtln_path(&dirs::home_dir().unwrap());
                self.preview_content += App::fmtln_sc("Go to the home directory");
            }
            SC_DIR_UP => {
                self.preview_content += App::fmtln_path(&self.cwd);
                self.preview_content += App::fmtln_sc("Go up to the parent directory");
            }
            SC_DIR_BACK => {
                self.preview_content += App::fmtln_path(&self.lwd);
                self.preview_content += App::fmtln_sc("Go back to the last working directory");
            }
            SC_EXP => {
                self.preview_content += App::fmtln_sc("Toggle explode mode");
                self.preview_content += Line::styled(
                    "Shows all files in subdirectories under the current directory.",
                    Style::default().fg(Color::Green),
                );
                let status = if self.mode_explode { "ON" } else { "OFF" };
                self.preview_content += App::fmtln_info("explode mode", status);
            }
            _ => {
                // TODO:
                // Metadata is not coming from item its being re-fetched here
                self.preview_content = Default::default();
                let mut selected_path = self.cwd.clone();
                selected_path.push(&self.selection);

                if selected_path.is_dir() {
                    self.preview_dir(&selected_path);
                } else if selected_path.is_file() {
                    self.preview_file(&selected_path);
                } else {
                    self.preview_content +=
                        Line::from("Selected item is neither file nor directory.");
                    let metadata = fs::metadata(&selected_path);
                    self.preview_content += Line::from(format!("{:?}", metadata))
                }
            }
        }
    }

    fn cmd_home(&mut self) {
        self.set_cwd(&dirs::home_dir().unwrap());
        self.update_directory_listing();
        self.update_results();
        self.selection_index = 0;
    }

    fn cmd_dir_up(&mut self) {
        self.selection = "..".to_string();
    }

    fn cmd_dir_back(&mut self) {
        self.selection = self.lwd.to_str().unwrap().to_string();
    }

    fn cmd_explode(&mut self) {
        self.mode_explode = !self.mode_explode;
        // TODO: Not sure why this needs to continue
        self.update_directory_listing();
        self.update_results();
        self.update_selection();
        self.update_preview();
        self.selection_index = 0;
    }

    fn cmd_sel_down(&mut self) {
        self.selection_index += 1;
        if self.selection_index >= self.results.len() as i32 {
            self.selection_index = 0;
        }
    }

    fn cmd_sel_up(&mut self) {
        self.selection_index += -1;
        if self.selection_index < 0 && !self.results.is_empty() {
            self.selection_index = self.results.len() as i32 - 1;
        } else if self.results.is_empty() {
            self.selection_index = 0;
        }
    }

    fn cmd_cmd_window_toggle(&mut self) {
        self.command_window_open = !self.command_window_open;
    }

    fn cmd_multi_sel(&mut self) {
        let mut selected_path = self.cwd.clone();
        selected_path.push(&self.selection);
        let is_sc = self
            .results
            .get(self.selection_index as usize)
            .map_or(false, |item| item.is_sc);
        if is_sc {
            return;
        }
        // Check if already in multi selection
        if let Some(pos) = self
            .multi_selection
            .iter()
            .position(|x| *x == selected_path)
        {
            self.multi_selection.remove(pos);
        } else {
            self.multi_selection.push(selected_path);
        }
    }

    fn cmd_multi_clear(&mut self) {
        self.multi_selection.clear();
    }

    fn handle_command(&mut self, cmd: &str) -> LoopReturn {
        match cmd {
            cmd_name::SELECT => {
                self.input = String::new();
                let selection = self.selection.clone();
                match selection.as_str() {
                    SC_EXIT => return LoopReturn::Break,
                    SC_HOME => {
                        self.cmd_home();
                        return LoopReturn::Continue;
                    }
                    SC_DIR_UP => {
                        self.cmd_dir_up();
                    }
                    SC_DIR_BACK => {
                        self.cmd_dir_back();
                    }
                    SC_EXP => {
                        self.cmd_explode();
                        return LoopReturn::Continue;
                    }
                    _ => {}
                }
                self.set_cwd(&self.selection.clone().into());
                self.update_directory_listing();
                self.update_results();
                self.selection_index = 0;
            }
            cmd_name::SEL_DOWN => self.cmd_sel_down(),
            cmd_name::SEL_UP => self.cmd_sel_up(),
            cmd_name::DIR_UP => self.cmd_dir_up(),
            cmd_name::DIR_BACK => self.cmd_dir_back(),
            cmd_name::EXPLODE => self.cmd_explode(),
            cmd_name::HOME => self.cmd_home(),
            cmd_name::CMD_TOGGLE => self.cmd_cmd_window_toggle(),
            cmd_name::MULTI_SEL => self.cmd_multi_sel(),
            cmd_name::MULTI_CLEAR => self.cmd_multi_clear(),
            cmd_name::EXIT => return LoopReturn::Break,
            _ => {
                dbg!("No command matched: {}", cmd);
            }
        }
        LoopReturn::Ok
    }
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> Result<()> {
    // Get directory listing
    app.set_cwd(&app.cwd.clone());
    app.update_directory_listing();
    app.update_results(); // Initial results
    app.update_selection();
    app.update_preview();
    loop {
        terminal.draw(|f| render(f, &app))?;
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(KeyEvent {
                code, modifiers, ..
            }) = event::read()?
            {
                // Command window input handling
                if app.command_window_open {
                    match (modifiers, code) {
                        (KeyModifiers::NONE, KeyCode::Char(c)) => {
                            app.command_input.push(c);
                        }
                        (KeyModifiers::NONE, KeyCode::Backspace) => {
                            app.command_input.pop();
                        }
                        (KeyModifiers::NONE, KeyCode::Enter) => {
                            // Handle commands
                            let cmd = app.command_input.clone();
                            let lr = app.handle_command(&cmd);
                            match lr {
                                LoopReturn::Continue => continue,
                                LoopReturn::Break => break,
                                LoopReturn::Ok => {}
                            }
                            app.command_input = String::new();
                        }
                        (KeyModifiers::NONE, KeyCode::Esc) => {
                            app.command_window_open = false;
                        }
                        _ => {}
                    }
                    continue;
                }
                // Before key press handling
                app.update_selection();
                let mut input_changed = false;
                // Some things are not bindable
                match (modifiers, code) {
                    (KeyModifiers::NONE, KeyCode::Char(c)) => {
                        app.input.push(c);
                        input_changed = true;
                    }
                    (KeyModifiers::NONE, KeyCode::Backspace) => {
                        app.input.pop();
                        input_changed = true;
                    }
                    (KeyModifiers::NONE, KeyCode::Esc) => break,
                    _ => {}
                }
                if input_changed {
                    app.update_results();
                }
                // Process key to command mapping
                let cmd = match (modifiers, code) {
                    (KeyModifiers::CONTROL, KeyCode::Char('t')) => cmd_name::CMD_TOGGLE,
                    (KeyModifiers::CONTROL, KeyCode::Char('s')) => cmd_name::MULTI_SEL,
                    (KeyModifiers::NONE, KeyCode::Enter) => cmd_name::SELECT,
                    (KeyModifiers::NONE, KeyCode::Right) => cmd_name::SELECT,
                    (KeyModifiers::NONE, KeyCode::Up) => cmd_name::SEL_UP,
                    (KeyModifiers::NONE, KeyCode::Down) => cmd_name::SEL_DOWN,
                    (KeyModifiers::NONE, KeyCode::Left) => cmd_name::DIR_BACK,
                    (KeyModifiers::CONTROL, KeyCode::Char('h')) => cmd_name::DIR_BACK,
                    (KeyModifiers::CONTROL, KeyCode::Char('j')) => cmd_name::SEL_DOWN,
                    (KeyModifiers::CONTROL, KeyCode::Char('k')) => cmd_name::SEL_UP,
                    (KeyModifiers::CONTROL, KeyCode::Char('l')) => cmd_name::SELECT,
                    _ => "",
                };
                dbg!(&cmd);
                // Handle commands
                let lr = app.handle_command(&cmd);
                match lr {
                    LoopReturn::Continue => continue,
                    LoopReturn::Break => break,
                    LoopReturn::Ok => {}
                }
                // After key press handling

                app.update_selection();
                app.update_preview();
            }
        }
    }

    Ok(())
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ]
            .as_ref(),
        )
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ]
            .as_ref(),
        )
        .split(popup_layout[1])[1]
}

fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Split horizontally: left (40%), right (60%)
    let horizontal_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)].as_ref())
        .split(area);

    // Split left chunk vertically: input (3), results (remaining)
    let left_vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .constraints([Constraint::Length(3), Constraint::Min(1)].as_ref())
        .split(horizontal_chunks[0]);

    // Input box
    let mut input_color;
    let input_str: String;
    if app.input.is_empty() {
        input_str = "Type to search...".to_string();
        input_color = Color::Gray;
    } else {
        input_str = app.input.clone();
        input_color = Color::White;
    };
    if app.results.is_empty() {
        input_color = Color::Red;
    }
    let input_span: Span = Span::styled(format!("{}", input_str), Style::default().fg(input_color));
    let suffix: Span = Span::styled(format!("|{} ", NF_MAG), Style::default().fg(Color::Green));
    let mut input_line = Line::from(input_span);
    input_line.push_span(suffix);
    let input = Paragraph::new(input_line).block(
        Block::default()
            .title(format!(" {}", app.cwd.to_str().unwrap()))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green)),
    );

    // List of results
    let mut results_pretty = app.dir_list_pretty(&app.results);
    for (idx, line) in results_pretty.lines.iter_mut().enumerate() {
        if idx as i32 == app.selection_index {
            let span = Span::styled(
                format!("{}", NF_SEL),
                Style::default().fg(Color::Blue).bg(Color::Black),
            );
            let mut new_line = Line::from(span);
            new_line.push_span(Span::raw(format!(" {}", line)));
            *line = new_line;
        }
    }
    let list_title = format!("(SONA)))[{}]", app.results.len());
    let list = List::new(results_pretty).block(
        Block::default()
            .title(list_title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue)),
    );

    // Create ListState and set selected index
    let mut state = ListState::default();
    if !app.results.is_empty() && app.selection_index >= 0 {
        state.select(Some(app.selection_index as usize));
    }

    // Preview box
    let preview = Paragraph::new(app.preview_content.clone())
        .block(
            Block::default()
                .title(format!("{} (0)_(0) {} ", NF_LOOK, app.selection))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .wrap(Wrap { trim: false });

    // Render widgets
    frame.render_widget(input, left_vertical_chunks[0]);
    frame.render_stateful_widget(list, left_vertical_chunks[1], &mut state);
    frame.render_widget(preview, horizontal_chunks[1]);

    // Render command popup if open
    if app.command_window_open {
        let popup_area = centered_rect(50, 10, area);
        let command_str = format!("> {}|", app.command_input);
        frame.render_widget(Clear, popup_area); // Clears the area first
        let command_paragraph = Paragraph::new(command_str)
            .style(Style::default().bg(Color::Black))
            .block(
                Block::default()
                    .title(format!("{} Command", NF_CMD))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Magenta))
                    .style(Style::default().bg(Color::Black)),
            );
        frame.render_widget(command_paragraph, popup_area);
    }
}

fn clear() {
    println!("\x1B[2J\x1B[1;1H");
}

fn main() -> Result<()> {
    color_eyre::install()?;
    clear();
    enable_raw_mode()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(std::io::stdout()))?;

    let app = App::new();

    run_app(&mut terminal, app)?;

    disable_raw_mode()?;
    clear();
    Ok(())
}
