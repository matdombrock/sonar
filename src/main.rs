use color_eyre::eyre::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use ratatui::{Frame, Terminal, layout::Constraint, prelude::Backend, widgets::Wrap};
use ratatui::{backend::CrosstermBackend, layout::Layout};
use ratatui::{
    layout::Direction,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
const NF_PREVIEW: &str = "󰍉";
const NF_SEL: &str = ""; //➤
const NF_DIR: &str = "";
const NF_FILE: &str = "";

// Shortcut strings
const SC_UP: &str = " .. up";
const SC_EXIT: &str = " exit";
const SC_HOME: &str = "~ home";
const SC_BACK: &str = " back";

struct App {
    input: String,
    items: Vec<String>,
    results: Vec<String>,
    selection: String,
    selection_index: i32,
    preview_content: String,
    cwd: PathBuf,
    lwd: PathBuf,
}

impl App {
    fn new() -> Self {
        Self {
            input: String::new(),
            items: Vec::new(),
            results: Vec::new(),
            selection: String::new(),
            selection_index: 0,
            preview_content: "...".to_string(),
            cwd: env::current_dir().unwrap(),
            lwd: env::current_dir().unwrap(),
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

    fn get_directory_listing(&mut self, path: &PathBuf) {
        let cwd = path;
        let mut entries = Vec::new();
        entries.push(format!("{} {}", "|", SC_EXIT.to_string()));
        entries.push(SC_UP.to_string());
        entries.push(SC_HOME.to_string());
        entries.push(SC_BACK.to_string());

        match fs::read_dir(cwd) {
            Ok(read_dir) => {
                for entry_result in read_dir {
                    if let Ok(entry) = entry_result {
                        let file_name = entry.file_name();
                        let file_name_str = file_name.to_string_lossy();
                        let mut display_name = file_name_str.to_string();
                        // Append '/' if it's a directory
                        if let Ok(metadata) = entry.metadata() {
                            if metadata.is_dir() {
                                display_name = format!("{}| {}/", NF_DIR, display_name);
                            } else if metadata.is_file() {
                                display_name = format!("{}| {}", NF_FILE, display_name);
                            }
                        }
                        entries.push(display_name);
                    }
                }
            }
            Err(_) => {
                // Handle or ignore directory read errors
            }
        }
        self.items = entries;
    }

    fn update_results(&mut self) {
        let matcher = SkimMatcherV2::default();
        let mut scored: Vec<_> = self
            .items
            .iter()
            .filter_map(|item| {
                matcher
                    .fuzzy_match(item, &self.input)
                    .map(|score| (score, item.clone()))
            })
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0));
        self.results = scored.into_iter().map(|(_, item)| item).collect();
    }

    fn update_selection(&mut self) {
        if self.selection_index < self.results.len() as i32 {
            self.selection = self.results[self.selection_index as usize].clone();
        } else if !self.results.is_empty() {
            self.selection_index = 0;
            self.selection = self.results[0].clone();
        } else {
            self.selection_index = 0;
            self.selection = String::new();
        }
        // Remove icon prefix from selection
        if let Some(pos) = self.selection.find("| ") {
            self.selection = self.selection[(pos + 2)..].to_string();
        }
    }

    fn update_preview_content(&mut self) {
        // Update preview content
        self.preview_content = String::new();
        match self.selection.as_str() {
            SC_EXIT => {
                self.preview_content = "Exit the application".to_string();
            }
            SC_HOME => {
                self.preview_content = "Go to home directory".to_string();
            }
            SC_UP => {
                self.preview_content = "Go up one directory".to_string();
            }
            SC_BACK => {
                self.preview_content = format!(
                    "Go back to last working directory:\n{}",
                    self.lwd.to_str().unwrap()
                );
            }
            _ => {
                self.preview_content = String::new();
                let mut selected_path = self.cwd.clone();
                selected_path.push(&self.selection);
                if selected_path.is_dir() {
                    self.preview_content = format!(
                        "Directory: {}\n\nContents:\n",
                        selected_path.to_str().unwrap()
                    );
                    match fs::read_dir(&selected_path) {
                        Ok(read_dir) => {
                            for entry_result in read_dir {
                                if let Ok(entry) = entry_result {
                                    let file_name = entry.file_name();
                                    let file_name_str = file_name.to_string_lossy();
                                    let mut display_name = file_name_str.to_string();
                                    // Append '/' if it's a directory
                                    if let Ok(metadata) = entry.metadata() {
                                        if metadata.is_dir() {
                                            display_name.push('/');
                                        }
                                    }
                                    self.preview_content
                                        .push_str(&format!("{}\n", display_name));
                                }
                            }
                        }
                        Err(e) => {
                            self.preview_content
                                .push_str(&format!("Error reading directory: {}", e));
                        }
                    }
                } else if selected_path.is_file() {
                    use std::process::Command;

                    let output = Command::new("bat")
                        // .arg("--color=always")
                        .arg("--style=plain")
                        .arg("--line-range=1:20")
                        .arg(selected_path.to_str().unwrap())
                        .output();

                    match output {
                        Ok(output) if output.status.success() => {
                            self.preview_content =
                                String::from_utf8_lossy(&output.stdout).to_string();
                        }
                        Ok(output) => {
                            let err_msg = String::from_utf8_lossy(&output.stderr);
                            self.preview_content = format!("bat failed: {}", err_msg);
                        }
                        Err(e) => {
                            self.preview_content = format!("Failed to run bat: {}", e);
                        }
                    }
                } else {
                    self.preview_content =
                        "Selected item is neither a file nor a directory.".to_string();
                }
            }
        }
    }
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> Result<()> {
    // Get directory listing
    app.set_cwd(&app.cwd.clone());
    app.get_directory_listing(&app.cwd.clone());
    app.update_results(); // Initial results
    app.update_selection();
    app.update_preview_content();
    loop {
        terminal.draw(|f| render(f, &app))?;
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(KeyEvent { code, .. }) = event::read()? {
                // Before key press handling
                app.update_selection();
                let mut input_changed = false;
                match code {
                    KeyCode::Char(c) => {
                        app.input.push(c);
                        input_changed = true;
                    }
                    KeyCode::Backspace => {
                        app.input.pop();
                        input_changed = true;
                    }
                    KeyCode::Enter => {
                        app.input = String::new();
                        let mut selection = app.selection.clone();
                        match selection.as_str() {
                            SC_EXIT => break,
                            SC_HOME => {
                                app.set_cwd(&dirs::home_dir().unwrap());
                                app.get_directory_listing(&app.cwd.clone());
                                app.update_results();
                                app.selection_index = 0;
                                continue;
                            }
                            SC_UP => {
                                selection = "..".to_string();
                            }
                            SC_BACK => {
                                selection = app.lwd.to_str().unwrap().to_string();
                            }
                            _ => {}
                        }
                        app.set_cwd(&selection.into());
                        app.get_directory_listing(&app.cwd.clone());
                        app.update_results();
                        app.selection_index = 0;
                    }
                    KeyCode::Down => {
                        app.selection_index += 1;
                        if app.selection_index >= app.results.len() as i32 {
                            app.selection_index = 0;
                        }
                    }
                    KeyCode::Up => {
                        app.selection_index += -1;
                        if app.selection_index < 0 {
                            app.selection_index = app.results.len() as i32 - 1;
                        }
                    }
                    KeyCode::Esc => break,
                    _ => {}
                }

                // After key press handling

                if input_changed {
                    app.update_results();
                }

                app.update_selection();
                app.update_preview_content();
            }
        }
    }

    Ok(())
}

// filepath: src/main.rs
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
    let input = Paragraph::new(app.input.as_str()).block(
        Block::default()
            .title(format!(" {}", app.cwd.to_str().unwrap()))
            .borders(Borders::ALL),
    );

    // List of results
    let list_items: Vec<ListItem> = app
        .results
        .iter()
        .enumerate()
        .map(|(idx, item)| {
            let display = if idx as i32 == app.selection_index {
                format!("{} {}", NF_SEL, item)
            } else {
                item.to_string()
            };
            ListItem::new(display)
        })
        .collect();

    let list =
        List::new(list_items).block(Block::default().title("(SONAR)))").borders(Borders::ALL));

    // Create ListState and set selected index
    let mut state = ListState::default();
    if !app.results.is_empty() && app.selection_index >= 0 {
        state.select(Some(app.selection_index as usize));
    }

    // Preview box
    let preview_content = app.preview_content.as_str();
    let preview = Paragraph::new(preview_content)
        .block(
            Block::default()
                .title(format!("{} (0)_(0) {} ", NF_PREVIEW, app.selection))
                .borders(Borders::ALL),
        )
        .wrap(Wrap { trim: false });

    // Render widgets
    frame.render_widget(input, left_vertical_chunks[0]);
    frame.render_stateful_widget(list, left_vertical_chunks[1], &mut state);
    frame.render_widget(preview, horizontal_chunks[1]);
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
