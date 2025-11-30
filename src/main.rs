use color_eyre::eyre::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use mime_guess::mime;
use ratatui::style::Color as RColor;
use ratatui::style::{Color, Style};
use ratatui::text::{Span, Text};
use ratatui::{
    Frame, Terminal,
    layout::{Constraint, Rect},
    prelude::Backend,
    text::Line,
    widgets::{Clear, Wrap},
};
use ratatui::{backend::CrosstermBackend, layout::Layout};
use ratatui::{
    // Might want ListItem
    layout::Direction,
    widgets::{Block, Borders, List, ListState, Paragraph},
};
use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style as SyntectStyle, ThemeSet};
use syntect::parsing::SyntaxSet;
// Might want Path
use std::path::PathBuf;

const APP_NAME: &str = "sona";

const LOGO: &str = r#"
 ██╗███████╗ ██████╗ ███╗   ██╗ █████╗ ██╗ ██╗ ██╗ 
██╔╝██╔════╝██╔═══██╗████╗  ██║██╔══██╗╚██╗╚██╗╚██╗
██║ ███████╗██║   ██║██╔██╗ ██║███████║ ██║ ██║ ██║
██║ ╚════██║██║   ██║██║╚██╗██║██╔══██║ ██║ ██║ ██║
╚██╗███████║╚██████╔╝██║ ╚████║██║  ██║██╔╝██╔╝██╔╝
 ╚═╝╚══════╝ ╚═════╝ ╚═╝  ╚═══╝╚═╝  ╚═╝╚═╝ ╚═╝ ╚═╝ 
"#;

// Limit for performance
const DIR_PRETTY_LIMIT: usize = 1000;
const SEARCH_LIMIT: usize = 1000;

// Nerd font icons
mod nf {
    pub const MAG: &str = "󰍉";
    pub const LOOK: &str = "";
    pub const SEL: &str = ""; //➤
    pub const DIR: &str = "";
    pub const DIRO: &str = "󰉒";
    pub const FILE: &str = "";
    pub const CMD: &str = "";
    pub const INFO: &str = "";
    pub const CHECK: &str = "";
}

// Shortcut strings
mod sc {
    pub const DIR_UP: &str = " .. up";
    pub const EXIT: &str = " exit";
    pub const HOME: &str = "~ home";
    pub const DIR_BACK: &str = " back";
    pub const MENU_BACK: &str = " menu";
    pub const EXP: &str = " explode";
    pub const CMDS: &str = " cmds";
    pub const MULTI_SHOW: &str = " show multi-selection";
    pub const MULTI_CLEAR: &str = " clear multi-selection";
    pub const MULTI_SAVE: &str = " save multi-selection";
    pub const LOG: &str = " show log";
    pub const LOG_CLEAR: &str = " clear log";
}

// Command names
mod cmd_name {
    pub const EXIT: &str = ":exit";
    pub const HOME: &str = ":home";
    pub const SEL_UP: &str = ":sel-up";
    pub const SEL_DOWN: &str = ":sel-down";
    pub const DIR_UP: &str = ":dir-up";
    pub const DIR_BACK: &str = ":dir-back";
    pub const EXPLODE: &str = ":explode";
    pub const SELECT: &str = ":select";
    pub const CMD_WIN_TOGGLE: &str = ":cmd";
    pub const CMD_VIS_TOGGLE: &str = ":cmd-vis-toggle";
    pub const CMD_VIS_SHOW: &str = ":cmd-vis-show";
    pub const OUTPUT_WIN_TOGGLE: &str = ":output-toggle";
    pub const MULTI_SEL: &str = ":multi-sel";
    pub const MULTI_CLEAR: &str = ":multi-clear";
    pub const MULTI_SHOW: &str = ":multi-show";
    pub const MULTI_SAVE: &str = ":multi-save";
    pub const MENU_BACK: &str = ":menu-back";
    pub const LOG: &str = ":log";
    pub const LOG_CLEAR: &str = ":log-clear";
    pub const SEC_UP: &str = ":sec-up";
    pub const SEC_DOWN: &str = ":sec-down";
}

// Logs to temp directory
mod log {
    use super::APP_NAME;
    pub fn log_path() -> std::path::PathBuf {
        std::env::temp_dir()
            .join(APP_NAME)
            .join(format!("{}.log", APP_NAME))
    }
    pub fn log_impl(msg: &str) {
        let log_path = log_path();
        let _ = std::fs::create_dir_all(log_path.parent().unwrap());
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .unwrap();
        use std::io::Write;
        let _ = writeln!(file, "{}", msg);
    }

    #[macro_export]
    macro_rules! log {
        ($($arg:tt)*) => {
            $crate::log::log_impl(&format!($($arg)*));
        };
    }
}

// Information about a file or directory
#[derive(Clone)]
struct ItemInfo {
    name: String,
    is_sc: bool,
    metadata: fs::Metadata,
}

// Return type for loop control
enum LoopReturn {
    Continue,
    Break,
    Ok,
}

// Main application state
struct App<'a> {
    input: String,
    dir_listing: Vec<ItemInfo>,
    results: Vec<ItemInfo>,
    selection: String,
    selection_index: i32,
    multi_selection: Vec<PathBuf>,
    preview_content: Text<'a>,
    scroll_off_preview: u16,
    scroll_off_output: u16,
    cwd: PathBuf,
    lwd: PathBuf,
    mode_explode: bool,
    mode_vis_commands: bool,
    command_window_open: bool,
    command_input: String,
    output_window_open: bool,
    output_text: String,
}
impl<'a> App<'a> {
    fn new() -> Self {
        log!("App initialized");
        Self {
            input: String::new(),
            dir_listing: Vec::new(),
            results: Vec::new(),
            selection: String::new(),
            selection_index: 0,
            multi_selection: Vec::new(),
            preview_content: Default::default(),
            scroll_off_preview: 0,
            scroll_off_output: 0,
            cwd: env::current_dir().unwrap(),
            lwd: env::current_dir().unwrap(),
            mode_explode: false,
            mode_vis_commands: false,
            command_window_open: false,
            command_input: String::new(),
            output_window_open: false,
            output_text: String::new(),
        }
    }

    fn set_cwd(&mut self, path: &PathBuf) {
        log!("Changing directory to: {}", path.to_str().unwrap());
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
        log!("Getting directory listing for: {}", path.to_str().unwrap());
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
                                // Handle metadata errors
                                log!("Failed to get metadata for entry: {}", file_name_str);
                                continue;
                            }
                        }
                    }
                }
            }
            Err(_) => {
                // Handle directory read errors
                log!("Failed to read directory: {}", path.to_str().unwrap());
            }
        }
        entries
    }

    fn fmtln_info(label: &str, value: &str) -> Line<'a> {
        Line::styled(
            format!("{} {}: {}", nf::INFO, label, value),
            Style::default().fg(Color::Yellow),
        )
    }
    fn fmtln_path(path: &PathBuf) -> Line<'a> {
        Line::styled(
            format!("{} {}", nf::DIRO, path.to_str().unwrap()),
            Style::default().fg(Color::Blue),
        )
    }
    fn fmtln_sc(description: &str) -> Line<'a> {
        Line::styled(
            format!("{} {}", nf::CMD, description),
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
            let ms_on = format!("{} ", nf::CHECK);
            if is_multi_selected {
                ms = &ms_on;
            }
            // Limit for performance
            let line = if item.is_sc {
                Line::styled(
                    format!("{}{} {}", ms, nf::CMD, item.name),
                    Style::default().fg(Color::Yellow),
                )
            } else if item.metadata.is_dir() {
                Line::styled(
                    format!("{}{} {}/", ms, nf::DIR, item.name),
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
                    format!("{}{} {}", ms, nf::FILE, name),
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
        // Helper to convert syntect color to ratatui Color
        fn syntect_to_ratatui_color(s: SyntectStyle) -> RColor {
            RColor::Rgb(s.foreground.r, s.foreground.g, s.foreground.b)
        }
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

        // Syntax highlighting
        let ss = SyntaxSet::load_defaults_newlines();
        let ts = ThemeSet::load_defaults();
        let ext = selected_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        let syntax = ss
            .find_syntax_by_extension(ext)
            .unwrap_or_else(|| ss.find_syntax_plain_text());
        let mut h = HighlightLines::new(syntax, &ts.themes["base16-ocean.dark"]);

        if let Ok(content) = fs::read_to_string(&selected_path) {
            for line in content.lines().take(100) {
                let ranges = h.highlight_line(line, &ss).unwrap_or_default();
                let mut styled_line = Line::default();
                for (style, text) in ranges {
                    styled_line.push_span(Span::styled(
                        text.to_string(),
                        Style::default().fg(syntect_to_ratatui_color(style)),
                    ));
                }
                self.preview_content += styled_line;
            }
        } else {
            self.preview_content += Line::from("Unable to read file content.");
        }
    }

    fn update_preview(&mut self) {
        self.preview_content = Default::default();
        match self.selection.as_str() {
            sc::EXIT => {
                self.preview_content += App::fmtln_sc("Exit the application");
                self.preview_content += Line::from("");
                for (i, line) in LOGO.lines().enumerate() {
                    if i == 0 {
                        continue;
                    };
                    self.preview_content +=
                        Line::styled(format!("{}", line), Style::default().fg(Color::LightGreen));
                }
            }
            sc::HOME => {
                self.preview_content += App::fmtln_path(&dirs::home_dir().unwrap());
                self.preview_content += App::fmtln_sc("Go to the home directory");
            }
            sc::DIR_UP => {
                self.preview_content += App::fmtln_path(&self.cwd);
                self.preview_content += App::fmtln_sc("Go up to the parent directory");
            }
            sc::DIR_BACK => {
                self.preview_content += App::fmtln_path(&self.lwd);
                self.preview_content += App::fmtln_sc("Go back to the last working directory");
            }
            sc::EXP => {
                self.preview_content += App::fmtln_sc("Toggle explode mode");
                self.preview_content += Line::styled(
                    "Shows all files in subdirectories under the current directory.",
                    Style::default().fg(Color::Green),
                );
                let status = if self.mode_explode { "ON" } else { "OFF" };
                self.preview_content += App::fmtln_info("explode mode", status);
            }
            sc::CMDS => {
                self.preview_content += App::fmtln_sc("Show visual commands");
                self.preview_content += Line::styled(
                    "Toggles a visual command menu in the listing.",
                    Style::default().fg(Color::Green),
                );
            }
            sc::MENU_BACK => {
                self.preview_content += App::fmtln_sc("Go back to the previous menu");
                self.preview_content += Line::styled(
                    "Exits the current visual command menu.",
                    Style::default().fg(Color::Green),
                );
            }
            sc::MULTI_SHOW => {
                self.preview_content += App::fmtln_sc("Show multi-selection");
                self.preview_content += Line::styled(
                    "Displays all currently selected items in the output window.",
                    Style::default().fg(Color::Green),
                );
            }
            sc::MULTI_CLEAR => {
                self.preview_content += App::fmtln_sc("Clear multi-selection");
                self.preview_content += Line::styled(
                    "Clears all items from the multi-selection list.",
                    Style::default().fg(Color::Green),
                );
            }
            sc::MULTI_SAVE => {
                self.preview_content += App::fmtln_sc("Save multi-selection");
                self.preview_content += Line::styled(
                    "Saves the multi-selection to a file. (Not implemented yet)",
                    Style::default().fg(Color::Green),
                );
            }
            sc::LOG => {
                self.preview_content += App::fmtln_sc("Show application log");
                self.preview_content += Line::styled(
                    "Displays the application log in the output window.",
                    Style::default().fg(Color::Green),
                );
            }
            sc::LOG_CLEAR => {
                self.preview_content += App::fmtln_sc("Clear application log");
                self.preview_content += Line::styled(
                    "Clear the application log file.",
                    Style::default().fg(Color::Green),
                );
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
                    // Check if we have an image
                    let mime = mime_guess::from_path(&selected_path).first_or_octet_stream();
                    if mime.type_() == mime::IMAGE {
                        self.preview_content += Line::from("Image file preview not yet supported.");
                        return;
                    }
                    // Preview a text file
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

    fn update_listing(&mut self) {
        log!(
            "Updating directory listing for cwd: {}",
            self.cwd.to_str().unwrap()
        );
        let empty_metadata = match fs::metadata(&self.cwd) {
            Ok(meta) => meta,
            Err(_) => fs::metadata(".").unwrap(),
        };
        // Handle visual commands
        if self.mode_vis_commands {
            self.dir_listing.clear();
            self.dir_listing.push(ItemInfo {
                name: sc::MENU_BACK.to_string(),
                is_sc: true,
                metadata: empty_metadata.clone(),
            });
            self.dir_listing.push(ItemInfo {
                name: sc::EXIT.to_string(),
                is_sc: true,
                metadata: empty_metadata.clone(),
            });
            self.dir_listing.push(ItemInfo {
                name: sc::HOME.to_string(),
                is_sc: true,
                metadata: empty_metadata.clone(),
            });
            self.dir_listing.push(ItemInfo {
                name: sc::DIR_UP.to_string(),
                is_sc: true,
                metadata: empty_metadata.clone(),
            });
            self.dir_listing.push(ItemInfo {
                name: sc::EXP.to_string(),
                is_sc: true,
                metadata: empty_metadata.clone(),
            });
            self.dir_listing.push(ItemInfo {
                name: sc::MULTI_SHOW.to_string(),
                is_sc: true,
                metadata: empty_metadata.clone(),
            });
            self.dir_listing.push(ItemInfo {
                name: sc::MULTI_CLEAR.to_string(),
                is_sc: true,
                metadata: empty_metadata.clone(),
            });
            self.dir_listing.push(ItemInfo {
                name: sc::MULTI_SAVE.to_string(),
                is_sc: true,
                metadata: empty_metadata.clone(),
            });
            self.dir_listing.push(ItemInfo {
                name: sc::LOG.to_string(),
                is_sc: true,
                metadata: empty_metadata.clone(),
            });
            self.dir_listing.push(ItemInfo {
                name: sc::LOG_CLEAR.to_string(),
                is_sc: true,
                metadata: empty_metadata.clone(),
            });
            return;
        }
        let mut listing = self.get_directory_listing(&self.cwd.clone());
        listing.insert(
            0,
            ItemInfo {
                name: sc::CMDS.to_string(),
                is_sc: true,
                metadata: empty_metadata.clone(),
            },
        );
        listing.insert(
            0,
            ItemInfo {
                name: sc::DIR_BACK.to_string(),
                is_sc: true,
                metadata: empty_metadata.clone(),
            },
        );
        listing.insert(
            0,
            ItemInfo {
                name: sc::DIR_UP.to_string(),
                is_sc: true,
                metadata: empty_metadata.clone(),
            },
        );
        listing.insert(
            0,
            ItemInfo {
                name: sc::EXP.to_string(),
                is_sc: true,
                metadata: empty_metadata.clone(),
            },
        );
        listing.insert(
            0,
            ItemInfo {
                name: sc::EXIT.to_string(),
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

    fn input_out_window(&mut self, modifiers: KeyModifiers, code: KeyCode) {
        match (modifiers, code) {
            (KeyModifiers::NONE, KeyCode::Esc) => {
                self.output_window_open = false;
                return;
            }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                self.output_window_open = false;
                return;
            }
            _ => {}
        }
        // Special command matching just for output window
        let cmd = match (modifiers, code) {
            (KeyModifiers::ALT, KeyCode::Char('j')) => cmd_name::SEC_DOWN,
            (KeyModifiers::ALT, KeyCode::Char('k')) => cmd_name::SEC_UP,
            _ => "",
        };
        self.handle_cmd(&cmd);
    }

    fn input_cmd_window(&mut self, modifiers: KeyModifiers, code: KeyCode) -> LoopReturn {
        match (modifiers, code) {
            (KeyModifiers::NONE, KeyCode::Char(c)) => {
                self.command_input.push(c);
            }
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                self.command_input.pop();
            }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                // Handle commands
                let cmd = self.command_input.clone();
                log!("cmd: {}", &cmd);
                self.command_input = String::new();
                return self.handle_cmd(&cmd);
            }
            (KeyModifiers::NONE, KeyCode::Esc) => {
                self.command_window_open = false;
            }
            _ => {}
        }
        LoopReturn::Ok
    }

    // Returns true if input changed
    fn input_main(&mut self, modifiers: KeyModifiers, code: KeyCode) -> bool {
        match (modifiers, code) {
            (KeyModifiers::NONE, KeyCode::Char(c)) => {
                self.input.push(c);
                return true;
            }
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                self.input.pop();
                return true;
            }
            _ => {}
        }
        return false;
    }

    fn input_cmd_map(&mut self, modifiers: KeyModifiers, code: KeyCode) -> String {
        let cmd = match (modifiers, code) {
            (KeyModifiers::CONTROL, KeyCode::Char('t')) => cmd_name::CMD_WIN_TOGGLE,
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
            (KeyModifiers::NONE, KeyCode::Esc) => cmd_name::EXIT,
            (KeyModifiers::CONTROL, KeyCode::Char('q')) => cmd_name::EXIT,
            (KeyModifiers::ALT, KeyCode::Char('j')) => cmd_name::SEC_DOWN,
            (KeyModifiers::ALT, KeyCode::Char('k')) => cmd_name::SEC_UP,
            _ => "",
        };
        if !cmd.is_empty() {
            log!("cmd from mapping: {}", &cmd);
        }
        cmd.to_string()
    }

    fn cmd_home(&mut self) {
        self.set_cwd(&dirs::home_dir().unwrap());
        self.update_listing();
        self.update_results();
        self.selection_index = 0;
    }

    fn cmd_dir_up(&mut self) {
        self.selection = "..".to_string();
        self.set_cwd(&self.selection.clone().into());
        self.update_listing();
        self.update_results();
        self.selection_index = 0;
    }

    fn cmd_dir_back(&mut self) {
        self.selection = self.lwd.to_str().unwrap().to_string();
        self.set_cwd(&self.selection.clone().into());
        self.update_listing();
        self.update_results();
        self.selection_index = 0;
    }

    fn cmd_explode(&mut self) {
        self.mode_explode = !self.mode_explode;
        self.update_listing();
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

    fn cmd_output_window_toggle(&mut self) {
        self.output_window_open = !self.output_window_open;
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
        self.output_text = "Multi selection cleared.".to_string();
        self.output_window_open = true;
    }

    fn cmd_multi_show(&mut self) {
        self.output_text = String::new();
        if self.multi_selection.is_empty() {
            self.output_text = "No items in multi selection.".to_string();
            self.output_window_open = true;
            return;
        }
        for path in self.multi_selection.iter() {
            self.output_text += &format!("{}\n", path.to_str().unwrap());
        }
        self.output_window_open = true;
    }

    // Write multi selection to a file
    fn cmd_multi_save(&mut self) {
        let tmp = env::temp_dir();
        let file = tmp.join(APP_NAME).join("multi.txt");
        fs::write(
            &file,
            self.multi_selection
                .iter()
                .map(|p| p.to_str().unwrap())
                .collect::<Vec<&str>>()
                .join("\n"),
        )
        .unwrap_or(());
        self.output_text = format!(
            "Multi selection saved to {} ({} items).",
            file.to_str().unwrap(),
            self.multi_selection.len()
        );
        self.output_window_open = true;
    }

    fn cmd_cmd_vis_toggle(&mut self) {
        self.mode_vis_commands = !self.mode_vis_commands;
        self.update_listing();
        self.update_results();
        self.selection_index = 0;
    }

    fn cmd_vis_show(&mut self) {
        self.mode_vis_commands = true;
        self.update_listing();
        self.update_results();
        self.selection_index = 0;
    }

    fn cmd_menu_back(&mut self) {
        self.mode_vis_commands = false;
        self.update_listing();
        self.update_results();
        self.selection_index = 0;
    }

    fn cmd_log_show(&mut self) {
        let log_path = log::log_path();
        match fs::read_to_string(&log_path) {
            Ok(content) => {
                // Reverse the log content to show latest entries first
                let mut lines: Vec<&str> = content.lines().collect();
                lines.reverse();
                let content = lines.join("\n");
                self.output_text = content;
            }
            Err(_) => {
                self.output_text = "No log file found.".to_string();
            }
        }
        self.output_window_open = true;
    }

    fn cmd_sec_up(&mut self) {
        if self.output_window_open {
            if self.scroll_off_output >= 5 {
                self.scroll_off_output -= 5;
            } else {
                self.scroll_off_output = 0;
            }
            log!("Output scroll offset: {}", self.scroll_off_output);
            return;
        }
        if self.scroll_off_preview >= 5 {
            self.scroll_off_preview -= 5;
        } else {
            self.scroll_off_preview = 0;
        }
        log!("Scroll offset: {}", self.scroll_off_preview);
    }
    fn cmd_sec_down(&mut self) {
        if self.output_window_open {
            self.scroll_off_output += 5;
            log!("Output scroll offset: {}", self.scroll_off_output);
            return;
        }
        self.scroll_off_preview += 5;
        log!("Scroll offset: {}", self.scroll_off_preview);
    }

    fn cmd_log_clear(&mut self) {
        let log_path = log::log_path();
        match fs::remove_file(&log_path) {
            Ok(_) => {
                self.output_text = "Log file cleared.".to_string();
            }
            Err(_) => {
                self.output_text = "No log file found to clear.".to_string();
            }
        }
        self.output_window_open = true;
    }

    fn handle_cmd(&mut self, cmd: &str) -> LoopReturn {
        match cmd {
            cmd_name::SELECT => {
                // Update input to empty to reset search
                self.input = String::new();
                self.update_results();
                // Get selection
                let selection = self.selection.clone();
                match selection.as_str() {
                    sc::EXIT => return LoopReturn::Break,
                    sc::HOME => {
                        self.cmd_home();
                        return LoopReturn::Continue;
                    }
                    sc::DIR_UP => {
                        self.cmd_dir_up();
                    }
                    sc::DIR_BACK => {
                        self.cmd_dir_back();
                    }
                    sc::EXP => {
                        self.cmd_explode();
                        return LoopReturn::Continue;
                    }
                    sc::CMDS => {
                        self.cmd_vis_show();
                        return LoopReturn::Continue;
                    }
                    sc::MENU_BACK => {
                        self.cmd_menu_back();
                        return LoopReturn::Continue;
                    }
                    sc::MULTI_SHOW => {
                        self.cmd_multi_show();
                    }
                    sc::MULTI_CLEAR => {
                        self.cmd_multi_clear();
                    }
                    sc::MULTI_SAVE => {
                        self.cmd_multi_save();
                    }
                    sc::LOG => {
                        self.cmd_log_show();
                    }
                    sc::LOG_CLEAR => {
                        self.cmd_log_clear();
                    }
                    _ => {
                        self.set_cwd(&self.selection.clone().into());
                        self.update_listing();
                        self.update_results();
                        self.selection_index = 0;
                    }
                }
            }
            cmd_name::SEL_DOWN => self.cmd_sel_down(),
            cmd_name::SEL_UP => self.cmd_sel_up(),
            cmd_name::DIR_UP => self.cmd_dir_up(),
            cmd_name::DIR_BACK => self.cmd_dir_back(),
            cmd_name::EXPLODE => self.cmd_explode(),
            cmd_name::HOME => self.cmd_home(),
            cmd_name::CMD_WIN_TOGGLE => self.cmd_cmd_window_toggle(),
            cmd_name::OUTPUT_WIN_TOGGLE => self.cmd_output_window_toggle(),
            cmd_name::MULTI_SEL => self.cmd_multi_sel(),
            cmd_name::MULTI_CLEAR => self.cmd_multi_clear(),
            cmd_name::MULTI_SHOW => self.cmd_multi_show(),
            cmd_name::MULTI_SAVE => self.cmd_multi_save(),
            cmd_name::CMD_VIS_TOGGLE => self.cmd_cmd_vis_toggle(),
            cmd_name::CMD_VIS_SHOW => self.cmd_vis_show(),
            cmd_name::MENU_BACK => self.cmd_menu_back(),
            cmd_name::LOG => self.cmd_log_show(),
            cmd_name::LOG_CLEAR => self.cmd_log_clear(),
            cmd_name::SEC_DOWN => self.cmd_sec_down(),
            cmd_name::SEC_UP => self.cmd_sec_up(),
            cmd_name::EXIT => return LoopReturn::Break,
            _ => {
                if !cmd.is_empty() {
                    log!("No command matched: {}", cmd);
                }
            }
        }
        LoopReturn::Ok
    }
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> Result<()> {
    log!("Starting main event loop");
    // Get directory listing
    app.set_cwd(&app.cwd.clone());
    app.update_listing();
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
                // Output window input handling
                if app.output_window_open {
                    app.input_out_window(modifiers, code);
                    continue;
                }
                // Command window input handling
                if app.command_window_open {
                    let lr = app.input_cmd_window(modifiers, code);
                    match lr {
                        LoopReturn::Continue => continue,
                        LoopReturn::Break => break,
                        LoopReturn::Ok => {}
                    }
                    continue;
                }
                // Before key press handling
                app.update_selection();
                let input_changed = app.input_main(modifiers, code);
                // Some things are not bindable
                if input_changed {
                    app.update_results();
                }
                // Process key to command mapping
                let cmd = app.input_cmd_map(modifiers, code);
                // Handle commands
                let lr = app.handle_cmd(&cmd);
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
    let suffix: Span = Span::styled(format!("|{} ", nf::MAG), Style::default().fg(Color::Green));
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
                format!("{}", nf::SEL),
                Style::default().fg(Color::Blue).bg(Color::Black),
            );
            let mut new_line = Line::from(span);
            new_line.push_span(Span::raw(format!(" {}", line)));
            *line = new_line;
        }
    }
    let list_title = format!("({})))[{}]", APP_NAME.to_uppercase(), app.results.len());
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
                .title(format!("{} (0)_(0) {} ", nf::LOOK, app.selection))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow))
                .style(Style::default().bg(Color::Black)),
        )
        .wrap(Wrap { trim: false })
        .scroll((app.scroll_off_preview as u16, app.scroll_off_preview as u16));

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
                    .title(format!("{} Command", nf::CMD))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Magenta))
                    .style(Style::default().bg(Color::Black)),
            );
        frame.render_widget(command_paragraph, popup_area);
    }

    if app.output_window_open {
        let popup_area = centered_rect(50, 90, area);
        frame.render_widget(Clear, popup_area); // Clears the area first
        let command_paragraph = Paragraph::new(app.output_text.clone())
            .style(Style::default().bg(Color::Black))
            .block(
                Block::default()
                    .title(format!("{} Output ('esc' to exit)", nf::CMD))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Magenta))
                    .style(Style::default().bg(Color::Black)),
            )
            .scroll((app.scroll_off_output as u16, app.scroll_off_output as u16));
        frame.render_widget(command_paragraph, popup_area);
    }
}

fn clear() {
    println!("\x1B[2J\x1B[1;1H");
}

fn main() -> Result<()> {
    log!("======= Starting application =======");
    color_eyre::install()?;
    clear();
    enable_raw_mode()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(std::io::stdout()))?;

    let app = App::new();

    run_app(&mut terminal, app)?;

    disable_raw_mode()?;
    clear();
    println!("{} exited successfully.", APP_NAME);
    Ok(())
}
