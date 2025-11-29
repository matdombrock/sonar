use color_eyre::eyre::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use ratatui::{Frame, Terminal, layout::Constraint, prelude::Backend, text::Line, widgets::Wrap};
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

const NF_PREVIEW: &str = "󰍉";
const NF_SEL: &str = ""; //➤
const NF_DIR: &str = "";
const NF_DIRO: &str = "󰉒";
const NF_FILE: &str = "";
const NF_CMD: &str = "";
const NF_INFO: &str = "";

// Shortcut strings
const SC_UP: &str = " .. up";
const SC_EXIT: &str = " exit";
const SC_HOME: &str = "~ home";
const SC_BACK: &str = " back";

#[derive(Clone)]
struct ItemInfo {
    name: String,
    is_sc: bool,
    metadata: fs::Metadata,
}

struct App<'a> {
    input: String,
    dir_listing: Vec<ItemInfo>,
    results: Vec<ItemInfo>,
    selection: String,
    selection_index: i32,
    preview_content: Text<'a>,
    cwd: PathBuf,
    lwd: PathBuf,
}

impl<'a> App<'a> {
    fn new() -> Self {
        Self {
            input: String::new(),
            dir_listing: Vec::new(),
            results: Vec::new(),
            selection: String::new(),
            selection_index: 0,
            preview_content: Default::default(),
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

    fn get_directory_listing(&self, path: &PathBuf) -> Vec<ItemInfo> {
        let cwd = path;
        let mut entries = Vec::new();

        match fs::read_dir(cwd) {
            Ok(read_dir) => {
                for entry_result in read_dir {
                    if let Ok(entry) = entry_result {
                        let file_name = entry.file_name();
                        let file_name_str = file_name.to_string_lossy();
                        if let Ok(_metadata) = entry.metadata() {
                            entries.push(ItemInfo {
                                name: file_name_str.to_string(),
                                is_sc: false,
                                metadata: _metadata,
                            });
                        }
                    }
                }
            }
            Err(_) => {
                // Handle or ignore directory read errors
            }
        }
        entries
    }

    fn dir_list_pretty(list: &Vec<ItemInfo>) -> Text<'a> {
        let mut text = Text::default();
        for item in list {
            let line = if item.is_sc {
                Line::styled(
                    format!("{} {}", NF_CMD, item.name),
                    Style::default().fg(Color::Yellow),
                )
            } else if item.metadata.is_dir() {
                Line::styled(
                    format!("{} {}", NF_DIR, item.name),
                    Style::default().fg(Color::Green),
                )
            } else {
                Line::styled(
                    format!("{} {}", NF_FILE, item.name),
                    Style::default().fg(Color::Cyan),
                )
            };
            text.lines.push(Line::from(line));
        }
        text
    }

    fn update_directory_listing(&mut self) {
        let mut listing = self.get_directory_listing(&self.cwd.clone());
        listing.insert(
            0,
            ItemInfo {
                name: SC_HOME.to_string(),
                is_sc: true,
                metadata: fs::metadata(&self.cwd).unwrap(),
            },
        );
        listing.insert(
            0,
            ItemInfo {
                name: SC_BACK.to_string(),
                is_sc: true,
                metadata: fs::metadata(&self.cwd).unwrap(),
            },
        );
        listing.insert(
            0,
            ItemInfo {
                name: SC_UP.to_string(),
                is_sc: true,
                metadata: fs::metadata(&self.cwd).unwrap(),
            },
        );
        listing.insert(
            0,
            ItemInfo {
                name: SC_EXIT.to_string(),
                is_sc: true,
                metadata: fs::metadata(&self.cwd).unwrap(),
            },
        );
        self.dir_listing = listing;
    }

    fn update_results(&mut self) {
        let matcher = SkimMatcherV2::default();
        let mut scored: Vec<_> = self
            .dir_listing
            .iter()
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
        // Update preview content
        self.preview_content = Default::default();
        match self.selection.as_str() {
            SC_EXIT => {
                self.preview_content +=
                    Line::styled("Exit the application", Style::default().fg(Color::Green));
            }
            SC_HOME => {
                self.preview_content += Line::styled(
                    "Move to your home directory",
                    Style::default().fg(Color::Green),
                );
                self.preview_content += Line::from("");
                self.preview_content +=
                    Line::styled("Home Directory:", Style::default().fg(Color::Yellow));
                self.preview_content +=
                    Line::from(format!("{}", dirs::home_dir().unwrap().to_str().unwrap()));
            }
            SC_UP => {
                self.preview_content += Line::styled(
                    "Move up to the parent directory",
                    Style::default().fg(Color::Green),
                );
                self.preview_content += Line::from("");
                self.preview_content +=
                    Line::styled("Parent Directory:", Style::default().fg(Color::Yellow));
                self.preview_content += Line::from(format!(
                    "{}",
                    self.cwd.parent().unwrap_or(&self.cwd).to_str().unwrap()
                ));
            }
            SC_BACK => {
                self.preview_content += Line::styled(
                    "Return to the last working directory",
                    Style::default().fg(Color::Green),
                );
                self.preview_content += Line::from("");
                self.preview_content += Line::styled(
                    "Last Working Directory:",
                    Style::default().fg(Color::Yellow),
                );
                self.preview_content += Line::from(format!("{}", self.lwd.to_str().unwrap()));
            }
            _ => {
                fn info_line<'a>(label: &str, value: &str) -> Line<'a> {
                    Line::styled(
                        format!("{} {}: {}\n", NF_INFO, label, value),
                        Style::default().fg(Color::Yellow),
                    )
                }
                self.preview_content = Default::default();
                let mut selected_path = self.cwd.clone();
                selected_path.push(&self.selection);

                if selected_path.is_dir() {
                    let path_line = Line::styled(
                        format!("{} {}\n", NF_DIRO, selected_path.to_str().unwrap()),
                        Style::default().fg(Color::Blue),
                    );
                    self.preview_content += path_line;
                    let listing = self.get_directory_listing(&selected_path);
                    let count_line = info_line("count", &listing.len().to_string());
                    self.preview_content += count_line;
                    // Get the file metadata
                    let metadata = fs::metadata(&selected_path);
                    if let Ok(meta) = metadata {
                        // Get permissions
                        let permissions = meta.permissions();
                        let perm_line =
                            info_line("permissions", &format!("{:o}", permissions.mode()));
                        self.preview_content += perm_line;
                    }
                    self.preview_content += Line::from("-------");
                    let pretty_listing = App::dir_list_pretty(&listing);
                    for line in pretty_listing.lines.iter().take(20) {
                        self.preview_content += Line::from(line.clone());
                    }
                } else if selected_path.is_file() {
                    let path_line = Line::styled(
                        format!("{} {}\n", NF_DIRO, selected_path.to_str().unwrap()),
                        Style::default().fg(Color::Blue),
                    );
                    self.preview_content += path_line;
                    // Get the file metadata
                    let metadata = fs::metadata(&selected_path);
                    if let Ok(meta) = metadata {
                        // Get permissions
                        let permissions = meta.permissions();
                        let perm_line =
                            info_line("permissions", &format!("{:o}", permissions.mode()));
                        self.preview_content += perm_line;
                        // Get mime type
                        if meta.file_type().is_file() {
                            // Get mimetype using mime_guess
                            let mime =
                                mime_guess::from_path(&selected_path).first_or_octet_stream();
                            let mime_line = info_line("mime", &mime.to_string());
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
                } else {
                    self.preview_content +=
                        Line::from("Selected item is neither file nor directory.");
                }
            }
        }
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
                                app.update_directory_listing();
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
                        app.update_directory_listing();
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
                        if app.selection_index < 0 && !app.results.is_empty() {
                            app.selection_index = app.results.len() as i32 - 1;
                        } else if app.results.is_empty() {
                            app.selection_index = 0;
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
                app.update_preview();
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
    let input = Paragraph::new(format!("{}|", app.input.as_str())).block(
        Block::default()
            .title(format!(" {}", app.cwd.to_str().unwrap()))
            .borders(Borders::ALL),
    );

    // List of results
    let mut results_pretty = App::dir_list_pretty(&app.results);
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
    // let list_items: Vec<ListItem> = results_pretty
    //     .iter()
    //     .enumerate()
    //     .map(|(idx, item)| {
    //         let display = if idx as i32 == app.selection_index {
    //             format!("{} {}", NF_SEL, item.name)
    //         } else {
    //             item.name.to_string()
    //         };
    //         ListItem::new(display)
    //     })
    //     .collect();

    let list =
        List::new(results_pretty).block(Block::default().title("(SONAR)))").borders(Borders::ALL));

    // Create ListState and set selected index
    let mut state = ListState::default();
    if !app.results.is_empty() && app.selection_index >= 0 {
        state.select(Some(app.selection_index as usize));
    }

    // Preview box
    let preview = Paragraph::new(app.preview_content.clone())
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
