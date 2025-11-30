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
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::{env, process::Command};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style as SyntectStyle, ThemeSet};
use syntect::parsing::SyntaxSet;
// Might want Path
use std::path::PathBuf;

use crate::cmd_list::CmdName;

const APP_NAME: &str = "sona";

const LOGO: &str = r#"
 ██╗███████╗ ██████╗ ███╗   ██╗ █████╗ ██╗ ██╗ ██╗ 
██╔╝██╔════╝██╔═══██╗████╗  ██║██╔══██╗╚██╗╚██╗╚██╗
██║ ███████╗██║   ██║██╔██╗ ██║███████║ ██║ ██║ ██║
██║ ╚════██║██║   ██║██║╚██╗██║██╔══██║ ██║ ██║ ██║
╚██╗███████║╚██████╔╝██║ ╚████║██║  ██║██╔╝██╔╝██╔╝
 ╚═╝╚══════╝ ╚═════╝ ╚═╝  ╚═══╝╚═╝  ╚═╝╚═╝ ╚═╝ ╚═╝ 
"#;

const WELCOME_MSG: &str = r#"
Welcome to Sona!
Type to search files and directories. Use arrow keys to navigate.
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
    pub const MULTI_COPY: &str = " copy multi-selection";
    pub const LOG: &str = " show log";
    pub const LOG_CLEAR: &str = " clear log";
    pub const CMDS_LIST: &str = " command list";
}

mod cmd_list {
    use std::collections::HashMap;

    #[derive(Hash, Eq, PartialEq, Debug, Clone)]
    pub enum CmdName {
        Exit,
        Home,
        SelUp,
        SelDown,
        DirUp,
        DirBack,
        Explode,
        Select,
        CmdWinToggle,
        CmdVisToggle,
        CmdVisShow,
        CmdFinder,
        CmdList,
        OutputWinToggle,
        OutputWinShow,
        OutputWinHide,
        MultiSel,
        MultiClear,
        MultiShow,
        MultiSave,
        MultiCopy,
        MenuBack,
        Log,
        LogClear,
        SecUp,
        SecDown,
    }

    #[derive(Debug, Clone)]
    pub struct CmdData {
        pub fname: &'static str,
        pub description: &'static str,
        pub cmd: &'static str,
    }
    pub type CmdList = HashMap<CmdName, CmdData>;
    pub fn make_cmd_list() -> CmdList {
        let mut map = HashMap::new();
        map.insert(
            CmdName::Exit,
            CmdData {
                fname: "cmd_exit",
                description: "Exit the application",
                cmd: "exit",
            },
        );
        map.insert(
            CmdName::Home,
            CmdData {
                fname: "cmd_home",
                description: "Go to the home directory",
                cmd: "home",
            },
        );
        map.insert(
            CmdName::SelUp,
            CmdData {
                fname: "cmd_sel_up",
                description: "Move selection up",
                cmd: "sel-up",
            },
        );
        map.insert(
            CmdName::SelDown,
            CmdData {
                fname: "cmd_sel_down",
                description: "Move selection down",
                cmd: "sel-down",
            },
        );
        map.insert(
            CmdName::DirUp,
            CmdData {
                fname: "cmd_dir_up",
                description: "Go up to the parent directory",
                cmd: "dir-up",
            },
        );
        map.insert(
            CmdName::DirBack,
            CmdData {
                fname: "cmd_dir_back",
                description: "Go back to the last working directory",
                cmd: "dir-back",
            },
        );
        map.insert(
            CmdName::Explode,
            CmdData {
                fname: "cmd_explode",
                description: "Toggle explode mode",
                cmd: "explode",
            },
        );
        map.insert(
            CmdName::Select,
            CmdData {
                fname: "cmd_select",
                description: "Select the current item",
                cmd: "select",
            },
        );
        map.insert(
            CmdName::CmdWinToggle,
            CmdData {
                fname: "cmd_cmd_window_toggle",
                description: "Toggle command window",
                cmd: "cmd",
            },
        );
        map.insert(
            CmdName::CmdVisToggle,
            CmdData {
                fname: "cmd_cmd_vis_toggle",
                description: "Toggle visual commands",
                cmd: "cmd-vis-toggle",
            },
        );
        map.insert(
            CmdName::CmdVisShow,
            CmdData {
                fname: "cmd_cmd_vis_show",
                description: "Show visual commands",
                cmd: "cmd-vis-show",
            },
        );
        map.insert(
            CmdName::CmdFinder,
            CmdData {
                fname: "cmd_cmd_finder",
                description: "Toggle command finder",
                cmd: "cmd-finder",
            },
        );
        map.insert(
            CmdName::CmdList,
            CmdData {
                fname: "cmd_cmd_list",
                description: "List all commands",
                cmd: "cmd-list",
            },
        );
        map.insert(
            CmdName::OutputWinToggle,
            CmdData {
                fname: "cmd_output_window_toggle",
                description: "Toggle output window",
                cmd: "output-toggle",
            },
        );
        map.insert(
            CmdName::OutputWinShow,
            CmdData {
                fname: "cmd_output_window_show",
                description: "Show output window",
                cmd: "output-show",
            },
        );
        map.insert(
            CmdName::OutputWinHide,
            CmdData {
                fname: "cmd_output_window_hide",
                description: "Hide output window",
                cmd: "output-hide",
            },
        );
        map.insert(
            CmdName::MultiSel,
            CmdData {
                fname: "cmd_multi_sel",
                description: "Toggle multi-selection for current item",
                cmd: "multi-sel",
            },
        );
        map.insert(
            CmdName::MultiClear,
            CmdData {
                fname: "cmd_multi_clear",
                description: "Clear multi-selection",
                cmd: "multi-clear",
            },
        );
        map.insert(
            CmdName::MultiShow,
            CmdData {
                fname: "cmd_multi_show",
                description: "Show multi-selection",
                cmd: "multi-show",
            },
        );
        map.insert(
            CmdName::MultiSave,
            CmdData {
                fname: "cmd_multi_save",
                description: "Save multi-selection to file",
                cmd: "multi-save",
            },
        );
        map.insert(
            CmdName::MultiCopy,
            CmdData {
                fname: "cmd_multi_copy",
                description: "Copy multi-selection to clipboard",
                cmd: "multi-copy",
            },
        );
        map.insert(
            CmdName::MenuBack,
            CmdData {
                fname: "cmd_menu_back",
                description: "Go back to previous menu",
                cmd: "menu-back",
            },
        );
        map.insert(
            CmdName::Log,
            CmdData {
                fname: "cmd_log",
                description: "Show application log",
                cmd: "log",
            },
        );
        map.insert(
            CmdName::LogClear,
            CmdData {
                fname: "cmd_log_clear",
                description: "Clear application log",
                cmd: "log-clear",
            },
        );
        map.insert(
            CmdName::SecUp,
            CmdData {
                fname: "cmd_sec_up",
                description: "Scroll secondary window up",
                cmd: "sec-up",
            },
        );
        map.insert(
            CmdName::SecDown,
            CmdData {
                fname: "cmd_sec_down",
                description: "Scroll secondary window down",
                cmd: "sec-down",
            },
        );

        map
    }
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
    listing: Vec<ItemInfo>,
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
    mode_cmd: bool,
    show_command_window: bool,
    command_input: String,
    show_output_window: bool,
    output_text: String,
    cmd_list: cmd_list::CmdList,
}
impl<'a> App<'a> {
    fn new() -> Self {
        log!("App initialized");
        Self {
            input: String::new(),
            listing: Vec::new(),
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
            mode_cmd: false,
            show_command_window: false,
            command_input: String::new(),
            show_output_window: false,
            output_text: String::new(),
            cmd_list: cmd_list::make_cmd_list(),
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

    fn set_output(&mut self, text: &str) {
        self.reset_sec_scroll();
        self.output_text = text.to_string();
    }

    fn get_cmd(&self, name: &cmd_list::CmdName) -> &'static str {
        self.cmd_list.get(name).unwrap().cmd
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
        log!("Updating preview for selection: {}", self.selection);
        self.preview_content = Default::default();
        self.reset_sec_scroll();
        match self.selection.as_str() {
            sc::EXIT => {
                self.preview_content += App::fmtln_sc("Exit the application");
                if self.mode_vis_commands {
                    return;
                }
                self.preview_content += Line::from("");
                for (i, line) in LOGO.lines().enumerate() {
                    if i == 0 {
                        continue;
                    };
                    self.preview_content +=
                        Line::styled(format!("{}", line), Style::default().fg(Color::LightGreen));
                }
                for line in WELCOME_MSG.lines() {
                    self.preview_content += Line::from(line);
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
                    "Saves the multi-selection to a file.",
                    Style::default().fg(Color::Green),
                );
            }
            sc::MULTI_COPY => {
                self.preview_content += App::fmtln_sc("Copy multi-selection");
                self.preview_content += Line::styled(
                    "Copies the multi-selection to the clipboard.",
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
            sc::CMDS_LIST => {
                self.preview_content += App::fmtln_sc("Command list");
                self.preview_content += Line::styled(
                    "Show a list of all available commands",
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
        let empty_metadata = match fs::metadata(&self.cwd) {
            Ok(meta) => meta,
            Err(_) => fs::metadata(".").unwrap(),
        };
        // Handle cmd search
        if self.mode_cmd {
            log!("Updating command listing");
            self.listing.clear();
            for (_, cmd_data) in self.cmd_list.iter() {
                self.listing.push(ItemInfo {
                    name: cmd_data.cmd.to_string(),
                    is_sc: true,
                    metadata: empty_metadata.clone(),
                });
            }
            return;
        }
        // Handle visual commands
        if self.mode_vis_commands {
            self.listing.clear();
            self.listing.push(ItemInfo {
                name: sc::MENU_BACK.to_string(),
                is_sc: true,
                metadata: empty_metadata.clone(),
            });
            self.listing.push(ItemInfo {
                name: sc::EXIT.to_string(),
                is_sc: true,
                metadata: empty_metadata.clone(),
            });
            self.listing.push(ItemInfo {
                name: sc::HOME.to_string(),
                is_sc: true,
                metadata: empty_metadata.clone(),
            });
            self.listing.push(ItemInfo {
                name: sc::DIR_UP.to_string(),
                is_sc: true,
                metadata: empty_metadata.clone(),
            });
            self.listing.push(ItemInfo {
                name: sc::EXP.to_string(),
                is_sc: true,
                metadata: empty_metadata.clone(),
            });
            self.listing.push(ItemInfo {
                name: sc::MULTI_SHOW.to_string(),
                is_sc: true,
                metadata: empty_metadata.clone(),
            });
            self.listing.push(ItemInfo {
                name: sc::MULTI_CLEAR.to_string(),
                is_sc: true,
                metadata: empty_metadata.clone(),
            });
            self.listing.push(ItemInfo {
                name: sc::MULTI_SAVE.to_string(),
                is_sc: true,
                metadata: empty_metadata.clone(),
            });
            self.listing.push(ItemInfo {
                name: sc::MULTI_COPY.to_string(),
                is_sc: true,
                metadata: empty_metadata.clone(),
            });
            self.listing.push(ItemInfo {
                name: sc::LOG.to_string(),
                is_sc: true,
                metadata: empty_metadata.clone(),
            });
            self.listing.push(ItemInfo {
                name: sc::LOG_CLEAR.to_string(),
                is_sc: true,
                metadata: empty_metadata.clone(),
            });
            self.listing.push(ItemInfo {
                name: sc::CMDS_LIST.to_string(),
                is_sc: true,
                metadata: empty_metadata.clone(),
            });
            return;
        }
        // Normal directory listing
        log!(
            "Updating directory listing for cwd: {}",
            self.cwd.to_str().unwrap()
        );
        let mut listing = self.get_directory_listing(&self.cwd.clone());
        // Inserted in reverse order
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
                name: sc::CMDS.to_string(),
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
        self.listing = listing;
    }

    fn update_results(&mut self) {
        let matcher = SkimMatcherV2::default();
        let mut scored: Vec<_> = self
            .listing
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

    fn reset_sec_scroll(&mut self) {
        self.scroll_off_preview = 0;
        self.scroll_off_output = 0;
    }

    fn update_selection(&mut self) -> bool {
        let old = self.selection.clone();
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
        return old != self.selection;
    }

    fn input_out_window(&mut self, modifiers: KeyModifiers, code: KeyCode) {
        match (modifiers, code) {
            (KeyModifiers::NONE, KeyCode::Esc) => {
                self.cmd_output_window_hide();
                return;
            }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                self.cmd_output_window_hide();
                return;
            }
            _ => {}
        }
        // Special command matching just for output window
        let cmd = match (modifiers, code) {
            (KeyModifiers::ALT, KeyCode::Char('j')) => self.get_cmd(&cmd_list::CmdName::SecDown),
            (KeyModifiers::ALT, KeyCode::Char('k')) => self.get_cmd(&cmd_list::CmdName::SecUp),
            _ => "",
        };
        self.handle_cmd(&cmd);
    }

    fn input_cmd_window(&mut self, modifiers: KeyModifiers, code: KeyCode) -> LoopReturn {
        match (modifiers, code) {
            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
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
                self.show_command_window = false;
            }
            _ => {}
        }
        LoopReturn::Ok
    }

    // Returns true if input changed
    fn input_main(&mut self, modifiers: KeyModifiers, code: KeyCode) -> bool {
        match (modifiers, code) {
            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
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
            (KeyModifiers::CONTROL, KeyCode::Char('t')) => self.get_cmd(&CmdName::CmdWinToggle),
            (KeyModifiers::CONTROL, KeyCode::Char('f')) => self.get_cmd(&CmdName::CmdFinder),
            (KeyModifiers::CONTROL, KeyCode::Char('s')) => self.get_cmd(&CmdName::MultiSel),
            (KeyModifiers::NONE, KeyCode::Enter) => self.get_cmd(&CmdName::Select),
            (KeyModifiers::NONE, KeyCode::Right) => self.get_cmd(&CmdName::Select),
            (KeyModifiers::NONE, KeyCode::Up) => self.get_cmd(&CmdName::SelUp),
            (KeyModifiers::NONE, KeyCode::Down) => self.get_cmd(&CmdName::SelDown),
            (KeyModifiers::NONE, KeyCode::Left) => self.get_cmd(&CmdName::DirBack),
            (KeyModifiers::CONTROL, KeyCode::Char('h')) => self.get_cmd(&CmdName::DirBack),
            (KeyModifiers::CONTROL, KeyCode::Char('j')) => self.get_cmd(&CmdName::SelDown),
            (KeyModifiers::CONTROL, KeyCode::Char('k')) => self.get_cmd(&CmdName::SelUp),
            (KeyModifiers::CONTROL, KeyCode::Char('l')) => self.get_cmd(&CmdName::Select),
            (KeyModifiers::NONE, KeyCode::Esc) => self.get_cmd(&CmdName::Exit),
            (KeyModifiers::CONTROL, KeyCode::Char('q')) => self.get_cmd(&CmdName::Exit),
            (KeyModifiers::ALT, KeyCode::Char('j')) => self.get_cmd(&CmdName::SecDown),
            (KeyModifiers::ALT, KeyCode::Char('k')) => self.get_cmd(&CmdName::SecUp),
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
        self.show_command_window = !self.show_command_window;
    }

    fn cmd_output_window_toggle(&mut self) {
        self.show_output_window = !self.show_output_window;
    }

    fn cmd_output_window_show(&mut self) {
        self.show_output_window = true;
    }

    fn cmd_output_window_hide(&mut self) {
        self.show_output_window = false;
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
        self.set_output("Multi selection cleared.");
        self.cmd_output_window_show();
    }

    fn cmd_multi_show(&mut self) {
        let mut output_text = String::new();
        if self.multi_selection.is_empty() {
            self.set_output("No items in multi selection.");
            self.cmd_output_window_show();
            return;
        }
        for path in self.multi_selection.iter() {
            output_text += &format!("{}\n", path.to_str().unwrap());
        }
        self.set_output(&output_text);
        self.cmd_output_window_show();
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
        self.set_output(&format!(
            "Multi selection saved to {} ({} items).",
            file.to_str().unwrap(),
            self.multi_selection.len()
        ));
        self.cmd_output_window_show();
    }

    // Copy multi selection to the cwd
    fn cmd_multi_copy(&mut self) {
        let mut output_text = String::new();
        if self.multi_selection.is_empty() {
            self.set_output("No items in multi selection to copy.");
            self.cmd_output_window_show();
            return;
        }
        for path in self.multi_selection.iter() {
            let file_name = match path.file_name() {
                Some(name) => name,
                None => continue,
            };
            let dest_path = self.cwd.join(file_name);
            match fs::copy(&path, &dest_path) {
                Ok(_) => {
                    output_text += &format!(
                        "Copied {} to {}\n",
                        path.to_str().unwrap(),
                        dest_path.to_str().unwrap()
                    );
                }
                Err(e) => {
                    output_text += &format!(
                        "Failed to copy {}: {}\n",
                        path.to_str().unwrap(),
                        e.to_string()
                    );
                }
            }
        }
        self.set_output(&output_text);
        self.cmd_output_window_show();
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

    fn cmd_cmd_finder(&mut self) {
        self.mode_cmd = !self.mode_cmd;
        self.update_listing();
        self.update_results();
        self.selection_index = 0;
    }

    // Show a list of commands
    fn cmd_cmd_list(&mut self) {
        let mut text = String::new();
        text += "Available Commands:\n";
        text += "-------------------\n";
        // Sort by command name
        let mut vec: Vec<_> = self.cmd_list.iter().collect();
        vec.sort_by(|a, b| a.1.cmd.cmp(&b.1.cmd));
        for (_name, cmd_data) in vec {
            text += &format!("{} - {}\n", cmd_data.cmd, cmd_data.description);
        }
        self.set_output(&text);
        self.cmd_output_window_show();
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
                lines.push("-------");
                lines.push("Top of log");
                lines.reverse();
                let content = lines.join("\n");
                self.set_output(content.as_str());
            }
            Err(_) => {
                self.set_output("No log file found.");
            }
        }
        self.cmd_output_window_show();
    }

    fn cmd_sec_up(&mut self) {
        if self.show_output_window {
            if self.scroll_off_output >= 5 {
                self.scroll_off_output -= 5;
            } else {
                self.scroll_off_output = 0;
            }
            log!("Output scroll offset up: {}", self.scroll_off_output);
            return;
        }
        if self.scroll_off_preview >= 5 {
            self.scroll_off_preview -= 5;
        } else {
            self.scroll_off_preview = 0;
        }
        log!("Preview scroll offset up: {}", self.scroll_off_preview);
    }
    fn cmd_sec_down(&mut self) {
        if self.show_output_window {
            let height = self.output_text.split("\n").count() as u16;
            if self.scroll_off_output < height {
                self.scroll_off_output += 5;
            }
            log!(
                "Output scroll offset down: {}/{}",
                self.scroll_off_output,
                height
            );
            return;
        }
        let height = self.preview_content.lines.len() as u16;
        if self.scroll_off_preview < height {
            self.scroll_off_preview += 5;
        }
        log!(
            "Preview scroll offset down: {}/{}",
            self.scroll_off_preview,
            height
        );
    }

    fn cmd_log_clear(&mut self) {
        let log_path = log::log_path();
        match fs::remove_file(&log_path) {
            Ok(_) => {
                self.set_output("Log file cleared.");
            }
            Err(_) => {
                self.set_output("No log file found to clear.");
            }
        }
        self.cmd_output_window_show();
    }

    fn handle_cmd(&mut self, cmd: &str) -> LoopReturn {
        match cmd {
            // The main Select command is long because it handles shortcuts
            _ if cmd == self.get_cmd(&CmdName::Select) => {
                // Update input to empty to reset search
                self.input = String::new();
                self.update_results();
                // Get selection
                let selection = self.selection.clone();
                // NOTE: Handle shortcuts selections
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
                    sc::MULTI_COPY => {}
                    sc::LOG => {
                        self.cmd_log_show();
                    }
                    sc::LOG_CLEAR => {
                        self.cmd_log_clear();
                    }
                    sc::CMDS_LIST => {
                        self.cmd_cmd_list();
                    }
                    _ => {
                        // Selection is a file or directory
                        self.set_cwd(&self.selection.clone().into());
                        self.update_listing();
                        self.update_results();
                        self.selection_index = 0;
                    }
                }
            }
            _ if cmd == self.get_cmd(&CmdName::SelDown) => self.cmd_sel_down(),
            _ if cmd == self.get_cmd(&CmdName::SelUp) => self.cmd_sel_up(),
            _ if cmd == self.get_cmd(&CmdName::DirUp) => self.cmd_dir_up(),
            _ if cmd == self.get_cmd(&CmdName::DirBack) => self.cmd_dir_back(),
            _ if cmd == self.get_cmd(&CmdName::Explode) => self.cmd_explode(),
            _ if cmd == self.get_cmd(&CmdName::Home) => self.cmd_home(),
            _ if cmd == self.get_cmd(&CmdName::CmdWinToggle) => self.cmd_cmd_window_toggle(),
            _ if cmd == self.get_cmd(&CmdName::OutputWinToggle) => self.cmd_output_window_toggle(),
            _ if cmd == self.get_cmd(&CmdName::OutputWinShow) => self.cmd_output_window_show(),
            _ if cmd == self.get_cmd(&CmdName::OutputWinHide) => self.cmd_output_window_hide(),
            _ if cmd == self.get_cmd(&CmdName::MultiSel) => self.cmd_multi_sel(),
            _ if cmd == self.get_cmd(&CmdName::MultiClear) => self.cmd_multi_clear(),
            _ if cmd == self.get_cmd(&CmdName::MultiShow) => self.cmd_multi_show(),
            _ if cmd == self.get_cmd(&CmdName::MultiSave) => self.cmd_multi_save(),
            _ if cmd == self.get_cmd(&CmdName::MultiCopy) => self.cmd_multi_copy(),
            _ if cmd == self.get_cmd(&CmdName::CmdVisToggle) => self.cmd_cmd_vis_toggle(),
            _ if cmd == self.get_cmd(&CmdName::CmdVisShow) => self.cmd_vis_show(),
            _ if cmd == self.get_cmd(&CmdName::CmdFinder) => self.cmd_cmd_finder(),
            _ if cmd == self.get_cmd(&CmdName::CmdList) => self.cmd_cmd_list(),
            _ if cmd == self.get_cmd(&CmdName::MenuBack) => self.cmd_menu_back(),
            _ if cmd == self.get_cmd(&CmdName::Log) => self.cmd_log_show(),
            _ if cmd == self.get_cmd(&CmdName::LogClear) => self.cmd_log_clear(),
            _ if cmd == self.get_cmd(&CmdName::SecDown) => self.cmd_sec_down(),
            _ if cmd == self.get_cmd(&CmdName::SecUp) => self.cmd_sec_up(),
            _ if cmd == self.get_cmd(&CmdName::Exit) => return LoopReturn::Break,
            _ => {
                // If the cmd starts with `!` treat it as a shell command

                if cmd.starts_with('!') {
                    let shell_cmd = &cmd[1..];
                    log!("Running shell command: {}", shell_cmd);
                    match Command::new("sh").arg("-c").arg(shell_cmd).output() {
                        Ok(output) => {
                            let stdout = String::from_utf8_lossy(&output.stdout);
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            let combined_output = format!("{}{}", stdout, stderr);
                            self.set_output(&combined_output);
                        }
                        Err(e) => {
                            self.set_output(&format!("Failed to run command: {}", e));
                        }
                    }
                    self.cmd_output_window_show();
                    return LoopReturn::Ok;
                }
                // If the command isnt empty print the incorrect command
                if !cmd.is_empty() {
                    log!("No command matched: {}", cmd);
                    self.set_output(&format!("No command matched: {}", cmd));
                    self.cmd_output_window_show();
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
                if app.show_output_window {
                    app.input_out_window(modifiers, code);
                    continue;
                }
                // Command window input handling
                if app.show_command_window {
                    let lr = app.input_cmd_window(modifiers, code);
                    match lr {
                        LoopReturn::Continue => continue,
                        LoopReturn::Break => break,
                        LoopReturn::Ok => {}
                    }
                    continue;
                }
                // Before key press handling
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
                let sel_changed = app.update_selection();
                if sel_changed {
                    app.update_preview();
                }
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
    let threshold = 100;

    // --- Widget creation ---
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
    let input_widget = Paragraph::new(input_line).block(
        Block::default()
            .title(format!(" {}", app.cwd.to_str().unwrap()))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green)),
    );

    // Results list
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
    let list_widget = List::new(results_pretty).block(
        Block::default()
            .title(list_title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue)),
    );
    let mut state = ListState::default();
    if !app.results.is_empty() && app.selection_index >= 0 {
        state.select(Some(app.selection_index as usize));
    }

    // Preview box
    let preview_widget = Paragraph::new(app.preview_content.clone())
        .block(
            Block::default()
                .title(format!("{} (0)_(0) {} ", nf::LOOK, app.selection))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow))
                .style(Style::default().bg(Color::Black)),
        )
        .wrap(Wrap { trim: false })
        .scroll((app.scroll_off_preview as u16, app.scroll_off_preview as u16));

    // --- Layout and rendering ---
    if area.width < threshold {
        // Vertical layout
        let vertical_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Length(3), // Input
                    Constraint::Min(5),    // Results
                    Constraint::Min(10),   // Preview
                ]
                .as_ref(),
            )
            .split(area);

        frame.render_widget(input_widget, vertical_chunks[0]);
        frame.render_stateful_widget(list_widget, vertical_chunks[1], &mut state);
        frame.render_widget(preview_widget, vertical_chunks[2]);
    } else {
        // Horizontal layout
        let horizontal_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)].as_ref())
            .split(area);

        let left_vertical_chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(0)
            .constraints([Constraint::Length(3), Constraint::Min(1)].as_ref())
            .split(horizontal_chunks[0]);

        frame.render_widget(input_widget, left_vertical_chunks[0]);
        frame.render_stateful_widget(list_widget, left_vertical_chunks[1], &mut state);
        frame.render_widget(preview_widget, horizontal_chunks[1]);
    }

    // --- Popups ---
    if app.show_command_window {
        let popup_area = centered_rect(50, 10, area);
        let command_str = format!("> {}|", app.command_input);
        frame.render_widget(Clear, popup_area);
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
    if app.show_output_window {
        let popup_area = centered_rect(50, 90, area);
        frame.render_widget(Clear, popup_area);
        let command_paragraph = Paragraph::new(app.output_text.clone())
            .style(Style::default().bg(Color::Black))
            .block(
                Block::default()
                    .title(format!("{} Output ('esc' to exit)", nf::CMD))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Magenta))
                    .style(Style::default().bg(Color::Black)),
            )
            .wrap(Wrap { trim: false })
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
