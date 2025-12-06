use ansi_to_tui::IntoText;
use color_eyre::eyre::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
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
use ratatui_image::{
    StatefulImage,
    picker::{Picker, ProtocolType},
    protocol::StatefulProtocol,
};
use regex::Regex;
use std::os::unix::fs::PermissionsExt;
use std::time::UNIX_EPOCH;
use std::{env, process::Command};
use std::{fs, time::SystemTime};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style as SyntectStyle, ThemeSet};
use syntect::parsing::SyntaxSet;
// Might want Path
use std::path::PathBuf;

// INTERNAL MODULES
use crate::{node_info::NodeInfo, node_info::NodeType};

const APP_NAME: &str = "sona";

const LOGO: &str = r#"
 ██╗███████╗ ██████╗ ███╗   ██╗ █████╗ ██╗ ██╗ ██╗ 
██╔╝██╔════╝██╔═══██╗████╗  ██║██╔══██╗╚██╗╚██╗╚██╗
██║ ███████╗██║   ██║██╔██╗ ██║███████║ ██║ ██║ ██║
██║ ╚════██║██║   ██║██║╚██╗██║██╔══██║ ██║ ██║ ██║
╚██╗███████║╚██████╔╝██║ ╚████║██║  ██║██╔╝██╔╝██╔╝
 ╚═╝╚══════╝ ╚═════╝ ╚═╝  ╚═══╝╚═╝  ╚═╝╚═╝ ╚═╝ ╚═╝ 
"#;

const SEP: &str = "───────────────────────────────────────────────";

const ASK: &str = "{ASK}";

// Nerd font icons
mod nf {
    pub const MAG: &str = "󰍉";
    pub const LOOK: &str = "󰮔";
    pub const SEL: &str = ""; //➤
    pub const MSEL: &str = "󰅎";
    pub const DIR: &str = "";
    pub const DIRO: &str = "󰉒";
    pub const FILE: &str = "";
    pub const IMG: &str = "󰋩";
    pub const CMD: &str = "";
    pub const INFO: &str = "";
    pub const CHECK: &str = "";
    pub const WARN: &str = "";
    pub const BOMB: &str = "";
    // UNUSED
    // pub const B4: &str = "█";
    // pub const B3: &str = "▓";
    // pub const B2: &str = "▒";
    // pub const B1: &str = "░";
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

// Command implementations
mod cmd {
    use crate::util;
    use crate::{APP_NAME, App, SEP, cfg, cmd_data, cs, kb, log, sc, shell_cmds};
    use clipboard::ClipboardContext;
    use clipboard::ClipboardProvider;
    use std::{env, fs, path::PathBuf, process::Command};

    pub fn exit(app: &mut App, _args: Vec<&str>) {
        app.should_quit = true;
    }

    pub fn enter(app: &mut App, _args: Vec<&str>) {
        // Update input to empty to reset search
        app.input = String::new();
        app.update_results();
        // Get focused
        let focused = app.focused.clone();
        // NOTE: Handle shortcuts selections
        // Handle internal commands
        // Handle actual selection
        match focused.name.as_str() {
            // Shortcuts
            sc::EXIT => {}
            sc::HOME => {
                home(app, vec![]);
                return;
            }
            sc::DIR_UP => {
                dir_up(app, vec![]);
                return;
            }
            sc::DIR_BACK => {
                dir_back(app, vec![]);
                return;
            }
            sc::EXP => {
                explode(app, vec![]);
                return;
            }
            sc::MENU_BACK => {
                menu_back(app, vec![]);
                return;
            }
            sc::CMDS => {
                cmd_finder_toggle(app, vec![]);
                return;
            }
            // Either an internal command, file or dir
            _ => {
                // Check if focused is an internal command
                if focused.is_command() {
                    let cmd_name = match cmd_data::cmd_name_from_str(&app.cmd_list, &focused.name) {
                        Some(name) => name,
                        None => {
                            app.set_output("Error", "Command data not found for focused command.");
                            output_window_show(app, vec![]);
                            return;
                        }
                    };
                    app.mode_cmd_finder = false;
                    app.update_listing();
                    app.update_results();
                    // If the command has params, open command window
                    let cmd_data = cmd_data::get_cmd_data(&app.cmd_list, &cmd_name);
                    if cmd_data.params.len() > 0 {
                        // Open command window for params
                        app.command_input = format!("{} ", cmd_data.cmd);
                        app.show_command_window = true;
                        return;
                    }
                    app.handle_cmd(&focused.name);
                    return;
                }
                // Check if focused is a shell command
                if focused.is_shell_command() {
                    app.mode_cmd_finder = false;
                    app.update_listing();
                    app.update_results();
                    app.handle_cmd(&focused.name);
                    return;
                }
                // If we have a file, run the on_enter command
                // We have a directory, enter it
                if focused.is_file() {
                    app.handle_cmd(app.cfg.cmd_on_enter.clone().as_str());
                    return;
                } else if focused.is_dir() {
                    app.append_cwd(&app.focused.name.clone().into());
                    app.update_listing();
                    app.update_results();
                    app.focus_index = 0;
                    return;
                } else {
                    app.set_output("Error", "Selected item is neither a file nor a directory.");
                    output_window_show(app, vec![]);
                    return;
                }
            }
        }
    }

    pub fn home(app: &mut App, _args: Vec<&str>) {
        app.append_cwd(&dirs::home_dir().unwrap());
        app.update_listing();
        app.update_results();
        app.focus_index = 0;
    }

    pub fn dir_up(app: &mut App, _args: Vec<&str>) {
        app.append_cwd(&"..".into());
        app.update_listing();
        app.update_results();
        app.focus_index = 0;
    }

    pub fn dir_back(app: &mut App, _args: Vec<&str>) {
        app.append_cwd(&app.lwd.clone());
        app.update_listing();
        app.update_results();
        app.focus_index = 0;
    }

    pub fn dir_reload(app: &mut App, _args: Vec<&str>) {
        app.update_listing();
        app.update_results();
        app.focus_index = 0;
    }

    pub fn explode(app: &mut App, _args: Vec<&str>) {
        app.mode_explode = !app.mode_explode;
        app.update_listing();
        app.update_results();
        app.update_focused();
        app.update_preview();
        app.focus_index = 0;
    }

    pub fn cur_down(app: &mut App, _args: Vec<&str>) {
        app.focus_index += 1;
        if app.focus_index >= app.results.len() as i32 {
            app.focus_index = 0;
        }
    }

    pub fn cur_up(app: &mut App, _args: Vec<&str>) {
        app.focus_index += -1;
        if app.focus_index < 0 && !app.results.is_empty() {
            app.focus_index = app.results.len() as i32 - 1;
        } else if app.results.is_empty() {
            app.focus_index = 0;
        }
    }

    pub fn cmd_window_toggle(app: &mut App, _args: Vec<&str>) {
        app.show_command_window = !app.show_command_window;
    }

    pub fn output_window_toggle(app: &mut App, _args: Vec<&str>) {
        app.show_output_window = !app.show_output_window;
    }

    pub fn output_window_show(app: &mut App, _args: Vec<&str>) {
        app.show_output_window = true;
    }

    pub fn output_window_hide(app: &mut App, _args: Vec<&str>) {
        app.show_output_window = false;
    }

    pub fn sel(app: &mut App, _args: Vec<&str>) {
        if !app.focused.is_file() && !app.focused.is_dir() {
            return;
        }
        let mut focused_path = app.cwd.clone();
        focused_path.push(&app.focused.name);
        // Check if already in multi selection
        if let Some(pos) = app.multi_selection.iter().position(|x| *x == focused_path) {
            app.multi_selection.remove(pos);
        } else {
            app.multi_selection.push(focused_path);
        }
    }

    pub fn sel_clear(app: &mut App, _args: Vec<&str>) {
        app.multi_selection.clear();
        app.set_output("", "Multi selection cleared.");
        output_window_show(app, vec![]);
    }

    pub fn sel_show(app: &mut App, _args: Vec<&str>) {
        let mut output_text = String::new();
        if app.multi_selection.is_empty() {
            app.set_output("Multi-select", "No items in multi selection.");
            output_window_show(app, vec![]);
            return;
        }
        for path in app.multi_selection.iter() {
            output_text += &format!("{}\n", path.to_str().unwrap());
        }
        app.set_output("Multi-select", &output_text);
        output_window_show(app, vec![]);
    }

    // Write multi selection to a file
    pub fn sel_save(app: &mut App, _args: Vec<&str>) {
        let tmp = env::temp_dir();
        let file = tmp.join(APP_NAME).join("multi.txt");
        fs::write(
            &file,
            app.multi_selection
                .iter()
                .map(|p| p.to_str().unwrap())
                .collect::<Vec<&str>>()
                .join("\n"),
        )
        .unwrap_or(());
        app.set_output(
            "Saved",
            &format!(
                "Multi selection saved to {} ({} items).",
                file.to_str().unwrap(),
                app.multi_selection.len()
            ),
        );
        output_window_show(app, vec![]);
    }

    // Copy multi selection to the cwd
    pub fn sel_copy(app: &mut App, _args: Vec<&str>) {
        let mut output_text = String::new();
        if app.multi_selection.is_empty() {
            app.set_output("Multi-select", "No items in multi selection to copy.");
            output_window_show(app, vec![]);
            return;
        }
        for path in app.multi_selection.iter() {
            let file_name = match path.file_name() {
                Some(name) => name,
                None => continue,
            };
            let dest_path = app.cwd.join(file_name);
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
        app.set_output("Multi-select", &output_text);
        output_window_show(app, vec![]);
    }

    pub fn sel_delete(app: &mut App, _args: Vec<&str>) {
        let mut output_text = String::new();
        if app.multi_selection.is_empty() {
            app.set_output("Multi-select", "No items in multi selection to delete.");
            output_window_show(app, vec![]);
            return;
        }
        for path in app.multi_selection.iter() {
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
        app.multi_selection.clear();
        app.set_output("Multi-select", &output_text);
        output_window_show(app, vec![]);
    }

    pub fn sel_move(app: &mut App, _args: Vec<&str>) {
        let mut output_text = String::new();
        if app.multi_selection.is_empty() {
            app.set_output("Multi-select", "No items in multi selection to move.");
            output_window_show(app, vec![]);
            return;
        }
        for path in app.multi_selection.iter() {
            let file_name = match path.file_name() {
                Some(name) => name,
                None => continue,
            };
            let dest_path = app.cwd.join(file_name);
            match fs::rename(&path, &dest_path) {
                Ok(_) => {
                    output_text += &format!(
                        "Moved {} to {}\n",
                        path.to_str().unwrap(),
                        dest_path.to_str().unwrap()
                    );
                }
                Err(e) => {
                    output_text += &format!(
                        "Failed to move {}: {}\n",
                        path.to_str().unwrap(),
                        e.to_string()
                    );
                }
            }
        }
        app.multi_selection.clear();
        app.set_output("Multi-select", &output_text);
        output_window_show(app, vec![]);
    }

    pub fn sel_clip_path(app: &mut App, _args: Vec<&str>) {
        if app.multi_selection.is_empty() {
            app.set_output(
                "Multi-select",
                "No items in multi selection to copy to clipboard.",
            );
            output_window_show(app, vec![]);
            return;
        }
        let paths = app
            .multi_selection
            .iter()
            .map(|p| p.to_str().unwrap())
            .collect::<Vec<&str>>()
            .join("\n");
        let mut ctx: ClipboardContext = ClipboardContext::new().unwrap();
        match ctx.set_contents(paths) {
            Ok(_) => {
                app.set_output("Multi-select", "Multi selection paths copied to clipboard.");
            }
            Err(e) => {
                app.set_output(
                    "Multi-select",
                    &format!("Failed to copy to clipboard: {}", e),
                );
            }
        }
        output_window_show(app, vec![]);
    }

    pub fn cmd_finder_toggle(app: &mut App, _args: Vec<&str>) {
        app.mode_cmd_finder = !app.mode_cmd_finder;
        app.update_listing();
        app.update_results();
        app.focus_index = 0;
    }

    // Show a list of commands
    pub fn cmd_list(app: &mut App, _args: Vec<&str>) {
        let mut text = String::new();
        // Sort by command name
        let mut vec: Vec<_> = app.cmd_list.iter().collect();
        vec.sort_by(|a, b| a.1.cmd.cmp(&b.1.cmd));
        for (_name, cmd_data) in vec {
            text += &format!("{} - {}\n", cmd_data.cmd, cmd_data.description);
        }
        app.set_output("Available Commands", &text);
        output_window_show(app, vec![]);
    }

    // Deprecated?
    pub fn menu_back(_app: &mut App, _args: Vec<&str>) {
        // app.mode_vis_commands = false;
        // app.update_listing();
        // app.update_results();
        // app.selection_index = 0;
    }

    pub fn log_show(app: &mut App, _args: Vec<&str>) {
        let log_path = log::log_path();
        match fs::read_to_string(&log_path) {
            Ok(content) => {
                // Reverse the log content to show latest entries first
                let mut lines: Vec<&str> = content.lines().collect();
                lines.push(SEP);
                lines.push("Top of log");
                lines.reverse();
                let content = lines.join("\n");
                app.set_output("Log", content.as_str());
            }
            Err(_) => {
                app.set_output("Log", "No log file found.");
            }
        }
        output_window_show(app, vec![]);
    }

    pub fn log_clear(app: &mut App, _args: Vec<&str>) {
        let log_path = log::log_path();
        match fs::remove_file(&log_path) {
            Ok(_) => {
                app.set_output("Log", "Log file cleared.");
            }
            Err(_) => {
                app.set_output("Log", "No log file found to clear.");
            }
        }
        output_window_show(app, vec![]);
    }

    pub fn sec_up(app: &mut App, _args: Vec<&str>) {
        if app.show_output_window {
            if app.scroll_off_output >= 5 {
                app.scroll_off_output -= 5;
            } else {
                app.scroll_off_output = 0;
            }
            log!("Output scroll offset up: {}", app.scroll_off_output);
            return;
        }
        if app.scroll_off_preview >= 5 {
            app.scroll_off_preview -= 5;
        } else {
            app.scroll_off_preview = 0;
        }
        log!("Preview scroll offset up: {}", app.scroll_off_preview);
    }

    pub fn sec_down(app: &mut App, _args: Vec<&str>) {
        if app.show_output_window {
            let height = app.output_text.split("\n").count() as u16;
            if app.scroll_off_output < height {
                app.scroll_off_output += 5;
            }
            log!(
                "Output scroll offset down: {}/{}",
                app.scroll_off_output,
                height
            );
            return;
        }
        let height = app.preview_content.lines.len() as u16;
        if app.scroll_off_preview < height {
            app.scroll_off_preview += 5;
        }
        log!(
            "Preview scroll offset down: {}/{}",
            app.scroll_off_preview,
            height
        );
    }

    pub fn show_keybinds(app: &mut App, _args: Vec<&str>) {
        let kb_path = kb::get_path();
        let found = app.found_keybinds;
        let mut out = String::from(format!("Path: {}", kb_path.to_str().unwrap()));
        if !found {
            out += " \n\n(not found, using defaults)";
        }

        out += "\n\nKeybinds:\n";

        for kb in app.keybinds.iter() {
            out += kb::to_string_full(&app.cmd_list, kb).as_str();
        }

        app.set_output("Keybinds", &out);
        output_window_show(app, vec![]);
    }

    // Edit the focused file
    pub fn edit(app: &mut App, _args: Vec<&str>) {
        let mut focused_path = app.cwd.clone();
        focused_path.push(&app.focused.name);
        let editor = env::var("EDITOR").unwrap_or("vi".to_string());
        log!(
            "Opening editor: {} {}",
            editor,
            focused_path.to_str().unwrap()
        );
        match Command::new(editor)
            .arg(focused_path.to_str().unwrap())
            .status()
        {
            Ok(_) => {}
            Err(e) => {
                app.set_output("Editor", &format!("Failed to open editor: {}", e));
                output_window_show(app, vec![]);
            }
        }
    }

    pub fn goto(app: &mut App, args: Vec<&str>) {
        if args.is_empty() {
            app.set_output("Goto", "Error: No path provided.");
            output_window_show(app, vec![]);
            return;
        }
        let path = PathBuf::from(args[0]);
        app.append_cwd(&path);
        app.update_listing();
        app.update_results();
        app.focus_index = 0;
    }

    pub fn input_clear(app: &mut App, _args: Vec<&str>) {
        app.input.clear();
        app.command_input.clear();
        app.update_results();
        app.focus_index = 0;
    }

    pub fn shell_quick(app: &mut App, args: Vec<&str>) {
        // Join args into a single command string
        let mut shell_cmd = args[0..].join(" ");

        // Replace variables
        shell_cmd = app.replace_shell_vars(shell_cmd);

        // Run the command
        log!("Running shell command: {}", shell_cmd);
        match Command::new("sh").arg("-c").arg(shell_cmd).output() {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let combined_output = format!("{}{}", stdout, stderr);
                app.set_output("Shell", &combined_output);
            }
            Err(e) => {
                app.set_output("Shell", &format!("Failed to run command: {}", e));
            }
        }
        output_window_show(app, vec![]);
    }

    pub fn shell_full(app: &mut App, _args: Vec<&str>) {
        let shell = env::var("SHELL").unwrap_or("/bin/sh".to_string());
        log!("Opening shell: {}", shell);
        util::cls();
        match Command::new(shell).status() {
            Ok(_) => {
                app.set_output("Shell", "Shell closed.");
            }
            Err(e) => {
                app.set_output("Shell", &format!("Failed to open shell: {}", e));
            }
        }
        util::cls();
        app.term_clear = true;
        output_window_show(app, vec![]);
    }

    pub fn config_init(app: &mut App, _args: Vec<&str>) {
        // Write the default files to the config directory
        let config_path = cfg::Config::get_path();
        let colors_path = cs::Colors::get_path();
        let kb_path = kb::get_path();
        let shell_cmds_path = shell_cmds::get_path();
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        fs::create_dir_all(colors_path.parent().unwrap()).unwrap();
        fs::create_dir_all(kb_path.parent().unwrap()).unwrap();
        fs::create_dir_all(shell_cmds_path.parent().unwrap()).unwrap();
        let mut output_text = String::new();
        match fs::write(&config_path, cfg::DEFAULT) {
            Ok(_) => {
                output_text += &format!("Created config file: {}\n", config_path.to_str().unwrap());
            }
            Err(e) => {
                output_text += &format!(
                    "Failed to create config file: {} ({})\n",
                    config_path.to_str().unwrap(),
                    e
                );
            }
        }
        match fs::write(&colors_path, cs::DEFAULT) {
            Ok(_) => {
                output_text += &format!("Created colors file: {}\n", colors_path.to_str().unwrap());
            }
            Err(e) => {
                output_text += &format!(
                    "Failed to create colors file: {} ({})\n",
                    colors_path.to_str().unwrap(),
                    e
                );
            }
        }
        match fs::write(&kb_path, kb::DEFAULT) {
            Ok(_) => {
                output_text += &format!("Created keybinds file: {}\n", kb_path.to_str().unwrap());
            }
            Err(e) => {
                output_text += &format!(
                    "Failed to create keybinds file: {} ({})\n",
                    kb_path.to_str().unwrap(),
                    e
                );
            }
        }
        match fs::write(&shell_cmds_path, shell_cmds::DEFAULT) {
            Ok(_) => {
                output_text += &format!(
                    "Created shell commands file: {}\n",
                    shell_cmds_path.to_str().unwrap()
                );
            }
            Err(e) => {
                output_text += &format!(
                    "Failed to create shell commands file: {} ({})\n",
                    shell_cmds_path.to_str().unwrap(),
                    e
                );
            }
        }
        app.set_output("Config Init", &output_text);
        output_window_show(app, vec![]);
    }

    pub fn config_clear(app: &mut App, _args: Vec<&str>) {
        let config_path = cfg::Config::get_path();
        let colors_path = cs::Colors::get_path();
        let kb_path = kb::get_path();
        let shell_cmds_path = shell_cmds::get_path();
        let mut output_text = String::new();
        match fs::remove_file(&config_path) {
            Ok(_) => {
                output_text += &format!("Removed config file: {}\n", config_path.to_str().unwrap());
            }
            Err(e) => {
                output_text += &format!(
                    "Failed to remove config file: {} ({})\n",
                    config_path.to_str().unwrap(),
                    e
                );
            }
        }
        match fs::remove_file(&colors_path) {
            Ok(_) => {
                output_text += &format!("Removed colors file: {}\n", colors_path.to_str().unwrap());
            }
            Err(e) => {
                output_text += &format!(
                    "Failed to remove colors file: {} ({})\n",
                    colors_path.to_str().unwrap(),
                    e
                );
            }
        }
        match fs::remove_file(&kb_path) {
            Ok(_) => {
                output_text += &format!("Removed keybinds file: {}\n", kb_path.to_str().unwrap());
            }
            Err(e) => {
                output_text += &format!(
                    "Failed to remove keybinds file: {} ({})\n",
                    kb_path.to_str().unwrap(),
                    e
                );
            }
        }
        match fs::remove_file(&shell_cmds_path) {
            Ok(_) => {
                output_text += &format!(
                    "Removed shell commands file: {}\n",
                    shell_cmds_path.to_str().unwrap()
                );
            }
            Err(e) => {
                output_text += &format!(
                    "Failed to remove shell commands file: {} ({})\n",
                    shell_cmds_path.to_str().unwrap(),
                    e
                );
            }
        }
        app.set_output("Config Clear", &output_text);
        output_window_show(app, vec![]);
    }

    pub fn dbg_clear_preview(app: &mut App, _args: Vec<&str>) {
        app.term_clear = true;
    }
}

// Command metadata
mod cmd_data {
    use std::collections::HashMap;

    use crate::{App, cmd, cmd_data};

    #[derive(Hash, Eq, PartialEq, Debug, Clone, PartialOrd, Ord)]
    pub enum CmdName {
        Exit,
        Home,
        CurUp,
        CurDown,
        DirUp,
        DirBack,
        DirReload,
        Explode,
        Enter,
        CmdWinToggle,
        CmdFinderToggle,
        CmdList,
        OutputWinToggle,
        OutputWinShow,
        OutputWinHide,
        Sel,
        SelClear,
        SelShow,
        SelSave,
        SelCopy,
        SelDelete,
        SelMove,
        SelClipPath,
        MenuBack,
        Log,
        LogClear,
        SecUp,
        SecDown,
        KeybindsShow,
        DbgClear,
        Edit,
        GoTo,
        InputClear,
        ShellQuick,
        ShellFull,
        ConfigInit,
        ConfigClear,
    }

    #[derive(Debug, Clone)]
    pub struct CmdData {
        pub fname: &'static str,
        pub description: &'static str,
        pub cmd: &'static str,
        pub vis_hidden: bool, // Hidden from visual cmd selection
        pub params: Vec<&'static str>,
        pub op: fn(&mut App, Vec<&str>) -> (),
    }
    pub type CmdList = HashMap<CmdName, CmdData>;
    pub fn cmd_name_from_str(
        cmd_list: &HashMap<CmdName, cmd_data::CmdData>,
        cmd: &str,
    ) -> Option<cmd_data::CmdName> {
        for (name, data) in cmd_list.iter() {
            if data.cmd == cmd {
                return Some(name.clone());
            }
        }
        None
    }
    pub fn get_cmd_data(
        cmd_list: &cmd_data::CmdList,
        name: &cmd_data::CmdName,
    ) -> cmd_data::CmdData {
        match cmd_list.get(name) {
            Some(data) => data.clone(),
            None => panic!("Command not found: {:?}", name),
        }
    }

    // Helper to get command string from CmdName
    pub fn get_cmd(cmd_list: &cmd_data::CmdList, name: &cmd_data::CmdName) -> String {
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
                params: vec![],
                op: cmd::exit,
            },
        );
        map.insert(
            CmdName::Home,
            CmdData {
                fname: "Home",
                description: "Go to your home directory",
                cmd: "home",
                vis_hidden: false,
                params: vec![],
                op: cmd::home,
            },
        );
        map.insert(
            CmdName::CurUp,
            CmdData {
                fname: "Cursor Up",
                description: "Move selection cursor up",
                cmd: "cur-up",
                vis_hidden: true,
                params: vec![],
                op: cmd::cur_up,
            },
        );
        map.insert(
            CmdName::CurDown,
            CmdData {
                fname: "Cursor Down",
                description: "Move selection cursor down",
                cmd: "cur-down",
                vis_hidden: true,
                params: vec![],
                op: cmd::cur_down,
            },
        );
        map.insert(
            CmdName::DirUp,
            CmdData {
                fname: "Directory Up (cd ..)",
                description: "Go up to the parent directory",
                cmd: "dir-up",
                vis_hidden: false,
                params: vec![],
                op: cmd::dir_up,
            },
        );
        map.insert(
            CmdName::DirBack,
            CmdData {
                fname: "Directory Back (cd -)",
                description: "Go back to the most recent working directory",
                cmd: "dir-back",
                vis_hidden: false,
                params: vec![],
                op: cmd::dir_back,
            },
        );
        map.insert(
            CmdName::DirReload,
            CmdData {
                fname: "Directory Reload",
                description: "Reload the current working directory",
                cmd: "dir-reload",
                vis_hidden: false,
                params: vec![],
                op: cmd::dir_reload,
            },
        );
        map.insert(
            CmdName::Explode,
            CmdData {
                fname: "Explode Mode Toggle",
                description: "Find all files in subdirectories under the current directory",
                cmd: "explode",
                vis_hidden: false,
                params: vec![],
                op: cmd::explode,
            },
        );
        map.insert(
            CmdName::Enter,
            CmdData {
                fname: "Enter",
                description: "Open/Edit/Run the item under the cursor",
                cmd: "enter",
                vis_hidden: true,
                params: vec![],
                op: cmd::enter,
            },
        );
        map.insert(
            CmdName::CmdWinToggle,
            CmdData {
                fname: "Command Window Toggle",
                description: "Toggle command window where you can type commands",
                cmd: "cmd-win",
                vis_hidden: false,
                params: vec![],
                op: cmd::cmd_window_toggle,
            },
        );
        map.insert(
            CmdName::CmdFinderToggle,
            CmdData {
                fname: "Command Finder Toggle",
                description: "Toggle the fuzzy command finder",
                cmd: "cmd-find",
                vis_hidden: false,
                params: vec![],
                op: cmd::cmd_finder_toggle,
            },
        );
        map.insert(
            CmdName::CmdList,
            CmdData {
                fname: "Command List",
                description: "List all commands in the output window",
                cmd: "cmd-list",
                vis_hidden: false,
                params: vec![],
                op: cmd::cmd_list,
            },
        );
        map.insert(
            CmdName::OutputWinToggle,
            CmdData {
                fname: "Output Window Toggle",
                description: "Toggle the output window",
                cmd: "output-toggle",
                vis_hidden: false,
                params: vec![],
                op: cmd::output_window_toggle,
            },
        );
        map.insert(
            CmdName::OutputWinShow,
            CmdData {
                fname: "Output Window Show",
                description: "Show the output window",
                cmd: "output-show",
                vis_hidden: false,
                params: vec![],
                op: cmd::output_window_show,
            },
        );
        map.insert(
            CmdName::OutputWinHide,
            CmdData {
                fname: "Output Window Hide",
                description: "Hide the output window",
                cmd: "output-hide",
                vis_hidden: false,
                params: vec![],
                op: cmd::output_window_hide,
            },
        );
        map.insert(
            CmdName::Sel,
            CmdData {
                fname: "Select Item Toggle",
                description: "Toggle selection of the item under the cursor",
                cmd: "sel",
                vis_hidden: false,
                params: vec![],
                op: cmd::sel,
            },
        );
        map.insert(
            CmdName::SelClear,
            CmdData {
                fname: "Selection Clear",
                description: "Clear the current selection of files and directories",
                cmd: "sel-clear",
                vis_hidden: false,
                params: vec![],
                op: cmd::sel_clear,
            },
        );
        map.insert(
            CmdName::SelShow,
            CmdData {
                fname: "Selection Show",
                description: "Show the current selection of files and directories in the output window",
                cmd: "sel-show",
                vis_hidden: false,
                params: vec![],
                op: cmd::sel_show,
            },
        );
        map.insert(
            CmdName::SelSave,
            CmdData {
                fname: "Selection Save",
                description: "Save the current selection of files and directories to file",
                cmd: "sel-save",
                vis_hidden: false,
                params: vec![],
                op: cmd::sel_save,
            },
        );
        map.insert(
            CmdName::SelCopy,
            CmdData {
                fname: "Selection Copy",
                description: "Copy the current selection of files and directories to the current directory",
                cmd: "sel-copy",
                vis_hidden: false,
                params: vec![],
                op: cmd::sel_copy,
            },
        );
        map.insert(
            CmdName::SelDelete,
            CmdData {
                fname: "Selection Delete",
                description: "Delete all currently selected files and directories",
                cmd: "sel-delete",
                vis_hidden: false,
                params: vec![],
                op: cmd::sel_delete,
            },
        );
        map.insert(
            CmdName::SelMove,
            CmdData {
                fname: "Selection Move",
                description: "Move (not copy) the currently selected files and directories to the current directory",
                cmd: "sel-move",
                vis_hidden: false,
                params: vec![],
                op: cmd::sel_move,
            },
        );
        map.insert(
            CmdName::SelClipPath,
            CmdData {
                fname: "Selection Copy Paths to Clipboard",
                description: "Copy the current selection of file and diretory paths to clipboard",
                cmd: "sel-clip",
                vis_hidden: false,
                params: vec![],
                op: cmd::sel_clip_path,
            },
        );
        map.insert(
            CmdName::MenuBack,
            CmdData {
                fname: "Menu Back",
                description: "Go back to previous menu",
                cmd: "menu-back",
                vis_hidden: true,
                params: vec![],
                op: cmd::menu_back,
            },
        );
        map.insert(
            CmdName::Log,
            CmdData {
                fname: "Low Viewer",
                description: "Show the application log",
                cmd: "log",
                vis_hidden: false,
                params: vec![],
                op: cmd::log_show,
            },
        );
        map.insert(
            CmdName::LogClear,
            CmdData {
                fname: "Log Clear",
                description: "Clear the application log",
                cmd: "log-clear",
                vis_hidden: false,
                params: vec![],
                op: cmd::log_clear,
            },
        );
        map.insert(
            CmdName::SecUp,
            CmdData {
                fname: "Secondary Window Scroll Up",
                description: "Scroll the secondary window up",
                cmd: "sec-up",
                vis_hidden: true,
                params: vec![],
                op: cmd::sec_up,
            },
        );
        map.insert(
            CmdName::SecDown,
            CmdData {
                fname: "Secondary Window Scroll Down",
                description: "Scroll the secondary window down",
                cmd: "sec-down",
                vis_hidden: true,
                params: vec![],
                op: cmd::sec_down,
            },
        );
        map.insert(
            CmdName::KeybindsShow,
            CmdData {
                fname: "Show Keybinds",
                description: "Show the currently loaded keybindings",
                cmd: "keybinds-show",
                vis_hidden: false,
                params: vec![],
                op: cmd::show_keybinds,
            },
        );
        map.insert(
            CmdName::DbgClear,
            CmdData {
                fname: "Debug Clear",
                description: "Clear the screen content. Some terminals may not refresh properly causing artifacts.",
                cmd: "dbg-prev-clear",
                vis_hidden: false,
                params: vec![],
                op: cmd::dbg_clear_preview,
            },
        );
        map.insert(
            CmdName::Edit,
            CmdData {
                fname: "Edit File",
                description: "Open the focused file or directory in $EDITOR",
                cmd: "edit",
                vis_hidden: false,
                params: vec![],
                op: cmd::edit,
            },
        );
        map.insert(
            CmdName::GoTo,
            CmdData {
                fname: "Go To Directory",
                description: "Go to a specified directory",
                cmd: "goto",
                vis_hidden: false,
                params: vec!["path"],
                op: cmd::goto,
            },
        );
        map.insert(
            CmdName::InputClear,
            CmdData {
                fname: "Clear Input",
                description: "Clear the current input/search",
                cmd: "input-clear",
                vis_hidden: false,
                params: vec![],
                op: cmd::input_clear,
            },
        );
        map.insert(
            CmdName::ShellQuick,
            CmdData {
                fname: "Shell Quick",
                description: "Run a quick shell command in the current directory",
                cmd: "shell",
                vis_hidden: false,
                params: vec!["command"],
                op: cmd::shell_quick,
            },
        );
        map.insert(
            CmdName::ShellFull,
            CmdData {
                fname: "Shell Full",
                description: "Run a full shell in the current directory",
                cmd: "shell-full",
                vis_hidden: false,
                params: vec![],
                op: cmd::shell_full,
            },
        );
        map.insert(
            CmdName::ConfigInit,
            CmdData {
                fname: "Config Init",
                description: "Initialize the configuration files with defaults",
                cmd: "config-init",
                vis_hidden: false,
                params: vec![],
                op: cmd::config_init,
            },
        );
        map.insert(
            CmdName::ConfigClear,
            CmdData {
                fname: "Config Clear",
                description: "Clear (delete) the configuration files",
                cmd: "config-clear",
                vis_hidden: false,
                params: vec![],
                op: cmd::config_clear,
            },
        );

        map
    }
}

// Color scheme management
mod cs {
    use crate::log;
    use ratatui::style::Color;
    use std::fs;

    const FILE_NAME: &str = "colors.txt";
    pub const DEFAULT: &str = r#"
#
# Default color
#

name           default
search_border  green
preview_border yellow
listing_border blue
file           green
dir            blue
command        cyan
executable     lightred
shortcut       yellow
image          lightmagenta
header         lightblue
info           yellow
tip            green
warning        yellow
error          red
ok             green
hi             white
dim            gray
misc           white
"#;
    // Color scheme struct
    #[derive(Clone)]
    pub struct Colors {
        pub name: String,
        pub search_border: Color,
        pub preview_border: Color,
        pub listing_border: Color,
        pub file: Color,
        pub dir: Color,
        pub command: Color,
        pub executable: Color,
        pub shortcut: Color,
        pub image: Color,
        pub info: Color,
        pub tip: Color,
        pub warning: Color,
        pub error: Color,
        pub ok: Color,
        pub header: Color,
        pub hi: Color,
        pub dim: Color,
        pub misc: Color,
    }
    impl Colors {
        pub fn new() -> Self {
            Self {
                name: "default".to_string(),
                search_border: Color::White,
                preview_border: Color::White,
                listing_border: Color::White,
                file: Color::White,
                dir: Color::White,
                command: Color::White,
                executable: Color::White,
                shortcut: Color::White,
                image: Color::White,
                info: Color::White,
                tip: Color::White,
                warning: Color::White,
                error: Color::White,
                ok: Color::White,
                header: Color::White,
                hi: Color::White,
                dim: Color::White,
                misc: Color::White,
            }
        }
        pub fn from_str(s: &str) -> Color {
            match s.to_lowercase().as_str() {
                "black" => Color::Black,
                "red" => Color::Red,
                "green" => Color::Green,
                "yellow" => Color::Yellow,
                "blue" => Color::Blue,
                "magenta" => Color::Magenta,
                "cyan" => Color::Cyan,
                "gray" => Color::Gray,
                "darkgray" => Color::DarkGray,
                "lightred" => Color::LightRed,
                "lightgreen" => Color::LightGreen,
                "lightyellow" => Color::LightYellow,
                "lightblue" => Color::LightBlue,
                "lightmagenta" => Color::LightMagenta,
                "lightcyan" => Color::LightCyan,
                "white" => Color::White,
                _ => Color::White,
            }
        }

        pub fn get_path() -> std::path::PathBuf {
            let cs_path = dirs::config_dir()
                .unwrap_or(std::env::current_dir().unwrap())
                .join(crate::APP_NAME)
                .join(FILE_NAME);
            cs_path
        }

        pub fn make_list_auto() -> Colors {
            // Read colors.txt
            let colors = match fs::read_to_string(crate::APP_NAME.to_string() + FILE_NAME) {
                Ok(content) => content,
                Err(_) => {
                    log!("colors.txt not found, using default colorscheme");
                    return Colors::make_list(DEFAULT);
                }
            };
            Colors::make_list(&colors)
        }

        fn make_list(colors_str: &str) -> Colors {
            let mut colors = Colors::new();
            for line in colors_str.lines() {
                // Ignore comments
                if line.starts_with('#') || line.trim().is_empty() {
                    continue;
                }
                // Trim whitespace
                let line = line.trim();
                let split = line.split_whitespace().collect::<Vec<&str>>();
                if split.len() != 2 {
                    continue;
                }
                let key = split[0];
                let value = split[1];
                match key {
                    "name" => colors.name = value.to_string(),
                    "search_border" => {
                        colors.search_border = Colors::from_str(value);
                    }
                    "preview_border" => {
                        colors.preview_border = Colors::from_str(value);
                    }
                    "listing_border" => {
                        colors.listing_border = Colors::from_str(value);
                    }
                    "file" => {
                        colors.file = Colors::from_str(value);
                    }
                    "dir" => {
                        colors.dir = Colors::from_str(value);
                    }
                    "command" => {
                        colors.command = Colors::from_str(value);
                    }
                    "executable" => {
                        colors.executable = Colors::from_str(value);
                    }
                    "shortcut" => {
                        colors.shortcut = Colors::from_str(value);
                    }
                    "image" => {
                        colors.image = Colors::from_str(value);
                    }
                    "info" => {
                        colors.info = Colors::from_str(value);
                    }
                    "tip" => {
                        colors.tip = Colors::from_str(value);
                    }
                    "warning" => {
                        colors.warning = Colors::from_str(value);
                    }
                    "error" => {
                        colors.error = Colors::from_str(value);
                    }
                    "ok" => {
                        colors.ok = Colors::from_str(value);
                    }
                    "header" => {
                        colors.header = Colors::from_str(value);
                    }
                    "hi" => {
                        colors.hi = Colors::from_str(value);
                    }
                    "dim" => {
                        colors.dim = Colors::from_str(value);
                    }
                    "misc" => {
                        colors.misc = Colors::from_str(value);
                    }
                    _ => {}
                }
            }
            colors
        }
    }
}

// Keybinding management
mod kb {
    use super::cmd_data;
    use super::cmd_data::CmdName;
    use crate::{APP_NAME, log};
    use crossterm::event::{KeyCode, KeyModifiers};
    use std::{env, fs, path::PathBuf};

    const FILE_NAME: &str = "keybinds.txt";
    pub const DEFAULT: &str = r#"
#
# Default keybinds
#

exit        esc
exit        ctrl-q
home        alt-h
cur-up      up
cur-up      ctrl-k
cur-down    down
cur-down    ctrl-j
sel         tab
dir-up      ctrl-h
dir-back    ctrl-u
explode     ctrl-x
edit        ctrl-e
goto        ctrl-g
enter       enter
enter       ctrl-l
cmd-win     ctrl-w
cmd-find    ctrl-t 
cmd-list    ctrl-i
sec-up      alt-k
sec-down    alt-j
input-clear ctrl-z
shell       ctrl-s
"#;
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

    // Short string like "ctrl-a"
    pub fn to_string_short(kb: &KeyBind) -> String {
        let modifier = match kb.modifiers {
            KeyModifiers::ALT => "alt",
            KeyModifiers::CONTROL => "ctrl",
            KeyModifiers::SHIFT => "shift",
            KeyModifiers::NONE => "",
            _ => "UNKNOWN",
        };
        if modifier.is_empty() {
            return format!("{}", kb.code.to_string().to_lowercase());
        }
        return format!("{}-{}", modifier, kb.code.to_string().to_lowercase());
    }

    // Full string with command name
    pub fn to_string_full(cmd_list: &cmd_data::CmdList, kb: &KeyBind) -> String {
        return format!(
            "{:<12} {}\n",
            cmd_data::get_cmd(cmd_list, &kb.command),
            to_string_short(kb)
        );
    }

    pub fn find_by_cmd(keybinds: &KeyBindList, cmd: &cmd_data::CmdName) -> Option<KeyBind> {
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
            .join(FILE_NAME);
        kb_path
    }

    pub fn make_list_auto() -> KeyBindList {
        // Read keybinds.txt
        let keybinds = match fs::read_to_string(get_path()) {
            Ok(content) => content,
            Err(_) => {
                log!("{} not found, using default keybinds", FILE_NAME);
                return make_list(DEFAULT);
            }
        };
        make_list(&keybinds)
    }

    fn make_list(keybinds_str: &str) -> KeyBindList {
        let mut list = KeyBindList::new();
        let cmd_list = cmd_data::make_cmd_list();
        for line in keybinds_str.lines() {
            // Ignore comments
            if line.starts_with('#') || line.trim().is_empty() {
                continue;
            }
            // Trim whitespace
            let line = line.trim();
            let split = line.split_whitespace().collect::<Vec<&str>>();
            if split.len() != 2 {
                log!("Invalid line in {}: {}", FILE_NAME, line);
                continue;
            }
            let cmd = split[0];
            let combo = split[1];
            let (modifier, code) = if combo.contains('-') {
                let combo_split = combo.splitn(2, '-').collect::<Vec<&str>>();
                (combo_split[0], combo_split[1])
            } else {
                ("none", combo)
            };
            let cmd = match cmd_data::cmd_name_from_str(&cmd_list, cmd) {
                Some(name) => name,
                None => {
                    log!("Unknown command in {}: {}", FILE_NAME, cmd);
                    continue;
                }
            };
            let modifiers = match modifier.to_lowercase().as_str() {
                "ctrl" => KeyModifiers::CONTROL,
                "alt" => KeyModifiers::ALT,
                "shift" => KeyModifiers::SHIFT,
                "none" => KeyModifiers::NONE,
                _ => {
                    log!("Unknown modifier in {}: {}", FILE_NAME, modifier);
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
                    log!("Unknown key code in {}: {}", FILE_NAME, code);
                    continue;
                }
            };
            let keybind = KeyBind::new(modifiers, code, cmd);
            list.push(keybind);
        }
        list
    }
}

// General configuration management
mod cfg {
    use std::fs;

    const FILE_NAME: &str = "config.txt";
    pub const DEFAULT: &str = r#"
# Default configuration
cmd_on_enter     edit
# 0 = no limit
list_limit       0
force_sixel      false
max_image_width  80
responsive_break 100
"#;
    pub struct Config {
        pub cmd_on_enter: String,
        pub list_limit: i32,
        pub force_sixel: bool,
        pub max_image_width: u16,
        pub responsive_break: u16,
    }
    impl Config {
        pub fn new() -> Self {
            Self {
                cmd_on_enter: "edit".to_string(),
                list_limit: 100000,
                force_sixel: false,
                max_image_width: 80,
                responsive_break: 100,
            }
        }
        pub fn get_path() -> std::path::PathBuf {
            let cfg_path = dirs::config_dir()
                .unwrap_or(std::env::current_dir().unwrap())
                .join(crate::APP_NAME)
                .join(FILE_NAME);
            cfg_path
        }
        pub fn make_list_auto() -> Config {
            // Read config.txt
            let config = match fs::read_to_string(crate::APP_NAME.to_string() + FILE_NAME) {
                Ok(content) => content,
                Err(_) => {
                    crate::log!("{} not found, using default configuration", FILE_NAME);
                    return Config::make_list(DEFAULT);
                }
            };
            Config::make_list(&config)
        }
        fn make_list(cfg_str: &str) -> Config {
            let mut config = Config::new();
            for line in cfg_str.lines() {
                // Ignore comments
                if line.starts_with('#') || line.trim().is_empty() {
                    continue;
                }
                // Trim whitespace
                let line = line.trim();
                let split = line.split_whitespace().collect::<Vec<&str>>();
                if split.len() != 2 {
                    continue;
                }
                let key = split[0];
                let value = split[1];
                match key {
                    "cmd_on_enter" => config.cmd_on_enter = value.to_string(),
                    "list_limit" => {
                        if let Ok(limit) = value.parse::<i32>() {
                            config.list_limit = if limit == 0 { i32::MAX } else { limit };
                        }
                    }
                    "force_sixel" => {
                        if value.to_lowercase() == "true" {
                            config.force_sixel = true;
                        } else {
                            config.force_sixel = false;
                        }
                    }
                    "max_image_width" => {
                        if let Ok(width) = value.parse::<u16>() {
                            config.max_image_width = width;
                        }
                    }
                    "responsive_break" => {
                        if let Ok(breakpoint) = value.parse::<u16>() {
                            config.responsive_break = breakpoint;
                        }
                    }
                    _ => {}
                }
            }
            config
        }
    }
}

// Shell command management
mod shell_cmds {
    use crate::cmd_data;

    const FILE_NAME: &str = "shell_cmds.txt";
    pub const DEFAULT: &str = r#"
#
# Default shell commands
#

# A simple command
ls -la
# Ask for input (open prompt window)
ls {ASK}
# Echo the first selected file/directory
echo $1
# Create a zip archive of all selected files/directories
zip -r archive.zip $...
"#;

    pub fn get_path() -> std::path::PathBuf {
        let shell_cmds_path = dirs::config_dir()
            .unwrap_or(std::env::current_dir().unwrap())
            .join(crate::APP_NAME)
            .join(FILE_NAME);
        shell_cmds_path
    }

    pub fn make_list_auto() -> Vec<String> {
        // Read shell_cmds.txt
        let shell_cmds = match std::fs::read_to_string(crate::APP_NAME.to_string() + FILE_NAME) {
            Ok(content) => content,
            Err(_) => {
                crate::log!("shell_cmds.txt not found, using default shell commands");
                return make_list(DEFAULT);
            }
        };
        make_list(&shell_cmds)
    }

    pub fn make_list(shell_str: &str) -> Vec<String> {
        let shell_cmd_name =
            cmd_data::get_cmd(&cmd_data::make_cmd_list(), &cmd_data::CmdName::ShellQuick);
        let mut list = Vec::new();
        for line in shell_str.lines() {
            // Ignore comments
            if line.starts_with('#') || line.trim().is_empty() {
                continue;
            }
            // Trim whitespace
            let line = line.trim();
            list.push(format!("{} {}", shell_cmd_name, line));
        }
        list
    }
}

mod util {
    use std::path::PathBuf;

    pub fn cls() {
        println!("\x1B[2J\x1B[1;1H");
    }

    pub fn fpath(path: &PathBuf) -> String {
        let pstring = path.to_str().unwrap().to_string();
        let home = dirs::home_dir().unwrap();
        let home_str = home.to_str().unwrap();
        if pstring.starts_with(home_str) {
            return pstring.replacen(home_str, "~", 1);
        }
        pstring
    }
}

// Each item in the listing is a "Node"
// NodeInfo holds information each node
mod node_info {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    use mime_guess::mime;

    use crate::log;

    #[derive(Clone, PartialEq)]
    pub enum NodeType {
        File,         // A regular file
        Directory,    // A directory
        Shortcut,     // Internal shortcut
        Command,      // Internal command, name should always be the command string
        ShellCommand, // A shell command
        Executable,   // An executable file
        Image,        // An image file
        Symlink,      // A symbolic link
        Unknown,      // Unknown, unsupported, etc.
    }
    impl NodeType {
        pub fn find(path: &Path, metadata: fs::Metadata) -> NodeType {
            if metadata.is_dir() {
                NodeType::Directory
            } else if metadata.is_file() {
                #[cfg(unix)]
                {
                    if metadata.permissions().mode() & 0o111 != 0 {
                        return NodeType::Executable;
                    }
                }
                // Check if image
                // // SLOW
                let file_type = mime_guess::from_path(path).first_or_octet_stream();
                if file_type.type_() == mime::IMAGE {
                    log!("Detected image mime type: {}", file_type.essence_str());
                    return NodeType::Image;
                }

                NodeType::File
            } else if metadata.file_type().is_symlink() {
                NodeType::Symlink
            } else {
                NodeType::Unknown
            }
        }
    }

    // Information about a file or directory
    #[derive(Clone)]
    pub struct NodeInfo {
        pub name: String,
        pub node_type: NodeType,
    }
    impl NodeInfo {
        pub fn new() -> Self {
            Self {
                name: String::new(),
                node_type: NodeType::Unknown,
            }
        }
        // UNUSED
        // pub fn is(&self, _is: NodeType) -> bool {
        //     return self.node_type == _is;
        // }
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
        pub fn is_shell_command(&self) -> bool {
            return self.node_type == NodeType::ShellCommand;
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

// Return type for loop control
// TODO: Im still suspicious of this design
// I think all we need is a way to break
enum LoopReturn {
    Continue,
    Break,
    Ok,
}

// Main application state and control methods
struct App<'a> {
    should_quit: bool,
    input: String,
    listing: Vec<NodeInfo>,
    results: Vec<NodeInfo>,
    focused: NodeInfo,
    focus_index: i32,
    multi_selection: Vec<PathBuf>,
    preview_content: Text<'a>,
    preview_image: Option<StatefulProtocol>,
    scroll_off_preview: u16,
    scroll_off_output: u16,
    cwd: PathBuf,
    lwd: PathBuf,
    mode_explode: bool,
    mode_cmd_finder: bool,
    show_command_window: bool,
    command_input: String,
    term_clear: bool, // When true the terminal will be cleared on next draw
    show_output_window: bool,
    output_title: String,
    output_text: String,
    show_yesno_window: bool,
    yesno_text: String,
    yesno_result: i32, // 0 = no, 1 = yes, 2 = unset
    cmd_list: cmd_data::CmdList,
    shell_cmd_list: Vec<String>,
    keybinds: kb::KeyBindList,
    cs: cs::Colors,
    cfg: cfg::Config,
    // Found config files
    found_keybinds: bool,
    found_cs: bool,
    found_cfg: bool,
    found_shell_cmds: bool,
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
        // There is no reason to read the whole file
        let kb_check = match fs::read_to_string(&kb::get_path()) {
            Ok(_) => true,
            Err(_) => false,
        };
        let cs_check = match fs::read_to_string(&cs::Colors::get_path()) {
            Ok(_) => true,
            Err(_) => false,
        };
        let cfg_check = match fs::read_to_string(&cfg::Config::get_path()) {
            Ok(_) => true,
            Err(_) => false,
        };
        let shell_cmds_ehck = match fs::read_to_string(&shell_cmds::get_path()) {
            Ok(_) => true,
            Err(_) => false,
        };

        Self {
            should_quit: false,
            input: String::new(),
            listing: Vec::new(),
            results: Vec::new(),
            focused: NodeInfo::new(),
            focus_index: 0,
            multi_selection: Vec::new(),
            preview_content: Default::default(),
            preview_image: None,
            scroll_off_preview: 0,
            scroll_off_output: 0,
            cwd: env::current_dir().unwrap(),
            lwd: env::current_dir().unwrap(),
            mode_explode: false,
            mode_cmd_finder: false,
            show_command_window: false,
            command_input: String::new(),
            term_clear: true, // Always clear on start
            show_output_window: false,
            output_title: String::new(),
            output_text: String::new(),
            show_yesno_window: false,
            yesno_text: String::new(),
            yesno_result: 2,
            cmd_list: cmd_data::make_cmd_list(),
            shell_cmd_list: shell_cmds::make_list_auto(),
            keybinds: kb::make_list_auto(),
            cs: cs::Colors::make_list_auto(),
            cfg: cfg::Config::make_list_auto(),
            found_keybinds: kb_check,
            found_cs: cs_check,
            found_cfg: cfg_check,
            found_shell_cmds: shell_cmds_ehck,
            has_bat: bat_check,
            lay_preview_area: Rect::default(),
        }
    }

    fn append_cwd(&mut self, path: &PathBuf) {
        log!("Changing directory to: {}", path.to_str().unwrap());
        let new_path = if path.to_str().unwrap() == ".." {
            self.cwd.parent().unwrap_or(&self.cwd).to_path_buf()
        } else {
            if path.is_absolute() {
                path.clone()
            } else if path.starts_with("~") {
                let mut home_path = dirs::home_dir().unwrap();
                let rel_path = path.strip_prefix("~").unwrap();
                home_path.push(rel_path);
                home_path
            } else {
                let mut temp_path = self.cwd.clone();
                temp_path.push(path);
                temp_path
            }
        };
        self.lwd = self.cwd.clone();
        self.cwd = new_path;
    }

    fn get_directory_listing(&self, path: &PathBuf) -> Vec<NodeInfo> {
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
                                let node_type = NodeType::find(&entry.path(), metadata.clone());
                                if self.mode_explode {
                                    let sub_path = entry.path();
                                    if metadata.is_dir() {
                                        // Recursively collect files from subdirectory
                                        let sub_entries = self.get_directory_listing(&sub_path);
                                        entries.extend(sub_entries);
                                    } else {
                                        entries.push(NodeInfo {
                                            name: sub_path.to_str().unwrap().to_string(),
                                            node_type,
                                        });
                                    }
                                } else {
                                    entries.push(NodeInfo {
                                        name: file_name_str.to_string(),
                                        node_type,
                                    });
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
    fn get_cmd(&self, name: &cmd_data::CmdName) -> String {
        cmd_data::get_cmd(&self.cmd_list, name)
    }

    fn replace_shell_vars(&self, mut shell_cmd: String) -> String {
        let mut path_all = String::new();
        for (i, path) in self.multi_selection.iter().enumerate() {
            let var_name = format!("${}", i + 1);
            let path_str = path.to_str().unwrap();
            shell_cmd = shell_cmd.replace(&var_name, path_str);
            path_all += &format!("{} ", path_str);
        }
        shell_cmd.replace("$...", &path_all.trim_end())
    }

    fn fmtln_info(&self, label: &str, value: &str) -> Line<'a> {
        Line::styled(
            format!("{} {:<12}: {}", nf::INFO, label, value),
            Style::default().fg(self.cs.info),
        )
    }

    fn fmtln_path(&self, path: &PathBuf) -> Line<'a> {
        Line::styled(
            format!("{} {}", nf::DIRO, path.to_str().unwrap()),
            Style::default().fg(self.cs.dir),
        )
    }

    fn fmtln_sc(&self, description: &str) -> Line<'a> {
        Line::styled(
            format!("{} {}", nf::CMD, description),
            Style::default().fg(self.cs.command),
        )
    }

    fn welcome_message(&mut self) {
        self.preview_content += self.fmtln_sc("Exit the application");
        self.preview_content += Line::from("");
        for (i, line) in LOGO.lines().enumerate() {
            if i == 0 {
                continue;
            };
            self.preview_content +=
                Line::styled(format!("{}", line), Style::default().fg(self.cs.tip));
        }
        let kb_exit = kb::find_by_cmd(&self.keybinds, &cmd_data::CmdName::Exit).unwrap();
        let kb_exit_str = kb::to_string_short(&kb_exit);
        self.preview_content += Line::from("");
        self.preview_content += Line::styled("Tips:", Style::default().fg(self.cs.header));
        self.preview_content += Line::styled(
            format!("└ Press {} to exit", kb_exit_str),
            Style::default().fg(self.cs.tip),
        );
        self.preview_content += Line::styled(
            "└ Start typing to fuzzy find files and directories",
            Style::default().fg(self.cs.tip),
        );
        // System Information
        // TODO: This is not very dry
        self.preview_content += Line::from("");
        self.preview_content +=
            Line::styled("System Information:", Style::default().fg(self.cs.header));
        if self.has_bat {
            self.preview_content += Line::styled(
                format!(
                    "{} 'bat' - file previews will use 'bat' for syntax highlighting",
                    nf::CHECK
                ),
                Style::default().fg(self.cs.ok),
            );
        } else {
            self.preview_content += Line::styled(
                format!(
                    "{} 'bat' - file previews will use built-in syntax highlighting",
                    nf::WARN
                ),
                Style::default().fg(self.cs.warning),
            );
        }
        fn check_found_file(
            found: bool,
            name: &str,
            path_str: &str,
            cs: cs::Colors,
        ) -> Line<'static> {
            if found {
                log!("{} found at {}", name, path_str);
                Line::styled(
                    format!("{} {} - loaded from {}", nf::CHECK, name, path_str),
                    Style::default().fg(cs.ok),
                )
            } else {
                log!("{} not found at {}", name, path_str);
                Line::styled(
                    format!("{} {} - not found at {}", nf::WARN, name, path_str),
                    Style::default().fg(cs.warning),
                )
            }
        }
        self.preview_content += check_found_file(
            self.found_keybinds,
            "keybinds",
            util::fpath(&kb::get_path()).as_ref(),
            self.cs.clone(),
        );
        self.preview_content += check_found_file(
            self.found_cs,
            "colors",
            util::fpath(&cs::Colors::get_path()).as_ref(),
            self.cs.clone(),
        );
        self.preview_content += check_found_file(
            self.found_cfg,
            "config",
            util::fpath(&cfg::Config::get_path()).as_ref(),
            self.cs.clone(),
        );
        self.preview_content += check_found_file(
            self.found_shell_cmds,
            "shell commands",
            util::fpath(&shell_cmds::get_path()).as_ref(),
            self.cs.clone(),
        );
        self.preview_content += Line::from("");
    }

    fn dir_list_pretty(&self, list: &Vec<NodeInfo>) -> Text<'a> {
        let mut text = Text::default();
        for item in list.iter().take(self.cfg.list_limit as usize) {
            // Check if this item is part of the multi selection
            let mut ms = "";
            let mut is_multi_selected = false;
            let mut cur_path = self.cwd.clone();
            cur_path.push(&item.name);
            for ms_item in self.multi_selection.iter() {
                if *ms_item == cur_path {
                    is_multi_selected = true;
                    break;
                }
            }
            let ms_on = format!("{} ", nf::MSEL);
            if is_multi_selected {
                ms = &ms_on;
            }
            let line = if item.is_shortcut() {
                Line::styled(
                    format!("{}{}| {}", ms, nf::CMD, item.name),
                    Style::default().fg(self.cs.shortcut),
                )
            } else if item.is_dir() {
                Line::styled(
                    format!("{}{}| {}/", ms, nf::DIR, item.name),
                    Style::default().fg(self.cs.dir),
                )
            } else if item.is_command() {
                Line::styled(
                    format!("{}{}| {}", ms, nf::CMD, item.name),
                    Style::default().fg(self.cs.command),
                )
            } else if item.is_shell_command() {
                Line::styled(
                    format!("{}{}| {}", ms, nf::CMD, item.name),
                    Style::default().fg(self.cs.executable),
                )
            } else if item.is_executable() {
                Line::styled(
                    format!("{}{}| {}", ms, nf::CMD, item.name),
                    Style::default().fg(self.cs.executable),
                )
            } else if item.is_image() {
                Line::styled(
                    format!("{}{}| {}", ms, nf::IMG, item.name),
                    Style::default().fg(self.cs.image),
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
                    Style::default().fg(self.cs.file),
                )
            };
            text.lines.push(Line::from(line));
        }
        text
    }

    fn preview_dir(&mut self, focused_path: &PathBuf) {
        let path_line = self.fmtln_path(&focused_path);
        self.preview_content += path_line;
        // Get the file metadata
        let metadata = fs::metadata(&focused_path);
        if let Ok(meta) = metadata {
            // Get permissions
            let permissions = meta.permissions();
            let perm_line = self.fmtln_info("permissions", &format!("{:o}", permissions.mode()));
            self.preview_content += perm_line;
        }
        let listing = self.get_directory_listing(&focused_path);
        let count_line = self.fmtln_info("count", &listing.len().to_string());
        self.preview_content += count_line;
        self.preview_content += Line::styled(SEP, Style::default().fg(self.cs.dim));
        let pretty_listing = self.dir_list_pretty(&listing);
        for line in pretty_listing.lines.iter().take(20) {
            self.preview_content += Line::from(line.clone());
        }
    }

    fn preview_file(&mut self, focused_path: &PathBuf) {
        let path_line = self.fmtln_path(&focused_path);
        self.preview_content += path_line;
        // Get the file metadata
        let metadata = fs::metadata(&focused_path);
        if let Ok(meta) = metadata {
            // Get permissions
            let permissions = meta.permissions();
            let perm_line = self.fmtln_info("permissions", &format!("{:o}", permissions.mode()));
            self.preview_content += perm_line;
            // Get mime type
            if meta.file_type().is_file() {
                // Get mimetype using mime_guess
                let mime = mime_guess::from_path(&focused_path).first_or_octet_stream();
                let mime_line = self.fmtln_info("mime", &mime.to_string());
                self.preview_content += mime_line;
            }
        }

        fn sanitize_content(input: &str) -> String {
            // Regex to match ANSI escape sequences
            let ansi_regex = Regex::new(r"\x1B\[[0-9;]*[A-Za-z]").unwrap();
            let mut result = String::new();
            let mut last = 0;

            for mat in ansi_regex.find_iter(input) {
                // Process text before the ANSI sequence
                let before = &input[last..mat.start()];
                for c in before.chars() {
                    if c.is_control() && c != '\n' && c != '\x1B' {
                        result.push('�');
                    } else {
                        result.push(c);
                    }
                }
                // Copy the ANSI sequence as-is
                result.push_str(mat.as_str());
                last = mat.end();
            }
            // Process any remaining text after the last ANSI sequence
            let after = &input[last..];
            for c in after.chars() {
                if c.is_control() && c != '\n' && c != '\x1B' {
                    result.push('�');
                } else {
                    result.push(c);
                }
            }
            result
        }
        // Check if bat is available
        // Use bat for preview if available
        if self.has_bat {
            // Use bat for preview
            log!("Using bat for file preview");
            if let Ok(bat_output) = Command::new("bat")
                .arg("--color=always")
                .arg("--style=plain")
                .arg(focused_path.to_str().unwrap())
                .output()
            {
                if bat_output.status.success() {
                    self.preview_content += Line::styled(SEP, Style::default().fg(self.cs.dim));
                    let mut bat_content = String::from_utf8_lossy(&bat_output.stdout);
                    bat_content = bat_content.replace("\r\n", "\n").into();
                    // Replace tabs with spaces
                    bat_content = bat_content.replace("\t", "    ").into();
                    // Replace non-printable characters
                    // NOTE: THIS SEEMS TO BE THE TRICK
                    bat_content = sanitize_content(&bat_content.to_string()).into();
                    let output = match bat_content.as_ref().into_text() {
                        Ok(text) => text,
                        Err(_) => {
                            self.preview_content += Line::styled(
                                "Error: Unable to convert bat output to text.",
                                Style::default().fg(self.cs.error),
                            );
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
        fn syntect_to_ratatui_color(s: SyntectStyle) -> Color {
            Color::Rgb(s.foreground.r, s.foreground.g, s.foreground.b)
        }
        let ss = SyntaxSet::load_defaults_newlines();
        // FIXME: Should only load once
        let ts = ThemeSet::load_defaults();
        let syntax = ss
            .find_syntax_for_file(&focused_path)
            .unwrap_or(None)
            .unwrap_or_else(|| ss.find_syntax_plain_text());
        let mut h = HighlightLines::new(syntax, &ts.themes["base16-eighties.dark"]);

        // Print syntax name
        self.preview_content += self.fmtln_info("detected", syntax.name.as_str());

        self.preview_content += Line::styled(SEP, Style::default().fg(self.cs.dim));

        if let Ok(content) = fs::read_to_string(&focused_path) {
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
            self.preview_content += Line::styled(
                "Err: Unable to read file content.",
                Style::default().fg(self.cs.error),
            );
        }
    }

    fn preview_image(&mut self, focused_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        self.preview_content = Default::default();
        let mut picker = Picker::from_fontsize((6, 12));
        if self.cfg.force_sixel {
            picker.set_protocol_type(ProtocolType::Sixel);
        } else {
            picker.set_protocol_type(ProtocolType::Halfblocks);
        }

        // Load an image with the image crate.
        let dyn_img = image::ImageReader::open(focused_path)?.decode()?;

        // Create the Protocol which will be used by the widget.
        let image = picker.new_resize_protocol(dyn_img);
        self.preview_image = Some(image);
        Ok(())
    }

    fn update_preview(&mut self) {
        log!("Updating preview for item: {}", self.focused.name);
        self.preview_content = Default::default();
        self.reset_sec_scroll();
        match self.focused.name.as_str() {
            sc::EXIT => {
                self.welcome_message();
            }
            sc::HOME => {
                self.preview_content += self.fmtln_path(&dirs::home_dir().unwrap());
                self.preview_content += self.fmtln_sc("Go to the home directory");
            }
            sc::DIR_UP => {
                let up_path = self.cwd.parent().unwrap_or(&self.cwd);
                self.preview_content += self.fmtln_path(&up_path.to_path_buf());
                self.preview_content += self.fmtln_sc("Go up to the parent directory");
            }
            sc::DIR_BACK => {
                self.preview_content += self.fmtln_path(&self.lwd);
                self.preview_content += self.fmtln_sc("Go back to the last working directory");
            }
            sc::EXP => {
                self.preview_content += self.fmtln_sc("Toggle explode mode");
                self.preview_content += Line::styled(
                    "Shows all files in subdirectories under the current directory.",
                    Style::default().fg(self.cs.tip),
                );
                let status = if self.mode_explode { "ON" } else { "OFF" };
                self.preview_content += self.fmtln_info("explode mode", status);
            }
            sc::CMDS => {
                self.preview_content += self.fmtln_sc("Show visual commands");
                self.preview_content += Line::styled(
                    "Toggles a visual command menu in the listing.",
                    Style::default().fg(self.cs.tip),
                );
            }
            sc::MENU_BACK => {
                self.preview_content += self.fmtln_sc("Go back to the previous menu");
                self.preview_content += Line::styled(
                    "Exits the current visual command menu.",
                    Style::default().fg(self.cs.tip),
                );
            }
            _ => {
                self.preview_content = Default::default();
                // Check if we have an internal command
                if self.focused.is_command() {
                    let cmd_name =
                        match cmd_data::cmd_name_from_str(&self.cmd_list, &self.focused.name) {
                            Some(name) => name,
                            None => {
                                self.preview_content += Line::styled(
                                    "Error: Command data not found.",
                                    Style::default().fg(self.cs.error),
                                );
                                return;
                            }
                        };
                    let data = cmd_data::get_cmd_data(&self.cmd_list, &cmd_name).clone();
                    self.preview_content += Line::styled(
                        format!("name: {}", data.fname),
                        Style::default().fg(self.cs.tip),
                    );
                    self.preview_content += Line::styled(
                        format!("cmd : {}", data.cmd),
                        Style::default().fg(self.cs.command),
                    );
                    self.preview_content += Line::styled(
                        format!("info: {}", data.description),
                        Style::default().fg(self.cs.info),
                    );
                    return;
                }
                // Check if we have a shell command
                if self.focused.is_shell_command() {
                    self.preview_content += Line::styled(
                        format!("shell: {}", self.focused.name),
                        Style::default().fg(self.cs.command),
                    );
                    let replaced = self.replace_shell_vars(self.focused.name.clone());
                    if replaced != self.focused.name {
                        self.preview_content += Line::styled(
                            format!("as: {}", replaced),
                            Style::default().fg(self.cs.info),
                        )
                    };
                    return;
                }
                // We have a file or dir
                let mut focused_path = self.cwd.clone();
                focused_path.push(&self.focused.name);

                if self.focused.is_dir() {
                    self.preview_dir(&focused_path);
                } else if self.focused.is_file() {
                    self.preview_file(&focused_path);
                } else if self.focused.is_image() {
                    self.preview_content += Line::styled(
                        "Image file preview not yet supported.",
                        Style::default().fg(self.cs.error),
                    );
                    let _ = self.preview_image(&focused_path);
                } else if self.focused.is_executable() {
                    self.preview_content += Line::styled(
                        "Executable file preview not yet supported.",
                        Style::default().fg(self.cs.error),
                    );
                } else if self.focused.is_shortcut() {
                    self.preview_content += Line::styled(
                        "Shortcut preview not supported.",
                        Style::default().fg(self.cs.error),
                    );
                } else if self.focused.is_unknown() {
                    self.preview_content +=
                        Line::styled("Unknown file type.", Style::default().fg(self.cs.error));
                } else {
                    // Unknown
                    self.preview_content += Line::styled(
                        "Error: Focused node type cant be detected",
                        Style::default().fg(self.cs.error),
                    );
                    // Debug info
                    let metadata = fs::metadata(&focused_path);
                    self.preview_content += Line::styled(
                        format!("{:?}", metadata),
                        Style::default().fg(self.cs.error),
                    );
                }
            }
        }
    }

    fn update_listing(&mut self) {
        // Handle cmd finder
        if self.mode_cmd_finder {
            log!("Updating command listing");
            self.listing.clear();
            // Shell commands
            for shell_cmd in self.shell_cmd_list.iter() {
                self.listing.push(NodeInfo {
                    name: shell_cmd.to_string(),
                    node_type: NodeType::ShellCommand,
                });
            }
            // Sort the commands alphabetically
            let mut entries: Vec<_> = self.cmd_list.iter().collect();
            entries.sort_by(|a, b| a.1.cmd.cmp(b.1.cmd));
            for (_, cmd_data) in entries {
                if cmd_data.vis_hidden {
                    continue;
                }
                self.listing.push(NodeInfo {
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
            NodeInfo {
                name: sc::DIR_BACK.to_string(),
                node_type: NodeType::Shortcut,
            },
        );
        listing.insert(
            0,
            NodeInfo {
                name: sc::DIR_UP.to_string(),
                node_type: NodeType::Shortcut,
            },
        );
        listing.insert(
            0,
            NodeInfo {
                name: sc::CMDS.to_string(),
                node_type: NodeType::Shortcut,
            },
        );
        listing.insert(
            0,
            NodeInfo {
                name: sc::EXP.to_string(),
                node_type: NodeType::Shortcut,
            },
        );
        listing.insert(
            0,
            NodeInfo {
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
            .take(self.cfg.list_limit as usize) // Limit for performance
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

    fn update_focused(&mut self) -> bool {
        let old = self.focused.clone();
        if self.focus_index < self.results.len() as i32 {
            self.focused = self.results[self.focus_index as usize].clone();
        } else if !self.results.is_empty() {
            self.focus_index = 0;
            self.focused = NodeInfo::new();
        } else {
            self.focus_index = 0;
            self.focused = NodeInfo::new();
        }
        // Remove icon prefix from selection
        // NOTE: This should be safe since file name should not contain pipe
        if let Some(pos) = self.focused.name.find("| ") {
            self.focused.name = self.focused.name[(pos + 2)..].to_string();
        }
        return old.name != self.focused.name;
    }

    fn input_out_window(&mut self, modifiers: KeyModifiers, code: KeyCode) {
        match (modifiers, code) {
            (KeyModifiers::NONE, KeyCode::Esc) => {
                cmd::output_window_hide(self, vec![]);
                return;
            }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                cmd::output_window_hide(self, vec![]);
                return;
            }
            _ => {}
        }
        // Special command matching just for output window
        let cmd = match (modifiers, code) {
            (KeyModifiers::ALT, KeyCode::Char('j')) => self.get_cmd(&cmd_data::CmdName::SecDown),
            (KeyModifiers::ALT, KeyCode::Char('k')) => self.get_cmd(&cmd_data::CmdName::SecUp),
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
                self.show_command_window = false;
                return self.handle_cmd(&cmd);
            }
            (KeyModifiers::NONE, KeyCode::Esc) => {
                self.show_command_window = false;
            }
            _ => {}
        }
        LoopReturn::Ok
    }

    // TODO: This doesnt really make sense because command cant wait for it
    fn input_yesno_window(&mut self, modifiers: KeyModifiers, code: KeyCode) {
        self.yesno_result = 2; // Reset
        match (modifiers, code) {
            (KeyModifiers::NONE, KeyCode::Char('y')) => {
                self.show_yesno_window = false;
                self.yesno_result = 1;
            }
            (KeyModifiers::NONE, KeyCode::Char('n')) => {
                self.show_yesno_window = false;
                self.yesno_result = 0;
            }
            _ => {}
        }
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
                let cmd_data = cmd_data::get_cmd_data(&self.cmd_list, &kb.command);
                if cmd_data.params.len() > 0 {
                    cmd += &format!(" {}", ASK); // Indicate that params are needed and should be asked for
                }
                break;
            }
        }
        if !cmd.is_empty() {
            log!("cmd from mapping: {}", &cmd);
        }
        cmd.to_string()
    }

    fn handle_cmd(&mut self, cmd: &str) -> LoopReturn {
        // Check if we have a command with args
        // If so just open command window to ask for args
        if cmd.contains(ASK) {
            self.command_input = cmd.replace(ASK, "").to_string();
            self.show_command_window = true;
            return LoopReturn::Continue;
        }

        let cmd_split = cmd.trim().split_whitespace().collect::<Vec<&str>>();
        let cmd = match cmd_split.first() {
            Some(c) => c.to_string(),
            None => cmd.to_string(),
        };
        let args = if cmd_split.len() > 1 {
            cmd_split[1..].to_vec()
        } else {
            Vec::new()
        };
        let cmd_name = match cmd_data::cmd_name_from_str(&self.cmd_list, &cmd) {
            Some(name) => name,
            None => {
                // If the command isnt empty print the incorrect command
                if !cmd.is_empty() {
                    log!("No command matched: {}", cmd);
                    self.set_output("Shell", &format!("No command matched: {}", cmd));
                    cmd::output_window_show(self, vec![]);
                }
                return LoopReturn::Ok;
            }
        };
        // TODO: This isnt handled correctly
        let cmd_data = cmd_data::get_cmd_data(&self.cmd_list, &cmd_name);
        (cmd_data.op)(self, args);
        LoopReturn::Ok
    }

    fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        log!("Starting main event loop");
        // Get directory listing
        self.append_cwd(&self.cwd.clone());
        self.update_listing();
        self.update_results(); // Initial results
        self.update_focused();
        self.update_preview();
        loop {
            if self.should_quit {
                terminal.clear()?;
                log!("Quitting main event loop");
                break;
            }
            if self.term_clear {
                terminal.clear()?;
                self.term_clear = false;
            }
            terminal.draw(|f| self.render(f))?;
            if event::poll(std::time::Duration::from_millis(100))? {
                if let Event::Key(KeyEvent {
                    code, modifiers, ..
                }) = event::read()?
                {
                    // Output window input handling
                    if self.show_output_window {
                        self.input_out_window(modifiers, code);
                        continue;
                    }
                    // Command window input handling
                    if self.show_command_window {
                        let lr = self.input_cmd_window(modifiers, code);
                        match lr {
                            LoopReturn::Continue => continue,
                            LoopReturn::Break => break,
                            LoopReturn::Ok => {}
                        }
                        continue;
                    }
                    // Yes/No window input handling
                    if self.show_yesno_window {
                        self.input_yesno_window(modifiers, code);
                        continue;
                    }
                    // Before key press handling
                    let input_changed = self.input_main(modifiers, code);
                    // Some things are not bindable
                    if input_changed {
                        self.update_results();
                    }
                    // Process key to command mselfing
                    let cmd = self.input_keybinds(modifiers, code);
                    // Handle commands
                    let lr = self.handle_cmd(&cmd);
                    match lr {
                        LoopReturn::Continue => continue,
                        LoopReturn::Break => break,
                        LoopReturn::Ok => {}
                    }
                    // After key press handling
                    let sel_changed = self.update_focused();
                    if sel_changed {
                        self.update_preview();
                    }
                }
            }
        }

        Ok(())
    }
    fn render(&mut self, frame: &mut Frame) {
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
        let threshold = self.cfg.responsive_break;

        // Reserve one line at the bottom for the status bar
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(1),    // Main UI
                Constraint::Length(1), // Status bar
            ])
            .split(area);

        let main_area = main_chunks[0];
        let status_area = main_chunks[1];

        // --- Widget creation ---
        // Input box
        let mut input_color;
        let input_str: String;
        if self.input.is_empty() {
            input_str = "Type to search...".to_string();
            input_color = self.cs.dim;
        } else {
            input_str = self.input.clone();
            input_color = self.cs.misc;
        };
        if self.results.is_empty() {
            input_color = self.cs.error;
        }
        let input_span: Span =
            Span::styled(format!("{}", input_str), Style::default().fg(input_color));
        let suffix: Span = Span::styled(
            format!("|{} ", nf::MAG),
            Style::default().fg(self.cs.search_border),
        );
        let mut input_line = Line::from(input_span);
        input_line.push_span(suffix);
        let input_widget = Paragraph::new(input_line).block(
            Block::default()
                .title(format!(
                    "( {} ) ) )  [ {} / {} ]",
                    APP_NAME.to_uppercase(),
                    self.results.len(),
                    self.listing.len(),
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(self.cs.search_border)),
        );

        // Results list
        let mut results_pretty = self.dir_list_pretty(&self.results);
        if let Some(line) = results_pretty.lines.get_mut(self.focus_index as usize) {
            let sel_span = Span::styled(
                format!("{}", nf::SEL),
                Style::default().fg(self.cs.hi).bg(Color::Black),
            );
            let line_span = Span::styled(format!(" {}", line), Style::default().fg(self.cs.hi));
            let mut new_line = Line::from(sel_span);
            new_line.push_span(line_span);
            *line = new_line;
        }
        let explode_str = if self.mode_explode {
            format!(" [{} exp]", nf::BOMB)
        } else {
            "".to_string()
        };
        let list_title = format!("|{}{} ", util::fpath(&self.cwd), explode_str);
        let list_widget = List::new(results_pretty).block(
            Block::default()
                .title(list_title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(self.cs.listing_border)),
        );
        let mut state = ListState::default();
        if !self.results.is_empty() && self.focus_index >= 0 {
            state.select(Some(self.focus_index as usize));
        }

        // Preview box
        let preview_widget = Paragraph::new(self.preview_content.clone())
            .block(
                Block::default()
                    .title(format!("{} m(0)_(0)m | {} ", nf::LOOK, self.focused.name))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(self.cs.preview_border)),
            )
            .wrap(Wrap { trim: false })
            .scroll((
                self.scroll_off_preview as u16,
                self.scroll_off_preview as u16,
            ));

        // --- Layout and rendering ---
        if main_area.width < threshold {
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
                .split(main_area);

            frame.render_widget(input_widget, vertical_chunks[0]);
            frame.render_stateful_widget(list_widget, vertical_chunks[1], &mut state);
            frame.render_widget(preview_widget, vertical_chunks[2]);
            self.lay_preview_area = vertical_chunks[2];
        } else {
            // Horizontal layout
            let horizontal_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(40), Constraint::Percentage(60)].as_ref())
                .split(main_area);

            let left_vertical_chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(0)
                .constraints([Constraint::Length(3), Constraint::Min(1)].as_ref())
                .split(horizontal_chunks[0]);

            frame.render_widget(input_widget, left_vertical_chunks[0]);
            frame.render_stateful_widget(list_widget, left_vertical_chunks[1], &mut state);
            frame.render_widget(preview_widget, horizontal_chunks[1]);
            self.lay_preview_area = horizontal_chunks[1];
        }

        // --- Status bar ---
        // FIXME: THESE SHOULD BE CACHED
        let username = env::var("USER").unwrap_or_else(|_| "unknown".to_string());
        let hostname = fs::read_to_string("/etc/hostname")
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        let whoami = format!("{}@{}", username, hostname);
        let unix_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let hhmmss = format!(
            "{:02}:{:02}:{:02}",
            (unix_time / 3600) % 24,
            (unix_time / 60) % 60,
            unix_time % 60
        );
        let multi_count = self.multi_selection.len();
        let status_text = format!(" {} | {} {} | {}", whoami, nf::MSEL, multi_count, hhmmss);
        let status_widget =
            Paragraph::new(status_text).style(Style::default().fg(self.cs.dim).bg(Color::Black));
        frame.render_widget(status_widget, status_area);

        // --- The image widget ---
        let image: StatefulImage<StatefulProtocol> = StatefulImage::default();
        match self.preview_image {
            Some(ref mut img) => {
                if !self.focused.is_image() {
                    return;
                }
                let mut new_area = Rect {
                    x: self.lay_preview_area.x + 1,
                    y: self.lay_preview_area.y + 1,
                    width: self.lay_preview_area.width - 2,
                    height: self.lay_preview_area.height - 2,
                };
                if new_area.width > self.cfg.max_image_width {
                    new_area.width = self.cfg.max_image_width;
                }
                frame.render_stateful_widget(image, new_area, img);
            }
            None => {}
        }

        // --- Popups ---
        if self.show_command_window {
            let popup_area = centered_rect(50, 10, area);
            let command_str = format!("> {}|", self.command_input);
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
        if self.show_output_window {
            let popup_area = centered_rect(50, 90, area);
            frame.render_widget(Clear, popup_area);
            let command_paragraph = Paragraph::new(self.output_text.clone())
                .style(Style::default().bg(Color::Black))
                .block(
                    Block::default()
                        .title(format!("{} {} ('esc' to exit)", nf::CMD, self.output_title))
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Magenta))
                        .style(Style::default().bg(Color::Black)),
                )
                .wrap(Wrap { trim: false })
                .scroll((self.scroll_off_output as u16, self.scroll_off_output as u16));
            frame.render_widget(command_paragraph, popup_area);
        }
        if self.show_yesno_window {
            let popup_area = centered_rect(30, 10, area);
            frame.render_widget(Clear, popup_area);
            let yesno_paragraph = Paragraph::new(self.yesno_text.clone())
                .style(Style::default().bg(Color::Black))
                .block(
                    Block::default()
                        .title(format!("{} Confirm (y/n)", nf::WARN))
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Red))
                        .style(Style::default().bg(Color::Black)),
                );
            frame.render_widget(yesno_paragraph, popup_area);
        }
    }
}

fn main() -> Result<()> {
    log!("======= Starting application =======");
    color_eyre::install()?;
    util::cls();
    enable_raw_mode()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(std::io::stdout()))?;

    let mut app = App::new();

    app.run(&mut terminal)?;

    disable_raw_mode()?;
    util::cls();
    println!("{} exited successfully.", APP_NAME);
    Ok(())
}
