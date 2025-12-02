use ansi_to_tui::IntoText;
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

// INTERNAL MODULES
use crate::{cmd_list::CmdName, item_info::ItemInfo, node_type::NodeType};

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

const SEP: &str = "=======";

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
    pub const WARN: &str = "";
    pub const BOMB: &str = "";
    pub const B4: &str = "█";
    pub const B3: &str = "▓";
    pub const B2: &str = "▒";
    pub const B1: &str = "░";
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
}

mod cmd_list {
    use std::collections::HashMap;

    use crate::cmd_list;

    #[derive(Hash, Eq, PartialEq, Debug, Clone, PartialOrd, Ord)]
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
        CmdFinderToggle,
        CmdList,
        OutputWinToggle,
        OutputWinShow,
        OutputWinHide,
        MultiSel,
        MultiClear,
        MultiShow,
        MultiSave,
        MultiCopy,
        MultiDelete,
        MenuBack,
        Log,
        LogClear,
        SecUp,
        SecDown,
        ShowKeybinds,
        DbgClearPreview,
        Edit,
        Shell,
    }

    #[derive(Debug, Clone)]
    pub struct CmdData {
        pub fname: &'static str,
        pub description: &'static str,
        pub cmd: &'static str,
        pub vis_hidden: bool, // Hidden from visual cmd selection
    }
    pub type CmdList = HashMap<CmdName, CmdData>;
    pub fn cmd_name_from_str(
        cmd_list: &HashMap<CmdName, cmd_list::CmdData>,
        cmd: &str,
    ) -> Option<cmd_list::CmdName> {
        for (name, data) in cmd_list.iter() {
            if data.cmd == cmd {
                return Some(name.clone());
            }
        }
        None
    }
    pub fn get_cmd_data(
        cmd_list: &cmd_list::CmdList,
        name: &cmd_list::CmdName,
    ) -> cmd_list::CmdData {
        match cmd_list.get(name) {
            Some(data) => data.clone(),
            None => panic!("Command not found: {:?}", name),
        }
    }

    // Helper to get command string from CmdName
    pub fn get_cmd(cmd_list: &cmd_list::CmdList, name: &cmd_list::CmdName) -> String {
        get_cmd_data(cmd_list, name).cmd.to_string()
    }
    pub fn make_cmd_list() -> CmdList {
        let mut map = HashMap::new();
        map.insert(
            CmdName::Exit,
            CmdData {
                fname: "Exit",
                description: "Exit the application",
                cmd: "exit",
                vis_hidden: false,
            },
        );
        map.insert(
            CmdName::Home,
            CmdData {
                fname: "Home",
                description: "Go to your home directory",
                cmd: "home",
                vis_hidden: false,
            },
        );
        map.insert(
            CmdName::SelUp,
            CmdData {
                fname: "Selection Up",
                description: "Move selection up",
                cmd: "sel-up",
                vis_hidden: true,
            },
        );
        map.insert(
            CmdName::SelDown,
            CmdData {
                fname: "Selection Down",
                description: "Move selection down",
                cmd: "sel-down",
                vis_hidden: true,
            },
        );
        map.insert(
            CmdName::DirUp,
            CmdData {
                fname: "Directory Up (cd ..)",
                description: "Go up to the parent directory",
                cmd: "dir-up",
                vis_hidden: false,
            },
        );
        map.insert(
            CmdName::DirBack,
            CmdData {
                fname: "Directory Back (cd -)",
                description: "Go back to the last working directory",
                cmd: "dir-back",
                vis_hidden: false,
            },
        );
        map.insert(
            CmdName::Explode,
            CmdData {
                fname: "Explode Mode Toggle",
                description: "Find all files in subdirectories",
                cmd: "explode",
                vis_hidden: false,
            },
        );
        map.insert(
            CmdName::Select,
            CmdData {
                fname: "Select Current Item",
                description: "Select the current item",
                cmd: "select",
                vis_hidden: true,
            },
        );
        map.insert(
            CmdName::CmdWinToggle,
            CmdData {
                fname: "Command Window Toggle",
                description: "Toggle command window where you can type commands",
                cmd: "cmd-win",
                vis_hidden: false,
            },
        );
        map.insert(
            CmdName::CmdFinderToggle,
            CmdData {
                fname: "Command Finder Toggle",
                description: "Toggle the fuzzy command finder",
                cmd: "cmd-find",
                vis_hidden: false,
            },
        );
        map.insert(
            CmdName::CmdList,
            CmdData {
                fname: "Command List",
                description: "List all commands",
                cmd: "cmd-list",
                vis_hidden: false,
            },
        );
        map.insert(
            CmdName::OutputWinToggle,
            CmdData {
                fname: "Output Window Toggle",
                description: "Toggle output window",
                cmd: "output-toggle",
                vis_hidden: false,
            },
        );
        map.insert(
            CmdName::OutputWinShow,
            CmdData {
                fname: "Output Window Show",
                description: "Show output window",
                cmd: "output-show",
                vis_hidden: false,
            },
        );
        map.insert(
            CmdName::OutputWinHide,
            CmdData {
                fname: "Output Window Hide",
                description: "Hide output window",
                cmd: "output-hide",
                vis_hidden: false,
            },
        );
        map.insert(
            CmdName::MultiSel,
            CmdData {
                fname: "Multi-Select Toggle",
                description: "Toggle multi-selection for current item",
                cmd: "mul-sel",
                vis_hidden: false,
            },
        );
        map.insert(
            CmdName::MultiClear,
            CmdData {
                fname: "Multi-Select Clear",
                description: "Clear multi-selection",
                cmd: "mul-clear",
                vis_hidden: false,
            },
        );
        map.insert(
            CmdName::MultiShow,
            CmdData {
                fname: "Multi-Select Show",
                description: "Show multi-selection in the output window",
                cmd: "mul-show",
                vis_hidden: false,
            },
        );
        map.insert(
            CmdName::MultiSave,
            CmdData {
                fname: "Multi-Select Save",
                description: "Save multi-selection to file",
                cmd: "mul-save",
                vis_hidden: false,
            },
        );
        map.insert(
            CmdName::MultiCopy,
            CmdData {
                fname: "Multi-Select Copy",
                description: "Copy multi-selection to the current directory",
                cmd: "mul-copy",
                vis_hidden: false,
            },
        );
        map.insert(
            CmdName::MultiDelete,
            CmdData {
                fname: "Multi-Select Delete",
                description: "Delete multi-selection files",
                cmd: "mul-delete",
                vis_hidden: false,
            },
        );
        map.insert(
            CmdName::MenuBack,
            CmdData {
                fname: "Menu Back",
                description: "Go back to previous menu",
                cmd: "menu-back",
                vis_hidden: true,
            },
        );
        map.insert(
            CmdName::Log,
            CmdData {
                fname: "Low Viewer",
                description: "Show application log",
                cmd: "log",
                vis_hidden: false,
            },
        );
        map.insert(
            CmdName::LogClear,
            CmdData {
                fname: "Log Clear",
                description: "Clear application log",
                cmd: "log-clear",
                vis_hidden: false,
            },
        );
        map.insert(
            CmdName::SecUp,
            CmdData {
                fname: "Secondary Scroll Up",
                description: "Scroll secondary window up",
                cmd: "sec-up",
                vis_hidden: true,
            },
        );
        map.insert(
            CmdName::SecDown,
            CmdData {
                fname: "Secondary Scroll Down",
                description: "Scroll secondary window down",
                cmd: "sec-down",
                vis_hidden: true,
            },
        );
        map.insert(
            CmdName::ShowKeybinds,
            CmdData {
                fname: "Show Keybinds",
                description: "Show current keybindings",
                cmd: "show-keybinds",
                vis_hidden: false,
            },
        );
        map.insert(
            CmdName::ShowKeybinds,
            CmdData {
                fname: "Show Keybinds",
                description: "Show current keybindings",
                cmd: "show-keybinds",
                vis_hidden: false,
            },
        );
        map.insert(
            CmdName::DbgClearPreview,
            CmdData {
                fname: "Debug Clear Preview",
                description: "Clear preview content. Some terminals may not refresh properly causing artifacts.",
                cmd: "dbg-prev-clear",
                vis_hidden: false,
            },
        );
        map.insert(
            CmdName::Edit,
            CmdData {
                fname: "Edit File",
                description: "Open selected file in $EDITOR",
                cmd: "edit",
                vis_hidden: false,
            },
        );
        map.insert(
            CmdName::Shell,
            CmdData {
                fname: "Shell",
                description: "Demo shell command",
                cmd: "!ls",
                vis_hidden: false,
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

mod node_type {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    #[derive(Clone, PartialEq)]
    pub enum NodeType {
        File,       // A regular file
        Directory,  // A directory
        Shortcut,   // Internal shortcut
        Command,    // Internal command, name should always be the command string
        Executable, // An executable file
        Image,      // An image file
        Unknown,    // Unknown, unsupported, etc.
    }
    impl NodeType {
        pub fn find(metadata: fs::Metadata) -> NodeType {
            if metadata.is_dir() {
                NodeType::Directory
            } else if metadata.is_file() {
                // Check if executable
                #[cfg(unix)]
                {
                    if metadata.permissions().mode() & 0o111 != 0 {
                        return NodeType::Executable;
                    }
                }
                NodeType::File
            } else {
                NodeType::Unknown
            }
        }
    }
}

mod item_info {
    use super::node_type::NodeType;
    // Information about a file or directory
    #[derive(Clone)]
    pub struct ItemInfo {
        pub name: String,
        pub node_type: NodeType,
    }
    impl ItemInfo {
        pub fn new(name: &str, node_type: NodeType) -> Self {
            Self {
                name: name.to_string(),
                node_type,
            }
        }
        pub fn empty() -> Self {
            Self {
                name: String::new(),
                node_type: NodeType::Unknown,
            }
        }
        pub fn is(&self, _is: NodeType) -> bool {
            return self.node_type == _is;
        }
        pub fn is_file(&self) -> bool {
            return self.node_type == NodeType::File;
        }
        pub fn is_dir(&self) -> bool {
            return self.node_type == NodeType::Directory;
        }
        pub fn is_shortcut(&self) -> bool {
            return self.node_type == NodeType::Shortcut;
        }
        pub fn is_command(&self) -> bool {
            return self.node_type == NodeType::Command;
        }
        pub fn is_executable(&self) -> bool {
            return self.node_type == NodeType::Executable;
        }
        pub fn is_image(&self) -> bool {
            return self.node_type == NodeType::Image;
        }
        pub fn is_unknown(&self) -> bool {
            return self.node_type == NodeType::Unknown;
        }
    }
}

mod kb {
    use super::cmd_list;
    use super::cmd_list::CmdName;
    use crate::{APP_NAME, log};
    use crossterm::event::{KeyCode, KeyModifiers};
    use std::{env, fs, path::PathBuf};

    #[derive(Clone)]
    pub struct KeyBind {
        pub modifiers: KeyModifiers,
        pub code: KeyCode,
        pub command: CmdName,
    }
    impl KeyBind {
        fn new(modifiers: KeyModifiers, code: KeyCode, command: CmdName) -> Self {
            Self {
                modifiers,
                code,
                command,
            }
        }
    }
    pub type KeyBindList = Vec<KeyBind>;
    fn make_defaults() -> KeyBindList {
        let mut list = KeyBindList::new();
        list.push(KeyBind::new(
            KeyModifiers::CONTROL,
            KeyCode::Char('t'),
            CmdName::CmdWinToggle,
        ));
        list.push(KeyBind::new(
            KeyModifiers::CONTROL,
            KeyCode::Char('f'),
            CmdName::CmdFinderToggle,
        ));
        list.push(KeyBind::new(
            KeyModifiers::CONTROL,
            KeyCode::Char('s'),
            CmdName::MultiSel,
        ));
        list.push(KeyBind::new(
            KeyModifiers::NONE,
            KeyCode::Tab,
            CmdName::MultiSel,
        ));
        list.push(KeyBind::new(
            KeyModifiers::NONE,
            KeyCode::Enter,
            CmdName::Select,
        ));
        list.push(KeyBind::new(
            KeyModifiers::NONE,
            KeyCode::Right,
            CmdName::Select,
        ));
        list.push(KeyBind::new(
            KeyModifiers::NONE,
            KeyCode::Up,
            CmdName::SelUp,
        ));
        list.push(KeyBind::new(
            KeyModifiers::NONE,
            KeyCode::Down,
            CmdName::SelDown,
        ));
        list.push(KeyBind::new(
            KeyModifiers::NONE,
            KeyCode::Left,
            CmdName::DirBack,
        ));
        list.push(KeyBind::new(
            KeyModifiers::CONTROL,
            KeyCode::Char('h'),
            CmdName::DirBack,
        ));
        list.push(KeyBind::new(
            KeyModifiers::CONTROL,
            KeyCode::Char('j'),
            CmdName::SelDown,
        ));
        list.push(KeyBind::new(
            KeyModifiers::CONTROL,
            KeyCode::Char('k'),
            CmdName::SelUp,
        ));
        list.push(KeyBind::new(
            KeyModifiers::CONTROL,
            KeyCode::Char('l'),
            CmdName::Select,
        ));
        list.push(KeyBind::new(
            KeyModifiers::NONE,
            KeyCode::Esc,
            CmdName::Exit,
        ));
        list.push(KeyBind::new(
            KeyModifiers::CONTROL,
            KeyCode::Char('q'),
            CmdName::Exit,
        ));
        list.push(KeyBind::new(
            KeyModifiers::ALT,
            KeyCode::Char('j'),
            CmdName::SecDown,
        ));
        list.push(KeyBind::new(
            KeyModifiers::ALT,
            KeyCode::Char('k'),
            CmdName::SecUp,
        ));
        list
    }

    pub fn to_string_short(kb: &KeyBind) -> String {
        let modifier = match kb.modifiers {
            KeyModifiers::ALT => "alt",
            KeyModifiers::CONTROL => "ctrl",
            KeyModifiers::SHIFT => "shift",
            KeyModifiers::NONE => "none",
            _ => "UNKNOWN",
        };
        return format!("{}-{}", modifier, kb.code.to_string().to_lowercase());
    }

    // Needs the command list before KeyBind only points to enum
    pub fn to_string_full(cmd_list: &cmd_list::CmdList, kb: &KeyBind) -> String {
        return format!(
            "{:<12} {}\n",
            cmd_list::get_cmd(cmd_list, &kb.command),
            to_string_short(kb)
        );
    }

    pub fn find_by_cmd(keybinds: &KeyBindList, cmd: &cmd_list::CmdName) -> Option<KeyBind> {
        for kb in keybinds.iter() {
            if &kb.command == cmd {
                return Some(kb.clone());
            }
        }
        None
    }

    pub fn get_path() -> PathBuf {
        let kb_path = dirs::config_dir()
            .unwrap_or(env::current_dir().unwrap())
            .join(APP_NAME)
            .join("keybinds.txt");
        kb_path
    }

    pub fn make_list() -> KeyBindList {
        // Read keybinds.txt
        let keybinds = match fs::read_to_string(get_path()) {
            Ok(content) => content,
            Err(_) => {
                log!("keybinds.txt not found, using default keybinds");
                // Try to create default keybinds.txt
                return make_defaults();
            }
        };
        let mut list = KeyBindList::new();
        let cmd_list = cmd_list::make_cmd_list();
        for line in keybinds.lines() {
            // Ignore comments
            if line.starts_with('#') || line.trim().is_empty() {
                continue;
            }
            // Trim whitespace
            let line = line.trim();
            let split = line.split_whitespace().collect::<Vec<&str>>();
            if split.len() != 2 {
                log!("Invalid line in keybinds.txt: {}", line);
                continue;
            }
            let cmd = split[0];
            let combo = split[1];
            let mut modifier = "none";
            let mut code = combo;
            if combo.contains('-') {
                let combo_split = combo.split('-').collect::<Vec<&str>>();
                modifier = combo_split[0];
                code = combo_split[1];
            }
            //
            let cmd = match cmd_list::cmd_name_from_str(&cmd_list, cmd) {
                Some(name) => name,
                None => {
                    log!("Unknown command in keybinds.txt: {}", cmd);
                    continue;
                }
            };
            let modifiers = match modifier.to_lowercase().as_str() {
                "ctrl" => KeyModifiers::CONTROL,
                "alt" => KeyModifiers::ALT,
                "shift" => KeyModifiers::SHIFT,
                "none" => KeyModifiers::NONE,
                _ => {
                    log!("Unknown modifier in keybinds.txt: {}", modifier);
                    continue;
                }
            };
            let code = match code.to_lowercase().as_str() {
                "enter" => KeyCode::Enter,
                "esc" => KeyCode::Esc,
                "up" => KeyCode::Up,
                "down" => KeyCode::Down,
                "left" => KeyCode::Left,
                "right" => KeyCode::Right,
                "tab" => KeyCode::Tab,
                "backspace" => KeyCode::Backspace,
                "home" => KeyCode::Home,
                "end" => KeyCode::End,
                "pageup" => KeyCode::PageUp,
                "pagedown" => KeyCode::PageDown,
                c if c.len() == 1 => {
                    let ch = c.chars().next().unwrap();
                    KeyCode::Char(ch)
                }
                _ => {
                    log!("Unknown key code in keybinds.txt: {}", code);
                    continue;
                }
            };
            let keybind = KeyBind::new(modifiers, code, cmd);
            list.push(keybind);
        }
        list
    }
}

// Return type for loop control
// TODO: Im still suspicious of this design
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
    selection: ItemInfo,
    selection_index: i32,
    multi_selection: Vec<PathBuf>,
    preview_content: Text<'a>,
    scroll_off_preview: u16,
    scroll_off_output: u16,
    cwd: PathBuf,
    lwd: PathBuf,
    mode_explode: bool,
    mode_cmd_finder: bool,
    show_command_window: bool,
    command_input: String,
    show_output_window: bool,
    output_title: String,
    output_text: String,
    cmd_list: cmd_list::CmdList,
    keybinds: kb::KeyBindList,
    keybinds_found: bool,
    // Has external tools
    has_bat: bool,
    // Layout vals - read only
    lay_preview_area: Rect,
}
impl<'a> App<'a> {
    fn new() -> Self {
        log!("App initialized");
        let bat_check = match Command::new("bat").arg("--version").output() {
            Ok(output) => output.status.success(),
            Err(_) => false,
        };
        // FIXME: THIS IS STUPID
        let kb_check = match fs::read_to_string(&kb::get_path()) {
            Ok(_) => true,
            Err(_) => false,
        };

        Self {
            input: String::new(),
            listing: Vec::new(),
            results: Vec::new(),
            selection: ItemInfo::empty(),
            selection_index: 0,
            multi_selection: Vec::new(),
            preview_content: Default::default(),
            scroll_off_preview: 0,
            scroll_off_output: 0,
            cwd: env::current_dir().unwrap(),
            lwd: env::current_dir().unwrap(),
            mode_explode: false,
            mode_cmd_finder: false,
            show_command_window: false,
            command_input: String::new(),
            show_output_window: false,
            output_title: String::new(),
            output_text: String::new(),
            cmd_list: cmd_list::make_cmd_list(),
            keybinds: kb::make_list(),
            keybinds_found: kb_check,
            has_bat: bat_check,
            lay_preview_area: Rect::default(),
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
                                let node_type = NodeType::find(metadata.clone());
                                if !self.mode_explode {
                                    entries.push(ItemInfo {
                                        name: file_name_str.to_string(),
                                        node_type,
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
                                            node_type,
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

    fn set_output(&mut self, title: &str, text: &str) {
        let title = match title {
            "" => "Message",
            _ => title,
        };
        self.output_title = title.to_string();
        self.reset_sec_scroll();
        self.output_text = text.to_string();
    }

    // A simple helper which avoids needing to pass cmd_list everywhere
    fn get_cmd(&self, name: &cmd_list::CmdName) -> String {
        cmd_list::get_cmd(&self.cmd_list, name)
    }

    fn fmtln_info(label: &str, value: &str) -> Line<'a> {
        Line::styled(
            format!("{} {:<12}: {}", nf::INFO, label, value),
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
            let line = if item.is_shortcut() {
                Line::styled(
                    format!("{}{}| {}", ms, nf::CMD, item.name),
                    Style::default().fg(Color::Yellow),
                )
            } else if item.is_dir() {
                Line::styled(
                    format!("{}{}| {}/", ms, nf::DIR, item.name),
                    Style::default().fg(Color::Green),
                )
            } else if item.is_command() {
                Line::styled(
                    format!("{}{}| {}", ms, nf::CMD, item.name),
                    Style::default().fg(Color::Yellow),
                )
            } else if item.is_executable() {
                Line::styled(
                    format!("{}{}| {}", ms, nf::CMD, item.name),
                    Style::default().fg(Color::Magenta),
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
                    format!("{}{}| {}", ms, nf::FILE, name),
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
        self.preview_content += Line::from(SEP);
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

        // Check if bat is available
        // Use bat for preview if available
        if self.has_bat {
            // Use bat for preview
            log!("Using bat for file preview");
            if let Ok(bat_output) = Command::new("bat")
                .arg("--color=always")
                .arg("--style=plain")
                .arg(selected_path.to_str().unwrap())
                .output()
            {
                if bat_output.status.success() {
                    self.preview_content += Line::from(SEP);
                    let bat_content = String::from_utf8_lossy(&bat_output.stdout);
                    let output = match bat_content.as_ref().into_text() {
                        Ok(text) => text,
                        Err(_) => {
                            self.preview_content +=
                                Line::from("Error: Unable to convert bat output to text.");
                            return;
                        }
                    };
                    for line in output.lines.iter().take(100) {
                        self.preview_content += Line::from(line.clone());
                    }
                    return;
                }
            }
        }
        // Fallback to syntect for syntax highlighting
        fn syntect_to_ratatui_color(s: SyntectStyle) -> RColor {
            RColor::Rgb(s.foreground.r, s.foreground.g, s.foreground.b)
        }
        let ss = SyntaxSet::load_defaults_newlines();
        // FIXME: Should only load once
        let ts = ThemeSet::load_defaults();
        let syntax = ss
            .find_syntax_for_file(&selected_path)
            .unwrap_or(None)
            .unwrap_or_else(|| ss.find_syntax_plain_text());
        let mut h = HighlightLines::new(syntax, &ts.themes["base16-eighties.dark"]);

        // Print syntax name
        self.preview_content += App::fmtln_info("detected", syntax.name.as_str());

        self.preview_content += Line::from(SEP);

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
        log!("Updating preview for selection: {}", self.selection.name);
        self.preview_content = Default::default();
        self.reset_sec_scroll();
        match self.selection.name.as_str() {
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
                let kb_exit = kb::find_by_cmd(&self.keybinds, &cmd_list::CmdName::Exit).unwrap();
                let kb_exit_str = kb::to_string_short(&kb_exit);
                self.preview_content += Line::from("");
                self.preview_content +=
                    Line::styled("Tips:", Style::default().fg(Color::LightBlue));
                self.preview_content += Line::from(format!("- Press {} to exit", kb_exit_str));
                self.preview_content +=
                    Line::from("- Start typing to fuzzy find files and directories");
                self.preview_content += Line::from("");
                self.preview_content +=
                    Line::styled("System Information:", Style::default().fg(Color::LightBlue));
                if self.has_bat {
                    self.preview_content += Line::styled(
                        format!(
                            "- {} 'bat' - file previews will use 'bat' for syntax highlighting",
                            nf::CHECK
                        ),
                        Style::default().fg(Color::Green),
                    );
                } else {
                    self.preview_content += Line::styled(
                        format!(
                            "- {} 'bat' - file previews will use built-in syntax highlighting",
                            nf::WARN
                        ),
                        Style::default().fg(Color::Yellow),
                    );
                }
                if self.keybinds_found {
                    self.preview_content += Line::styled(
                        format!(
                            "- {} keybinds - loaded from {}",
                            nf::CHECK,
                            kb::get_path().to_str().unwrap()
                        ),
                        Style::default().fg(Color::Green),
                    );
                } else {
                    self.preview_content += Line::styled(
                        format!(
                            "- {} keybinds - no keybinds found at {}",
                            nf::WARN,
                            kb::get_path().to_str().unwrap()
                        ),
                        Style::default().fg(Color::Yellow),
                    );
                }
                self.preview_content += Line::from("");
            }
            sc::HOME => {
                self.preview_content += App::fmtln_path(&dirs::home_dir().unwrap());
                self.preview_content += App::fmtln_sc("Go to the home directory");
            }
            sc::DIR_UP => {
                let up_path = self.cwd.parent().unwrap_or(&self.cwd);
                self.preview_content += App::fmtln_path(&up_path.to_path_buf());
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
            _ => {
                self.preview_content = Default::default();
                // Check if we have an internal command
                if self.selection.is_command() {
                    let cmd_name =
                        match cmd_list::cmd_name_from_str(&self.cmd_list, &self.selection.name) {
                            Some(name) => name,
                            None => {
                                self.preview_content +=
                                    Line::from("Error: Command data not found.");
                                return;
                            }
                        };
                    let data = cmd_list::get_cmd_data(&self.cmd_list, &cmd_name).clone();
                    self.preview_content += Line::styled(
                        format!("name: {}", data.fname),
                        Style::default().fg(Color::Green),
                    );
                    self.preview_content += Line::styled(
                        format!("cmd : {}", data.cmd),
                        Style::default().fg(Color::Yellow),
                    );
                    self.preview_content += Line::from(format!("info: {}", data.description));
                    return;
                }
                // We have a file or dir
                let mut selected_path = self.cwd.clone();
                selected_path.push(&self.selection.name);

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
        // Handle cmd finder
        if self.mode_cmd_finder {
            log!("Updating command listing");
            self.listing.clear();
            // Soft the commands alphabetically
            let mut entries: Vec<_> = self.cmd_list.iter().collect();
            entries.sort_by(|a, b| a.1.cmd.cmp(b.1.cmd));
            for (_, cmd_data) in entries {
                if cmd_data.vis_hidden {
                    continue;
                }
                self.listing.push(ItemInfo {
                    name: cmd_data.cmd.to_string(),
                    node_type: NodeType::Command,
                });
            }
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
                node_type: NodeType::Shortcut,
            },
        );
        listing.insert(
            0,
            ItemInfo {
                name: sc::DIR_UP.to_string(),
                node_type: NodeType::Shortcut,
            },
        );
        listing.insert(
            0,
            ItemInfo {
                name: sc::CMDS.to_string(),
                node_type: NodeType::Shortcut,
            },
        );
        listing.insert(
            0,
            ItemInfo {
                name: sc::EXP.to_string(),
                node_type: NodeType::Shortcut,
            },
        );
        listing.insert(
            0,
            ItemInfo {
                name: sc::EXIT.to_string(),
                node_type: NodeType::Shortcut,
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
            self.selection = self.results[self.selection_index as usize].clone();
        } else if !self.results.is_empty() {
            self.selection_index = 0;
            self.selection = ItemInfo::empty();
        } else {
            self.selection_index = 0;
            self.selection = ItemInfo::empty();
        }
        // Remove icon prefix from selection
        // NOTE: This should be safe since file name should not contain pipe
        if let Some(pos) = self.selection.name.find("| ") {
            self.selection.name = self.selection.name[(pos + 2)..].to_string();
        }
        return old.name != self.selection.name;
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
            _ => "".to_string(),
        };
        self.handle_cmd(&cmd.to_string());
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

    fn input_keybinds(&mut self, modifiers: KeyModifiers, code: KeyCode) -> String {
        let mut cmd = String::new();
        for kb in self.keybinds.iter() {
            if kb.modifiers == modifiers && kb.code == code {
                cmd = self.get_cmd(&kb.command).to_string();
                break;
            }
        }
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
        self.set_cwd(&"..".into());
        self.update_listing();
        self.update_results();
        self.selection_index = 0;
    }

    fn cmd_dir_back(&mut self) {
        self.set_cwd(&self.lwd.clone());
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
        selected_path.push(&self.selection.name);
        let is_sc = self
            .results
            .get(self.selection_index as usize)
            .map_or(false, |item| item.is_shortcut());
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
        self.set_output("", "Multi selection cleared.");
        self.cmd_output_window_show();
    }

    fn cmd_multi_show(&mut self) {
        let mut output_text = String::new();
        if self.multi_selection.is_empty() {
            self.set_output("Multi-select", "No items in multi selection.");
            self.cmd_output_window_show();
            return;
        }
        for path in self.multi_selection.iter() {
            output_text += &format!("{}\n", path.to_str().unwrap());
        }
        self.set_output("Multi-select", &output_text);
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
        self.set_output(
            "Saved",
            &format!(
                "Multi selection saved to {} ({} items).",
                file.to_str().unwrap(),
                self.multi_selection.len()
            ),
        );
        self.cmd_output_window_show();
    }

    // Copy multi selection to the cwd
    fn cmd_multi_copy(&mut self) {
        let mut output_text = String::new();
        if self.multi_selection.is_empty() {
            self.set_output("Multi-select", "No items in multi selection to copy.");
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
        self.set_output("Multi-select", &output_text);
        self.cmd_output_window_show();
    }

    fn cmd_multi_delete(&mut self) {
        let mut output_text = String::new();
        if self.multi_selection.is_empty() {
            self.set_output("Multi-select", "No items in multi selection to delete.");
            self.cmd_output_window_show();
            return;
        }
        for path in self.multi_selection.iter() {
            match fs::remove_file(&path) {
                Ok(_) => {
                    output_text += &format!("Deleted {}\n", path.to_str().unwrap());
                }
                Err(e) => {
                    output_text += &format!(
                        "Failed to delete {}: {}\n",
                        path.to_str().unwrap(),
                        e.to_string()
                    );
                }
            }
        }
        self.multi_selection.clear();
        self.set_output("Multi-select", &output_text);
        self.cmd_output_window_show();
    }

    fn cmd_cmd_finder_toggle(&mut self) {
        self.mode_cmd_finder = !self.mode_cmd_finder;
        self.update_listing();
        self.update_results();
        self.selection_index = 0;
    }

    // Show a list of commands
    fn cmd_cmd_list(&mut self) {
        let mut text = String::new();
        // Sort by command name
        let mut vec: Vec<_> = self.cmd_list.iter().collect();
        vec.sort_by(|a, b| a.1.cmd.cmp(&b.1.cmd));
        for (_name, cmd_data) in vec {
            text += &format!("{} - {}\n", cmd_data.cmd, cmd_data.description);
        }
        self.set_output("Available Commands", &text);
        self.cmd_output_window_show();
    }

    // Deprecated?
    fn cmd_menu_back(&mut self) {
        // self.mode_vis_commands = false;
        // self.update_listing();
        // self.update_results();
        // self.selection_index = 0;
    }

    fn cmd_log_show(&mut self) {
        let log_path = log::log_path();
        match fs::read_to_string(&log_path) {
            Ok(content) => {
                // Reverse the log content to show latest entries first
                let mut lines: Vec<&str> = content.lines().collect();
                lines.push(SEP);
                lines.push("Top of log");
                lines.reverse();
                let content = lines.join("\n");
                self.set_output("Log", content.as_str());
            }
            Err(_) => {
                self.set_output("Log", "No log file found.");
            }
        }
        self.cmd_output_window_show();
    }

    fn cmd_log_clear(&mut self) {
        let log_path = log::log_path();
        match fs::remove_file(&log_path) {
            Ok(_) => {
                self.set_output("Log", "Log file cleared.");
            }
            Err(_) => {
                self.set_output("Log", "No log file found to clear.");
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

    fn cmd_show_keybinds(&mut self) {
        let kb_path = kb::get_path();
        let found = self.keybinds_found;
        let mut out = String::from(format!("Path: {}", kb_path.to_str().unwrap()));
        if !found {
            out += " \n\n(not found, using defaults)";
        }

        out += "\n\nKeybinds:\n";

        for kb in self.keybinds.iter() {
            out += kb::to_string_full(&self.cmd_list, kb).as_str();
        }

        self.set_output("Keybinds", &out);
        self.cmd_output_window_show();
    }

    // Edit the selected file
    fn cmd_edit(&mut self) {
        let mut selected_path = self.cwd.clone();
        selected_path.push(&self.selection.name);
        let editor = env::var("EDITOR").unwrap_or("vi".to_string());
        log!(
            "Opening editor: {} {}",
            editor,
            selected_path.to_str().unwrap()
        );
        match Command::new(editor)
            .arg(selected_path.to_str().unwrap())
            .status()
        {
            Ok(_) => {
                self.set_output("Editor", "Editor closed.");
            }
            Err(e) => {
                self.set_output("Editor", &format!("Failed to open editor: {}", e));
            }
        }
        self.cmd_output_window_show();
    }

    fn cmd_dbg_clear_preview(&mut self) {
        self.preview_content = Default::default();
        for _ in 0..self.lay_preview_area.height {
            self.preview_content +=
                Line::from((nf::B4).repeat(self.lay_preview_area.width as usize - 2));
        }
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
                match selection.name.as_str() {
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
                    sc::MENU_BACK => {
                        self.cmd_menu_back();
                        return LoopReturn::Continue;
                    }
                    sc::CMDS => {
                        self.cmd_cmd_finder_toggle();
                        return LoopReturn::Continue;
                    }
                    _ => {
                        // Check if selection is an internal command
                        if selection.is_command() {
                            self.handle_cmd(&selection.name);
                            self.cmd_cmd_finder_toggle();
                            return LoopReturn::Continue;
                        }
                        // Selection is a file or directory
                        self.set_cwd(&self.selection.name.clone().into());
                        self.update_listing();
                        self.update_results();
                        self.selection_index = 0;
                    }
                }
            }
            _ if cmd == self.get_cmd(&CmdName::Exit) => return LoopReturn::Break,
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
            _ if cmd == self.get_cmd(&CmdName::MultiDelete) => self.cmd_multi_delete(),
            _ if cmd == self.get_cmd(&CmdName::CmdFinderToggle) => self.cmd_cmd_finder_toggle(),
            _ if cmd == self.get_cmd(&CmdName::CmdList) => self.cmd_cmd_list(),
            _ if cmd == self.get_cmd(&CmdName::MenuBack) => self.cmd_menu_back(),
            _ if cmd == self.get_cmd(&CmdName::Log) => self.cmd_log_show(),
            _ if cmd == self.get_cmd(&CmdName::LogClear) => self.cmd_log_clear(),
            _ if cmd == self.get_cmd(&CmdName::SecDown) => self.cmd_sec_down(),
            _ if cmd == self.get_cmd(&CmdName::SecUp) => self.cmd_sec_up(),
            _ if cmd == self.get_cmd(&CmdName::ShowKeybinds) => self.cmd_show_keybinds(),
            _ if cmd == self.get_cmd(&CmdName::Edit) => self.cmd_edit(),
            _ if cmd == self.get_cmd(&CmdName::DbgClearPreview) => self.cmd_dbg_clear_preview(),
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
                            self.set_output("Shell", &combined_output);
                        }
                        Err(e) => {
                            self.set_output("Shell", &format!("Failed to run command: {}", e));
                        }
                    }
                    self.cmd_output_window_show();
                    return LoopReturn::Ok;
                }
                // If the command isnt empty print the incorrect command
                if !cmd.is_empty() {
                    log!("No command matched: {}", cmd);
                    self.set_output("Shell", &format!("No command matched: {}", cmd));
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
        terminal.draw(|f| render(f, &mut app))?;
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
                let cmd = app.input_keybinds(modifiers, code);
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

fn render(frame: &mut Frame, app: &mut App) {
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
    let area = frame.area();
    // frame.render_widget(Clear, area); // Clear the area
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
            .title(format!(
                "( {} ) ) )  [ {} / {} ] ",
                APP_NAME.to_uppercase(),
                app.results.len(),
                app.listing.len(),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green)),
    );

    // Results list
    let mut results_pretty = app.dir_list_pretty(&app.results);
    // TODO: This is slow and should be done on dir pretty
    for (idx, line) in results_pretty.lines.iter_mut().enumerate() {
        if idx as i32 == app.selection_index {
            let span = Span::styled(
                format!("{}", nf::SEL),
                Style::default().fg(Color::LightBlue).bg(Color::Black),
            );
            let mut new_line = Line::from(span);
            new_line.push_span(Span::raw(format!(" {}", line)));
            *line = new_line;
        }
    }
    let explode_str = if app.mode_explode {
        format!(" [{} exp]", nf::BOMB)
    } else {
        "".to_string()
    };
    let list_title = format!("|{}{} ", app.cwd.to_str().unwrap(), explode_str);
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
                .title(format!("{} m(0)_(0)m | {} ", nf::LOOK, app.selection.name))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
            // .style(Style::default().bg(Color::Back)),
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
        app.lay_preview_area = vertical_chunks[2];
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
        app.lay_preview_area = horizontal_chunks[1];
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
                    .title(format!("{} {} ('esc' to exit)", nf::CMD, app.output_title))
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
