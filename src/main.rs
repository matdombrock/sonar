use ansi_to_tui::IntoText;
use color_eyre::eyre::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    crossterm::terminal,
    layout::{Constraint, Direction, Layout, Rect},
    prelude::Backend,
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListState, Paragraph, Wrap},
};
use ratatui_image::{
    StatefulImage,
    picker::{Picker, ProtocolType},
    protocol::StatefulProtocol,
};
use regex::Regex;
use std::{
    env,
    fs::{self, File},
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    pin::Pin,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
use syntect::{
    easy::HighlightLines,
    highlighting::{Style as SyntectStyle, ThemeSet},
    parsing::SyntaxSet,
};

// INTERNAL MODULES
use crate::{node_info::NodeInfo, node_info::NodeType};

const APP_NAME: &str = "sonar";

// ANSI Shadow
const LOGO: &str = r#"
    ███████╗ ██████╗ ███╗   ██╗ █████╗ ██████╗ ██╗ ██╗ ██╗ 
    ██╔════╝██╔═══██╗████╗  ██║██╔══██╗██╔══██╗╚██╗╚██╗╚██╗
    ███████╗██║   ██║██╔██╗ ██║███████║██████╔╝ ██║ ██║ ██║
    ╚════██║██║   ██║██║╚██╗██║██╔══██║██╔══██╗ ██║ ██║ ██║
    ███████║╚██████╔╝██║ ╚████║██║  ██║██║  ██║██╔╝██╔╝██╔╝
    ╚══════╝ ╚═════╝ ╚═╝  ╚═══╝╚═╝  ╚═╝╚═╝  ╚═╝╚═╝ ╚═╝ ╚═╝ 
"#;

const LOADING: &str = r#"
    ██╗      ██████╗  █████╗ ██████╗ ██╗███╗   ██╗ ██████╗ 
    ██║     ██╔═══██╗██╔══██╗██╔══██╗██║████╗  ██║██╔════╝ 
    ██║     ██║   ██║███████║██║  ██║██║██╔██╗ ██║██║  ███╗
    ██║     ██║   ██║██╔══██║██║  ██║██║██║╚██╗██║██║   ██║
    ███████╗╚██████╔╝██║  ██║██████╔╝██║██║ ╚████║╚██████╔╝
    ╚══════╝ ╚═════╝ ╚═╝  ╚═╝╚═════╝ ╚═╝╚═╝  ╚═══╝ ╚═════╝ 
"#;

const E404: &str = r#"
        ██╗  ██╗ ██████╗ ██╗  ██╗    ██╗    
        ██║  ██║██╔═████╗██║  ██║██╗██╔╝    
        ███████║██║██╔██║███████║╚═╝██║     
        ╚════██║████╔╝██║╚════██║██╗██║     
             ██║╚██████╔╝     ██║╚═╝╚██╗    
             ╚═╝ ╚═════╝      ╚═╝    ╚═╝    
     ██╗██╗  ██╗██╗          ██╗██╗  ██╗██╗ 
    ██╔╝╚██╗██╔╝╚██╗        ██╔╝╚██╗██╔╝╚██╗
    ██║  ╚███╔╝  ██║        ██║  ╚███╔╝  ██║
    ██║  ██╔██╗  ██║        ██║  ██╔██╗  ██║
    ╚██╗██╔╝ ██╗██╔╝███████╗╚██╗██╔╝ ██╗██╔╝
     ╚═╝╚═╝  ╚═╝╚═╝ ╚══════╝ ╚═╝╚═╝  ╚═╝╚═╝ 
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
    pub const SCMD: &str = "";
    pub const INFO: &str = "";
    pub const CHECK: &str = "";
    pub const WARN: &str = "";
    pub const BOMB: &str = "";
    pub const DUDE: &str = "󰢚";
    pub const WAIT: &str = "󱑆";
    pub const EYEN: &str = "󰈉";
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
    pub const LOADING: &str = "󱑆 loading...";
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

mod node_meta {
    use std::os::unix::fs::PermissionsExt;
    use std::{path::PathBuf, time::SystemTime};

    #[derive(Clone, Debug)]
    pub struct NodeMeta {
        pub size: u64,
        pub modified: u32,
        pub permissions: u32,
        pub mime: String,
        pub path: PathBuf,
    }
    impl NodeMeta {
        pub fn empty() -> Self {
            NodeMeta {
                size: 0,
                modified: 0,
                permissions: 0,
                mime: "unknown".to_string(),
                path: PathBuf::new(),
            }
        }
        pub fn get(path: &PathBuf) -> Self {
            match std::fs::metadata(&path) {
                Ok(metadata) => {
                    let size = metadata.len();
                    let modified = metadata
                        .modified()
                        .ok()
                        .and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
                        .map(|duration| duration.as_secs() as u32)
                        .unwrap_or(0);
                    let permissions = metadata.permissions().mode();
                    let mime = if metadata.is_file() {
                        mime_guess::from_path(&path)
                            .first_raw()
                            .unwrap_or("application/octet-stream")
                            .to_string()
                    } else {
                        "inode/directory".to_string()
                    };
                    let full_path = path.clone();
                    NodeMeta {
                        size,
                        modified,
                        permissions,
                        mime,
                        path: full_path,
                    }
                }
                Err(_) => NodeMeta {
                    size: 0,
                    modified: 0,
                    permissions: 0,
                    mime: "unknown".to_string(),
                    path: path.clone(),
                },
            }
        }
    }
}

// Async queue
mod aq {

    use ratatui::text::Text;
    use ratatui_image::protocol::StatefulProtocol;
    use tokio::task::JoinHandle;

    use crate::{node_info::NodeInfo, node_meta::NodeMeta};

    #[derive(PartialEq, Debug)]
    pub enum Kind {
        ListingResult,
        ListingDir,
        ListingPreview,
        ImagePreview,
        FilePreview,
        FsOperation,
    }
    // Holds the data that the async fns can return
    // TODO: This is a little messy
    // Might want to make this more generic later
    pub struct ResData {
        pub rc: u32,
        pub data_str: Option<String>,
        pub data_listing: Option<Vec<NodeInfo>>,
        pub data_image: Option<StatefulProtocol>,
        pub data_meta: Option<NodeMeta>,
        pub data_file: Option<Text<'static>>,
    }
    impl ResData {
        pub fn as_str(rc: u32, data: String) -> Self {
            ResData {
                rc,
                data_str: Some(data),
                data_listing: None,
                data_image: None,
                data_meta: None,
                data_file: None,
            }
        }
        pub fn as_listing(rc: u32, data: Vec<NodeInfo>, meta: NodeMeta) -> Self {
            ResData {
                rc,
                data_str: None,
                data_listing: Some(data),
                data_image: None,
                data_meta: Some(meta),
                data_file: None,
            }
        }
        pub fn as_image(rc: u32, data: StatefulProtocol, meta: NodeMeta) -> Self {
            ResData {
                rc,
                data_str: None,
                data_listing: None,
                data_image: Some(data),
                data_meta: Some(meta),
                data_file: None,
            }
        }
        pub fn as_file(rc: u32, data: Text<'static>, meta: NodeMeta) -> Self {
            ResData {
                rc,
                data_str: None,
                data_listing: None,
                data_image: None,
                data_meta: Some(meta),
                data_file: Some(data),
            }
        }
    }
    // Returned by the queue when a task is done
    pub struct Res {
        pub id: usize,
        pub kind: Kind,
        pub res: ResData,
    }
    // An item in the queue
    pub struct Item {
        pub id: usize,
        pub kind: Kind,
        handle: JoinHandle<ResData>,
    }
    // The main queue struct
    pub struct Queue {
        items: Vec<Item>,
        next_id: usize,
    }
    impl Queue {
        pub fn new() -> Self {
            Queue {
                items: Vec::new(),
                next_id: 0,
            }
        }

        pub fn add_task<F>(&mut self, kind: Kind, task: F) -> usize
        where
            F: std::future::Future<Output = ResData> + Send + 'static,
        {
            let handle = tokio::spawn(task);
            let id = self.next_id;
            self.next_id += 1;
            self.items.push(Item { id, kind, handle });
            id
        }

        // Abort and remove any existing task with the same title
        pub fn add_task_unique<F>(&mut self, kind: Kind, task: F) -> Option<usize>
        where
            F: std::future::Future<Output = ResData> + Send + 'static,
        {
            for item in self.items.iter_mut() {
                if item.kind == kind {
                    item.handle.abort();
                }
            }
            // Remove aborted items
            self.items.retain(|item| item.kind != kind);

            Some(self.add_task(kind, task))
        }

        pub async fn check_tasks(&mut self) -> Vec<Res> {
            let mut completed = Vec::new();
            let mut finished_indices = Vec::new();

            for (i, item) in self.items.iter().enumerate() {
                if item.handle.is_finished() {
                    finished_indices.push(i);
                }
            }

            // Remove finished items and await their results
            // Awaiting must be done after identifying finished tasks
            // This does not block the loop
            // Iterate in reverse to avoid shifting indices
            for &i in finished_indices.iter().rev() {
                let item = self.items.remove(i);
                if let Ok(result) = item.handle.await {
                    completed.push(Res {
                        id: item.id,
                        kind: item.kind,
                        res: result,
                    });
                }
            }

            completed
        }

        pub fn pending_count(&self) -> usize {
            self.items.len()
        }

        pub fn get_pending(&self) -> Vec<&Item> {
            self.items.iter().collect()
        }
    }
}

// Command implementations
mod cmd {
    use crate::{APP_NAME, App, SEP, cfg, cmd_data, cs, kb, log, sc, shell_cmds};
    use crate::{aq, util};
    use clipboard::ClipboardContext;
    use clipboard::ClipboardProvider;
    use std::{env, fs, path::PathBuf, process::Command};

    pub fn exit(app: &mut App, _args: Vec<&str>) {
        app.should_quit = true;
    }

    pub fn enter(app: &mut App, _args: Vec<&str>) {
        // Update input to empty to reset search
        app.search_buf = String::new();
        app.update_results();
        // Get focused
        let focused = app.focused.clone();
        // NOTE: Handle shortcuts selections
        // Handle internal commands
        // Handle actual selection
        match focused.name.as_str() {
            // Shortcuts
            sc::EXIT => {
                exit(app, vec![]);
                return;
            }
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
        if app.focus_index >= app.results.len() {
            app.focus_index = 0;
        }
    }

    pub fn cur_up(app: &mut App, _args: Vec<&str>) {
        if app.results.is_empty() {
            app.focus_index = 0;
        } else if app.focus_index == 0 {
            app.focus_index = app.results.len() - 1;
        } else {
            app.focus_index -= 1;
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
        if !app.focused.is_file_like() {
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
    }

    pub fn sel_show(app: &mut App, _args: Vec<&str>) {
        let mut output_text = String::new();
        if app.multi_selection.is_empty() {
            app.set_output("Multi-select", "No items in multi selection.");
            return;
        }
        for path in app.multi_selection.iter() {
            output_text += &format!("{}\n", path.to_str().unwrap());
        }
        app.set_output("Multi-select", &output_text);
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
    }

    // Copy multi selection to the cwd
    pub fn sel_copy(app: &mut App, _args: Vec<&str>) {
        use tokio::fs;
        fn copy_dir_recursive<'a>(
            src: &'a std::path::Path,
            dst: &'a std::path::Path,
        ) -> std::pin::Pin<Box<dyn Future<Output = tokio::io::Result<()>> + Send + 'a>> {
            Box::pin(async move {
                fs::create_dir_all(dst).await?;
                let mut dir = fs::read_dir(src).await?;
                while let Some(entry) = dir.next_entry().await? {
                    let file_type = entry.file_type().await?;
                    let src_path = entry.path();
                    let dst_path = dst.join(entry.file_name());
                    if file_type.is_dir() {
                        copy_dir_recursive(&src_path, &dst_path).await?;
                    } else if file_type.is_file() {
                        fs::copy(&src_path, &dst_path).await?;
                    }
                    // Symlinks and other types can be handled here if needed
                }
                Ok(())
            })
        }
        if app.multi_selection.is_empty() {
            app.set_output("Multi-select", "No items in multi selection to copy.");
            return;
        }
        for path in app.multi_selection.iter() {
            let path = path.clone();
            let file_name = match path.file_name() {
                Some(name) => name.to_owned(),
                None => continue,
            };
            let dest_path = app.cwd.join(&file_name);
            app.async_queue.add_task(aq::Kind::FsOperation, async move {
                match fs::metadata(&path).await {
                    Ok(meta) => {
                        if meta.is_file() {
                            match fs::copy(&path, &dest_path).await {
                                Ok(_) => aq::ResData::as_str(
                                    0,
                                    format!(
                                        "Copied file {} to {}",
                                        path.to_string_lossy(),
                                        dest_path.to_string_lossy()
                                    ),
                                ),
                                Err(e) => aq::ResData::as_str(
                                    1,
                                    format!(
                                        "Failed to copy file {}: {}",
                                        path.to_string_lossy(),
                                        e
                                    ),
                                ),
                            }
                        } else if meta.is_dir() {
                            match copy_dir_recursive(&path, &dest_path).await {
                                Ok(_) => aq::ResData::as_str(
                                    0,
                                    format!(
                                        "Copied directory {} to {}",
                                        path.to_string_lossy(),
                                        dest_path.to_string_lossy()
                                    ),
                                ),
                                Err(e) => aq::ResData::as_str(
                                    1,
                                    format!(
                                        "Failed to copy directory {}: {}",
                                        path.to_string_lossy(),
                                        e
                                    ),
                                ),
                            }
                        } else {
                            aq::ResData::as_str(
                                1,
                                format!("Unsupported file type: {}", path.to_string_lossy()),
                            )
                        }
                    }
                    Err(e) => aq::ResData::as_str(
                        1,
                        format!("Failed to stat {}: {}", path.to_string_lossy(), e),
                    ),
                }
            });
        }
        app.multi_selection.clear();
        app.set_output("Multi-select", "Copy tasks queued.");
    }

    pub fn sel_delete(app: &mut App, _args: Vec<&str>) {
        use tokio::fs;
        if app.multi_selection.is_empty() {
            app.set_output("Multi-select", "No items in multi selection to delete.");
            return;
        }
        for path in app.multi_selection.iter() {
            let path = path.clone();
            app.async_queue.add_task(aq::Kind::FsOperation, async move {
                match fs::metadata(&path).await {
                    Ok(meta) => {
                        if meta.is_file() {
                            match fs::remove_file(&path).await {
                                Ok(_) => aq::ResData::as_str(
                                    0,
                                    format!("Deleted file {}", path.to_string_lossy()),
                                ),
                                Err(e) => aq::ResData::as_str(
                                    1,
                                    format!(
                                        "Failed to delete file {}: {}",
                                        path.to_string_lossy(),
                                        e
                                    ),
                                ),
                            }
                        } else if meta.is_dir() {
                            match fs::remove_dir_all(&path).await {
                                Ok(_) => aq::ResData::as_str(
                                    0,
                                    format!("Deleted directory {}", path.to_string_lossy()),
                                ),
                                Err(e) => aq::ResData::as_str(
                                    1,
                                    format!(
                                        "Failed to delete directory {}: {}",
                                        path.to_string_lossy(),
                                        e
                                    ),
                                ),
                            }
                        } else {
                            aq::ResData::as_str(
                                1,
                                format!("{} is neither file nor directory", path.to_string_lossy()),
                            )
                        }
                    }
                    Err(e) => aq::ResData::as_str(
                        1,
                        format!(
                            "Failed to get metadata for {}: {}",
                            path.to_string_lossy(),
                            e
                        ),
                    ),
                }
            });
        }
        app.multi_selection.clear();
        app.set_output("Multi-select", "Delete tasks queued.");
    }

    pub fn sel_move(app: &mut App, _args: Vec<&str>) {
        use tokio::fs;
        if app.multi_selection.is_empty() {
            app.set_output("Multi-select", "No items in multi selection to move.");
            return;
        }
        for path in app.multi_selection.iter() {
            let path = path.clone();
            let file_name = match path.file_name() {
                Some(name) => name.to_owned(),
                None => continue,
            };
            let dest_path = app.cwd.join(&file_name);
            app.async_queue.add_task(aq::Kind::FsOperation, async move {
                match fs::rename(&path, &dest_path).await {
                    Ok(_) => aq::ResData::as_str(
                        0,
                        format!(
                            "Moved {} to {}",
                            path.to_string_lossy(),
                            dest_path.to_string_lossy()
                        ),
                    ),
                    Err(e) => aq::ResData::as_str(
                        1,
                        format!("Failed to move {}: {}", path.to_string_lossy(), e),
                    ),
                }
            });
        }
        app.multi_selection.clear();
        app.set_output("Multi-select", "Move tasks queued.");
    }

    pub fn sel_clip_path(app: &mut App, _args: Vec<&str>) {
        if app.multi_selection.is_empty() {
            app.set_output(
                "Multi-select",
                "No items in multi selection to copy to clipboard.",
            );
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
    }

    pub fn cmd_finder_toggle(app: &mut App, _args: Vec<&str>) {
        app.mode_cmd_finder = !app.mode_cmd_finder;
        if app.mode_cmd_finder {
            app.search_buf = String::new();
        }
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
            text += &format!("{:<16} : {}\n", cmd_data.cmd, cmd_data.description);
        }
        app.set_output("Available Commands", &text);
    }

    pub fn cmd_list_dump(app: &mut App, args: Vec<&str>) {
        if args.is_empty() {
            app.set_output("Error", "No file path provided.");
            return;
        }
        let file_path = PathBuf::from(args[0]);
        let mut text = String::new();
        // Sort by command name
        let mut vec: Vec<_> = app.cmd_list.iter().collect();
        vec.sort_by(|a, b| a.1.cmd.cmp(&b.1.cmd));
        for (_name, cmd_data) in vec {
            text += &format!("{:<16} : {}\n", cmd_data.cmd, cmd_data.description);
        }
        match fs::write(&file_path, text) {
            Ok(_) => {
                app.set_output(
                    "Dumped Commands",
                    &format!("Command list dumped to {}", file_path.to_str().unwrap()),
                );
            }
            Err(e) => {
                app.set_output(
                    "Error",
                    &format!(
                        "Failed to dump command list to {}: {}",
                        file_path.to_str().unwrap(),
                        e
                    ),
                );
            }
        }
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
    }

    pub fn sec_up(app: &mut App, _args: Vec<&str>) {
        if app.show_output_window {
            if app.scroll_off_output >= 5 {
                app.scroll_off_output -= 5;
            } else {
                app.scroll_off_output = 0;
            }
            return;
        }
        if app.scroll_off_preview >= 5 {
            app.scroll_off_preview -= 5;
        } else {
            app.scroll_off_preview = 0;
        }
    }

    pub fn sec_down(app: &mut App, _args: Vec<&str>) {
        if app.show_output_window {
            let height = app.output_text.split("\n").count() as u16;
            if app.scroll_off_output < height {
                app.scroll_off_output += 5;
            }
            return;
        }
        let height = app.preview_content.lines.len() as u16;
        if app.scroll_off_preview < height {
            app.scroll_off_preview += 5;
        }
    }

    pub fn keybinds_show(app: &mut App, _args: Vec<&str>) {
        let kb_path = kb::get_path();
        let found = app.found_keybinds;
        let mut out = String::from(format!("Path: {}", kb_path.to_str().unwrap()));
        if !found {
            out += " \n(not found, using defaults)";
        }

        out += "\n\nKeybinds:\n";

        for kb in app.keybinds.iter() {
            out += kb::to_string_full(&app.cmd_list, kb).as_str();
        }

        app.set_output("Keybinds", &out);
    }

    pub fn hidden_toggle(app: &mut App, _args: Vec<&str>) {
        app.cfg.show_hidden = !app.cfg.show_hidden;
        app.update_listing();
        app.update_results();
        app.focus_index = 0;
    }

    // Edit the focused file
    pub fn edit(app: &mut App, _args: Vec<&str>) {
        let focused_path = app.find_focused_path();
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
            }
        }
    }

    pub fn os_open(app: &mut App, _args: Vec<&str>) {
        let focused_path = app.find_focused_path();

        let result = if cfg!(target_os = "windows") {
            Command::new("cmd")
                .args(&["/C", "start", "", &focused_path.to_string_lossy()])
                .spawn()
        } else if cfg!(target_os = "macos") {
            Command::new("open").arg(&focused_path).spawn()
        } else {
            // Assume Linux/Unix
            Command::new("xdg-open").arg(&focused_path).spawn()
        };

        if let Err(e) = result {
            app.set_output("Open", &format!("Failed to open: {}", e));
        }
    }

    pub fn goto(app: &mut App, args: Vec<&str>) {
        if args.is_empty() {
            app.set_output("Goto", "Error: No path provided.");
            return;
        }
        let path = PathBuf::from(args[0]);
        app.append_cwd(&path);
        app.update_listing();
        app.update_results();
        app.focus_index = 0;
    }

    pub fn input_clear(app: &mut App, _args: Vec<&str>) {
        app.search_buf.clear();
        app.command_input.clear();
        app.update_results();
        app.focus_index = 0;
    }

    pub fn shell_quick(app: &mut App, args: Vec<&str>) {
        // Join args into a single command string
        let mut shell_cmd = args[0..].join(" ");

        // Replace variables
        shell_cmd = app.replace_shell_vars(shell_cmd);

        // Add the cwd prefix
        shell_cmd = format!("cd '{}' && {}", app.cwd.to_str().unwrap(), shell_cmd);

        // Run the command
        log!("Running shell command: {}", shell_cmd);
        let out: String;
        match Command::new(app.user_shell.as_str())
            .arg("-c")
            .arg(shell_cmd)
            .output()
        {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                out = format!("{}{}", stdout, stderr);
            }
            Err(e) => {
                out = format!("Failed to run command: {}", e);
            }
        }
        if !out.is_empty() {
            app.set_output("Shell", &out);
        }
        app.update_listing();
        app.update_results();
    }

    pub fn shell_full(app: &mut App, _args: Vec<&str>) {
        let shell = env::var("SHELL").unwrap_or("/bin/sh".to_string());
        // Set the real program cwd
        env::set_current_dir(&app.cwd).unwrap_or(());
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
    }

    pub fn config_goto(app: &mut App, _args: Vec<&str>) {
        let config_path = cfg::Config::get_path();
        app.append_cwd(&config_path.parent().unwrap().to_path_buf());
        app.update_listing();
        app.update_results();
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
        CmdListDump,
        OutputWinToggle,
        OutputWinShow,
        OutputWinHide,
        Sel,
        SelClear,
        SelShow,
        SelSave,
        Copy,
        Delete,
        Move,
        ClipPath,
        MenuBack,
        Log,
        LogClear,
        SecUp,
        SecDown,
        KeybindsShow,
        DbgClear,
        Edit,
        OsOpen,
        GoTo,
        HiddenToggle,
        InputClear,
        ShellQuick,
        ShellFull,
        ConfigInit,
        ConfigClear,
        ConfigGoto,
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
            CmdName::CmdListDump,
            CmdData {
                fname: "Command List Dump",
                description: "Dump all commands to a file",
                cmd: "cmd-list-dump",
                vis_hidden: false,
                params: vec!["<file_path>"],
                op: cmd::cmd_list_dump,
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
            CmdName::Copy,
            CmdData {
                fname: "Copy Selection",
                description: "Copy the current selection of files and directories to the current directory",
                cmd: "copy",
                vis_hidden: false,
                params: vec![],
                op: cmd::sel_copy,
            },
        );
        map.insert(
            CmdName::Delete,
            CmdData {
                fname: "Delete Selection",
                description: "Delete all currently selected files and directories",
                cmd: "delete",
                vis_hidden: false,
                params: vec![],
                op: cmd::sel_delete,
            },
        );
        map.insert(
            CmdName::Move,
            CmdData {
                fname: "Move Selection",
                description: "Move (not copy) the currently selected files and directories to the current directory",
                cmd: "move",
                vis_hidden: false,
                params: vec![],
                op: cmd::sel_move,
            },
        );
        map.insert(
            CmdName::ClipPath,
            CmdData {
                fname: "Copy Selection Paths to Clipboard",
                description: "Copy the current selection of file and diretory paths to clipboard",
                cmd: "clip",
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
                op: cmd::keybinds_show,
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
            CmdName::OsOpen,
            CmdData {
                fname: "OS Open",
                description: "Open the focused file or directory with the default OS application",
                cmd: "os-open",
                vis_hidden: false,
                params: vec![],
                op: cmd::os_open,
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
            CmdName::HiddenToggle,
            CmdData {
                fname: "Hidden Toggle",
                description: "Toggle showing hidden files and directories",
                cmd: "hidden-toggle",
                vis_hidden: false,
                params: vec![],
                op: cmd::hidden_toggle,
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
        map.insert(
            CmdName::ConfigGoto,
            CmdData {
                fname: "Config Goto",
                description: "Go to the configuration directory",
                cmd: "config-goto",
                vis_hidden: false,
                params: vec![],
                op: cmd::config_goto,
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

exit           esc
exit           ctrl-q
home           alt-g
cur-up         up
cur-up         ctrl-k
cur-down       down
cur-down       ctrl-j
sel            tab
dir-up         ctrl-h
dir-up         left
dir-back       ctrl-u
explode        ctrl-x
edit           ctrl-e
goto           ctrl-g
enter          enter
enter          right
enter          ctrl-l
cmd-win        ctrl-w
cmd-find       ctrl-t 
cmd-list       ctrl-i
sec-up         alt-k
sec-up         ctrl-up
sec-down       alt-j
sec-down       ctrl-down
input-clear    ctrl-z
shell          ctrl-s
os-open        ctrl-o
hidden-toggle  alt-h
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

    // Unused
    // pub fn to_strings(kb: &KeyBind) -> (String, String) {
    //     let modifier = match kb.modifiers {
    //         KeyModifiers::ALT => "alt",
    //         KeyModifiers::CONTROL => "ctrl",
    //         KeyModifiers::SHIFT => "shift",
    //         KeyModifiers::NONE => "none",
    //         _ => "UNKNOWN",
    //     };
    //     let code_str = match &kb.code {
    //         KeyCode::Char(c) => c.to_string(),
    //         _ => kb.code.to_string(),
    //     };
    //     (modifier.to_string(), code_str.to_lowercase())
    // }

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
                "space" => KeyCode::Char(' '),
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
#
# Default configuration
#

# Command to run when you 'enter' on a file/directory
cmd_on_enter     edit

# How many items to show in a list - 0 = no limit
list_limit       100

# How many items can be searched at once - 0 = no limit
find_limit       0

# How many lines to preview - 0 = no limit
preview_limit    100

# Whether to force sixel image rendering (if terminal supports it)
force_sixel      false

# Maximum image width in characters
max_image_width  80

# Responsive breakpoint in characters
responsive_break 96

# Input polling interval in milliseconds
# Higher value = lower CPU usage, lower value = more responsive input
input_poll       10

# Whether to show hidden files by default
# show_hidden    true
"#;
    pub struct Config {
        pub cmd_on_enter: String,
        pub list_limit: u32,
        pub find_limit: u32,
        pub preview_limit: usize,
        pub force_sixel: bool,
        pub max_image_width: u16,
        pub responsive_break: u16,
        pub input_poll: u64,
        pub show_hidden: bool,
    }
    impl Config {
        pub fn new() -> Self {
            Self {
                cmd_on_enter: "edit".to_string(),
                list_limit: 100,
                find_limit: 0,
                preview_limit: 100,
                force_sixel: false,
                max_image_width: 80,
                responsive_break: 100,
                input_poll: 10,
                show_hidden: true,
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
                        if let Ok(limit) = value.parse::<u32>() {
                            config.list_limit = if limit == 0 { u32::MAX } else { limit };
                        }
                    }
                    "find_limit" => {
                        if let Ok(limit) = value.parse::<u32>() {
                            config.find_limit = if limit == 0 { u32::MAX } else { limit };
                        }
                    }
                    "preview_limit" => {
                        if let Ok(limit) = value.parse::<usize>() {
                            config.preview_limit = if limit == 0 { usize::MAX } else { limit };
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
                    "input_poll" => {
                        if let Ok(poll) = value.parse::<u64>() {
                            config.input_poll = poll;
                        }
                    }
                    "show_hidden" => {
                        if value.to_lowercase() == "true" {
                            config.show_hidden = true;
                        } else {
                            config.show_hidden = false;
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
echo "you said" {ASK}
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
        pub fn is_file_like(&self) -> bool {
            return self.node_type == NodeType::File
                || self.node_type == NodeType::Directory
                || self.node_type == NodeType::Image
                || self.node_type == NodeType::Executable
                || self.node_type == NodeType::Symlink;
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
        pub fn is_symlink(&self) -> bool {
            return self.node_type == NodeType::Symlink;
        }
    }
}

// Return type for loop control
enum LoopReturn {
    Continue,
    Ok,
}

// Main application state and control methods
struct App<'a> {
    async_queue: aq::Queue,
    should_quit: bool,
    search_buf: String,
    listing: Vec<NodeInfo>, // Full listing data
    results: Vec<NodeInfo>, // Filtered listing data
    focused: NodeInfo,
    focus_index: usize,
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
    //
    loading_preview: bool,
    loading_listing: bool,
    //
    whoami: String,
    user_shell: String,
}

impl<'a> App<'a> {
    fn new() -> Self {
        log!("App initialized");
        let bat_check = match Command::new("bat").arg("--version").output() {
            Ok(output) => output.status.success(),
            Err(_) => false,
        };
        let kb_check = Path::new(&kb::get_path()).exists();
        let cs_check = Path::new(&cs::Colors::get_path()).exists();
        let cfg_check = Path::new(&cfg::Config::get_path()).exists();
        let shell_cmds_check = Path::new(&shell_cmds::get_path()).exists();

        let username = env::var("USER").unwrap_or_else(|_| "unknown".to_string());
        let hostname = fs::read_to_string("/etc/hostname")
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        let whoami = format!("{}@{}", username, hostname);

        let user_shell = env::var("SHELL").unwrap_or("/bin/sh".to_string());

        Self {
            async_queue: aq::Queue::new(),
            should_quit: false,
            search_buf: String::new(),
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
            cmd_list: cmd_data::make_cmd_list(),
            shell_cmd_list: shell_cmds::make_list_auto(),
            keybinds: kb::make_list_auto(),
            cs: cs::Colors::make_list_auto(),
            cfg: cfg::Config::make_list_auto(),
            found_keybinds: kb_check,
            found_cs: cs_check,
            found_cfg: cfg_check,
            found_shell_cmds: shell_cmds_check,
            has_bat: bat_check,
            lay_preview_area: Rect::default(),
            loading_preview: false,
            loading_listing: false,
            whoami,
            user_shell,
        }
    }

    // Update cwd based on given path
    // Handles relative paths, absolute paths, home dir (~), and parent dir (..)
    // TODO: Should this actually change the env dir as well?
    // This would prevent issues with shell commands run from the app
    fn append_cwd(&mut self, path: &PathBuf) {
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

    // If no focused, use cwd
    fn find_focused_path(&self) -> PathBuf {
        let mut focused_path = self.cwd.clone();

        // Use the first multi selection as the focused path
        if !self.multi_selection.is_empty() {
            focused_path = self.multi_selection[0].clone();
            return focused_path;
        }

        // If no multi selection, use focused
        focused_path.push(&self.focused.name);
        // If focused is not a path, use cwd
        if !focused_path.exists() {
            focused_path = self.cwd.clone();
        }

        focused_path
    }

    fn get_directory_listing<'b>(
        path: PathBuf,
        mode_explode: bool,
        show_hidden: bool,
    ) -> Pin<Box<dyn Future<Output = aq::ResData> + Send + 'b>> {
        Box::pin(async move {
            let mut entries = Vec::new();

            match tokio::fs::read_dir(path.clone()).await {
                Ok(mut read_dir) => {
                    while let Some(entry_result) = read_dir.next_entry().await.transpose() {
                        if let Ok(entry) = entry_result {
                            let file_name = entry.file_name();
                            let file_name_str = file_name.to_string_lossy();
                            match entry.metadata().await {
                                Ok(metadata) => {
                                    let node_type = NodeType::find(&entry.path(), metadata.clone());
                                    if mode_explode {
                                        let sub_path = entry.path();
                                        if metadata.is_dir() {
                                            // Recursively collect files from subdirectory
                                            let sub_entries = App::get_directory_listing(
                                                sub_path,
                                                mode_explode,
                                                show_hidden,
                                            )
                                            .await;
                                            if let Some(sub_list) = sub_entries.data_listing {
                                                entries.extend(sub_list);
                                            }
                                        } else {
                                            if show_hidden == false
                                                && file_name_str.starts_with('.')
                                            {
                                                continue;
                                            }
                                            entries.push(NodeInfo {
                                                name: sub_path.to_str().unwrap().to_string(),
                                                node_type,
                                            });
                                        }
                                    } else {
                                        if show_hidden == false && file_name_str.starts_with('.') {
                                            continue;
                                        }
                                        entries.push(NodeInfo {
                                            name: file_name_str.to_string(),
                                            node_type,
                                        });
                                    }
                                }
                                Err(_) => {
                                    log!("Failed to get metadata for entry: {}", file_name_str);
                                    continue;
                                }
                            }
                        }
                    }
                    let meta = node_meta::NodeMeta::get(&path);
                    return aq::ResData::as_listing(0, entries.clone(), meta);
                }
                Err(_) => {
                    log!("Failed to read directory: {}", path.to_str().unwrap());
                    let meta = node_meta::NodeMeta::get(&path);
                    return aq::ResData::as_listing(1, entries.clone(), meta);
                }
            }
        })
    }

    fn set_output(&mut self, title: &str, text: &str) {
        let title = match title {
            "" => "Message",
            _ => title,
        };
        self.output_title = title.to_string();
        self.reset_sec_scroll();
        self.output_text = text.to_string();
        self.show_output_window = true;
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

    fn loading_line(&mut self) {
        self.preview_content = Text::default();
        for line in LOADING.lines() {
            self.preview_content += Line::styled(line, Style::default().fg(self.cs.info))
        }
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
    // TODO: This could be cached instead of regenerated every time
    fn welcome_message(&mut self) {
        self.preview_content = Text::default();
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
        let kb_sec_up = kb::find_by_cmd(&self.keybinds, &cmd_data::CmdName::SecUp).unwrap();
        let kb_sec_down = kb::find_by_cmd(&self.keybinds, &cmd_data::CmdName::SecDown).unwrap();
        let kb_sec_up_str = kb::to_string_short(&kb_sec_up);
        let kb_sec_down_str = kb::to_string_short(&kb_sec_down);
        // Tips
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
        self.preview_content += Line::styled(
            format!(
                "└ Scroll preview windows with {} & {}",
                kb_sec_up_str, kb_sec_down_str
            ),
            Style::default().fg(self.cs.tip),
        );
        // System Information
        self.preview_content += Line::from("");
        self.preview_content +=
            Line::styled("System Information:", Style::default().fg(self.cs.header));
        self.preview_content += Line::styled(
            format!("{} shell - {}", nf::CHECK, self.user_shell),
            Style::default().fg(self.cs.ok),
        );
        if self.has_bat {
            self.preview_content += Line::styled(
                format!(
                    "{} bat - file previews will use bat for syntax highlighting",
                    nf::CHECK
                ),
                Style::default().fg(self.cs.ok),
            );
        } else {
            self.preview_content += Line::styled(
                format!(
                    "{} bat - file previews will use built-in syntax highlighting",
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
                Line::styled(
                    format!("{} {} - loaded from {}", nf::CHECK, name, path_str),
                    Style::default().fg(cs.ok),
                )
            } else {
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
        // List keybinds
        self.preview_content += Line::from("");
        self.preview_content += Line::styled("Your keybinds:", Style::default().fg(self.cs.header));
        for kb in self.keybinds.iter() {
            let kb_short = kb::to_string_short(&kb);
            let cmd_name = cmd_data::get_cmd(&self.cmd_list, &kb.command);
            let kb_span = Span::styled(
                format!("{:<16}", cmd_name),
                Style::default().fg(self.cs.command),
            );
            let cmd_span = Span::styled(format!(" {}", kb_short), Style::default().fg(self.cs.tip));
            let line = Line::from(vec![kb_span, cmd_span]);
            self.preview_content += line;
        }
        self.preview_content += Line::from("");
    }

    fn pretty_dir_list(&self, list: &Vec<NodeInfo>) -> Text<'a> {
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
                    format!("{}{}| {}", ms, nf::SCMD, item.name),
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

    fn pretty_metadata(&self, metadata: &node_meta::NodeMeta) -> Text<'a> {
        fn line(icon: &str, label: &str, value: &str, color: Color) -> Line<'static> {
            Line::styled(
                format!("{} {:<14}: {}", icon, label, value),
                Style::default().fg(color),
            )
        }
        let mut text = Text::default();
        text += Line::styled(
            format!("{} {}", nf::DIRO, metadata.path.to_str().unwrap()),
            Style::default().fg(self.cs.dir),
        );
        text += line(
            nf::INFO,
            "permissions",
            &format!("{}", metadata.permissions),
            self.cs.info,
        );
        text += line(
            nf::INFO,
            "size",
            &format!("{}", metadata.size),
            self.cs.info,
        );
        text += line(
            nf::INFO,
            "modified",
            &format!("{}", metadata.modified),
            self.cs.info,
        );
        text += line(nf::INFO, "mime", &metadata.mime, self.cs.info);
        text
    }

    fn preview_dir(&mut self, focused_path: &PathBuf) {
        let owned_path = focused_path.clone();
        let owned_explode = self.mode_explode;
        let owned_hidden = self.cfg.show_hidden;
        self.async_queue.add_task_unique(
            aq::Kind::ListingPreview,
            App::get_directory_listing(owned_path, owned_explode, owned_hidden),
        );
    }

    fn preview_file(&mut self, focused_path: &PathBuf) {
        let focused_path = focused_path.clone();
        let cs = self.cs.clone();
        let preview_limit = self.cfg.preview_limit;
        let has_bat = self.has_bat;
        let sep = SEP.to_string();

        self.async_queue
            .add_task_unique(aq::Kind::FilePreview, async move {
                fn sanitize_content(input: &str) -> String {
                    let ansi_regex = Regex::new(r"\x1B\[[0-9;]*[A-Za-z]").unwrap();
                    let mut result = String::new();
                    let mut last = 0;
                    for mat in ansi_regex.find_iter(input) {
                        let before = &input[last..mat.start()];
                        for c in before.chars() {
                            if c.is_control() && c != '\n' && c != '\x1B' {
                                result.push('�');
                            } else {
                                result.push(c);
                            }
                        }
                        result.push_str(mat.as_str());
                        last = mat.end();
                    }
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

                fn syntect_to_ratatui_color(s: SyntectStyle) -> Color {
                    Color::Rgb(s.foreground.r, s.foreground.g, s.foreground.b)
                }

                let mut text = Text::default();
                let meta = crate::node_meta::NodeMeta::get(&focused_path);

                text += Line::styled(sep.clone(), Style::default().fg(cs.dim));

                // Try bat first
                if has_bat {
                    if let Ok(bat_output) = std::process::Command::new("bat")
                        .arg("--color=always")
                        .arg("--style=plain")
                        .arg(format!("--line-range=:{}", preview_limit))
                        .arg(focused_path.to_str().unwrap())
                        .output()
                    {
                        if bat_output.status.success() {
                            let mut bat_content = String::from_utf8_lossy(&bat_output.stdout);
                            bat_content = bat_content.replace("\r\n", "\n").into();
                            bat_content = bat_content.replace("\t", "    ").into();
                            bat_content = sanitize_content(&bat_content.to_string()).into();
                            match bat_content.as_ref().into_text() {
                                Ok(bat_text) => {
                                    for line in bat_text.lines.iter().take(preview_limit) {
                                        text += Line::from(line.clone());
                                    }
                                    return aq::ResData::as_file(0, text, meta);
                                }
                                Err(_) => {
                                    text += Line::styled(
                                        "Error: Unable to convert bat output to text.",
                                        Style::default().fg(cs.error),
                                    );
                                    return aq::ResData::as_file(1, text, meta);
                                }
                            }
                        }
                    }
                }

                // Fallback to syntect
                let ss = SyntaxSet::load_defaults_newlines();
                let ts = ThemeSet::load_defaults();
                let syntax = ss
                    .find_syntax_for_file(&focused_path)
                    .unwrap_or(None)
                    .unwrap_or_else(|| ss.find_syntax_plain_text());
                let mut h = HighlightLines::new(syntax, &ts.themes["base16-eighties.dark"]);
                text += Line::styled(
                    format!("detected: {}", syntax.name),
                    Style::default().fg(cs.info),
                );
                text += Line::styled(sep, Style::default().fg(cs.dim));

                let file = File::open(&focused_path);
                if let Ok(file) = file {
                    let reader = BufReader::new(file);
                    for (i, line) in reader.lines().enumerate() {
                        if i >= preview_limit {
                            break;
                        }
                        if let Ok(line) = line {
                            let ranges = h.highlight_line(&line, &ss).unwrap_or_default();
                            let mut styled_line = Line::default();
                            for (style, text_part) in ranges {
                                styled_line.push_span(Span::styled(
                                    text_part.to_string(),
                                    Style::default().fg(syntect_to_ratatui_color(style)),
                                ));
                            }
                            text += styled_line;
                        }
                    }
                } else {
                    text += Line::styled(
                        "Err: Unable to read file content.",
                        Style::default().fg(cs.error),
                    );
                }

                aq::ResData::as_file(0, text, meta)
            });
    }

    fn preview_image(&mut self, focused_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        let mut picker = Picker::from_fontsize((6, 12));
        if self.cfg.force_sixel {
            picker.set_protocol_type(ProtocolType::Sixel);
        } else {
            picker.set_protocol_type(ProtocolType::Halfblocks);
        }
        self.loading_line();

        // Clear the existing image data
        self.preview_image = None;

        let focused_path = focused_path.clone();
        self.async_queue
            .add_task_unique(aq::Kind::ImagePreview, async move {
                // Load an image with the image crate.
                let dyn_img = image::ImageReader::open(&focused_path)
                    .unwrap()
                    .decode()
                    .unwrap();

                // Create the Protocol which will be used by the widget.
                let image = picker.new_resize_protocol(dyn_img);
                let meta = node_meta::NodeMeta::get(&focused_path);
                aq::ResData::as_image(0, image, meta)
            });
        Ok(())
    }

    fn update_preview(&mut self) {
        self.reset_sec_scroll();
        match self.focused.name.as_str() {
            sc::EXIT => {
                self.welcome_message();
            }
            sc::HOME => {
                self.preview_content = Text::default();
                self.preview_content += self.fmtln_path(&dirs::home_dir().unwrap());
                self.preview_content += self.fmtln_sc("Go to the home directory");
            }
            sc::DIR_UP => {
                let up_path = self.cwd.parent().unwrap_or(&self.cwd);
                self.preview_content = Text::default();
                self.preview_content += self.fmtln_path(&up_path.to_path_buf());
                self.preview_content += self.fmtln_sc("Go up to the parent directory");
            }
            sc::DIR_BACK => {
                self.preview_content = Text::default();
                self.preview_content += self.fmtln_path(&self.lwd);
                self.preview_content += self.fmtln_sc("Go back to the last working directory");
            }
            sc::EXP => {
                self.preview_content = Text::default();
                self.preview_content += self.fmtln_sc("Toggle explode mode");
                self.preview_content += Line::styled(
                    "Shows all files in subdirectories under the current directory.",
                    Style::default().fg(self.cs.tip),
                );
                let status = if self.mode_explode { "ON" } else { "OFF" };
                self.preview_content += self.fmtln_info("explode mode", status);
            }
            sc::CMDS => {
                self.preview_content = Text::default();
                self.preview_content += self.fmtln_sc("Show visual commands");
                self.preview_content += Line::styled(
                    "Toggles a visual command menu in the listing.",
                    Style::default().fg(self.cs.tip),
                );
            }
            sc::MENU_BACK => {
                self.preview_content = Text::default();
                self.preview_content += self.fmtln_sc("Go back to the previous menu");
                self.preview_content += Line::styled(
                    "Exits the current visual command menu.",
                    Style::default().fg(self.cs.tip),
                );
            }
            sc::LOADING => {
                self.preview_content = Text::default();
                self.preview_content += self.fmtln_sc("Loading...");
                self.loading_line();
                self.preview_content +=
                    Line::styled("The listing is loading.", Style::default().fg(self.cs.tip));
                if self.mode_explode {
                    self.preview_content += Line::styled(
                        "Explode mode is ON. This may take a while.",
                        Style::default().fg(self.cs.warning),
                    );
                }
            }
            _ => {
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
                    self.preview_content = Default::default();
                    self.preview_content += Line::styled(
                        format!("{} internal command", nf::CMD),
                        Style::default().fg(self.cs.command),
                    );
                    self.preview_content += Line::styled(
                        format!("name   : {}", data.fname),
                        Style::default().fg(self.cs.tip),
                    );
                    self.preview_content += Line::styled(
                        format!("cmd    : {}", data.cmd),
                        Style::default().fg(self.cs.command),
                    );
                    let kb_data = kb::find_by_cmd(&self.keybinds, &cmd_name);
                    let mut kb_str = "unbound".to_string();
                    if let Some(kb) = kb_data {
                        kb_str = kb::to_string_short(&kb);
                    }
                    self.preview_content += Line::styled(
                        format!("keybind: {}", kb_str),
                        Style::default().fg(self.cs.command),
                    );
                    self.preview_content += Line::styled(
                        format!("info   : {}", data.description),
                        Style::default().fg(self.cs.info),
                    );
                    return;
                }
                // Check if we have a shell command
                if self.focused.is_shell_command() {
                    self.preview_content = Default::default();
                    self.preview_content += Line::styled(
                        format!("{} user shell script", nf::SCMD),
                        Style::default().fg(self.cs.command),
                    );
                    self.preview_content += Line::styled(
                        format!("cmd: {}", self.focused.name),
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
                } else if self.focused.is_executable() {
                    self.preview_file(&focused_path);
                } else if self.focused.is_image() {
                    let _ = self.preview_image(&focused_path);
                } else if self.focused.is_shortcut() {
                    // Internal shortcut
                    // Populated elsewhere
                    self.preview_content = Text::styled(
                        "Shortcut preview not supported.",
                        Style::default().fg(self.cs.error),
                    );
                } else if self.focused.is_unknown() {
                    if self.results.is_empty() {
                        self.preview_content = Text::default();
                        self.preview_content +=
                            Line::styled("Nothing found...", Style::default().fg(self.cs.error));
                        for line in E404.lines() {
                            self.preview_content +=
                                Line::styled(line, Style::default().fg(self.cs.warning));
                        }
                    } else {
                        self.preview_content =
                            Text::styled("Unknown file type.", Style::default().fg(self.cs.error));
                    }
                } else {
                    // Unknown
                    self.preview_content = Text::styled(
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
        // Clear listing and display loading items
        self.listing = Vec::new();
        self.listing.insert(
            0,
            NodeInfo {
                name: sc::EXP.to_string(),
                node_type: NodeType::Shortcut,
            },
        );
        self.listing.insert(
            0,
            NodeInfo {
                name: sc::EXIT.to_string(),
                node_type: NodeType::Shortcut,
            },
        );
        self.listing.insert(
            0,
            NodeInfo {
                name: sc::LOADING.to_string(),
                node_type: NodeType::Shortcut,
            },
        );
        // Handle cmd finder
        if self.mode_cmd_finder {
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
        let owned_cwd = self.cwd.clone();
        let owned_explode = self.mode_explode;
        let owned_hidden = self.cfg.show_hidden;
        self.async_queue
            .add_task_unique(aq::Kind::ListingDir, async move {
                let mut listing_res =
                    App::get_directory_listing(owned_cwd.clone(), owned_explode, owned_hidden)
                        .await;
                // Turn listing into listing vec
                let mut listing = match listing_res.data_listing.take() {
                    Some(list) => list,
                    None => Vec::new(),
                };
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
                let meta = node_meta::NodeMeta::get(&owned_cwd);
                aq::ResData::as_listing(0, listing, meta)
            });
    }

    // Fuzzy finding
    fn update_results(&mut self) {
        let limit = self.cfg.find_limit;
        let input = self.search_buf.clone();
        let listing = self.listing.clone();
        self.async_queue
            .add_task_unique(aq::Kind::ListingResult, async move {
                let matcher = SkimMatcherV2::default();
                let mut scored: Vec<_> = listing
                    .iter()
                    .take(limit as usize) // Limit for performance
                    .filter_map(|item| {
                        matcher
                            .fuzzy_match(&item.name, &input)
                            .map(|score| (score, item.clone()))
                    })
                    .collect();
                scored.sort_by(|a, b| b.0.cmp(&a.0));
                aq::ResData::as_listing(
                    0,
                    scored.into_iter().map(|(_, item)| item).collect(),
                    node_meta::NodeMeta::empty(),
                )
            });
    }

    fn reset_sec_scroll(&mut self) {
        self.scroll_off_preview = 0;
        self.scroll_off_output = 0;
    }

    fn update_focused(&mut self) -> bool {
        let old = self.focused.clone();
        if self.focus_index < self.results.len() {
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
        let kb_up = kb::find_by_cmd(&self.keybinds, &cmd_data::CmdName::CurUp).unwrap();
        let kb_dn = kb::find_by_cmd(&self.keybinds, &cmd_data::CmdName::CurDown).unwrap();
        let kb_ups = kb::find_by_cmd(&self.keybinds, &cmd_data::CmdName::SecUp).unwrap();
        let kb_dns = kb::find_by_cmd(&self.keybinds, &cmd_data::CmdName::SecDown).unwrap();
        let cmd = match (modifiers, code) {
            v if v == (kb_dn.modifiers, kb_dn.code) => self.get_cmd(&cmd_data::CmdName::SecDown),
            v if v == (kb_up.modifiers, kb_up.code) => self.get_cmd(&cmd_data::CmdName::SecUp),
            v if v == (kb_dns.modifiers, kb_dns.code) => self.get_cmd(&cmd_data::CmdName::SecDown),
            v if v == (kb_ups.modifiers, kb_ups.code) => self.get_cmd(&cmd_data::CmdName::SecUp),
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

    // Returns true if input changed
    fn input_search(&mut self, modifiers: KeyModifiers, code: KeyCode) -> bool {
        match (modifiers, code) {
            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                self.search_buf.push(c);
                return true;
            }
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                self.search_buf.pop();
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
        let cmd_data = cmd_data::get_cmd_data(&self.cmd_list, &cmd_name);
        (cmd_data.op)(self, args);
        LoopReturn::Ok
    }

    async fn handle_async(&mut self) {
        self.loading_listing = false;
        self.loading_preview = false;
        let pending = self.async_queue.get_pending();
        for p in pending {
            match p.kind {
                aq::Kind::ListingResult => self.loading_listing = true,
                aq::Kind::ListingDir => self.loading_listing = true,
                aq::Kind::ListingPreview => self.loading_preview = true,
                aq::Kind::FilePreview => self.loading_preview = true,
                aq::Kind::ImagePreview => self.loading_preview = true,
                aq::Kind::FsOperation => {}
            }
        }
        let completed = self.async_queue.check_tasks().await;
        let mut output = String::new();
        for item in completed {
            match item.kind {
                aq::Kind::ListingDir => {
                    self.listing = item.res.data_listing.unwrap(); // This should be safe to unwrap
                    self.update_results();
                }
                aq::Kind::ListingResult => {
                    self.results = item.res.data_listing.unwrap(); // This should be safe to unwrap
                    // TODO: Should make a "reset_focus" function
                    self.focus_index = 0;
                    self.update_focused();
                    self.update_preview();
                }
                aq::Kind::ListingPreview => {
                    let data = item.res.data_listing.unwrap();
                    let meta = item.res.data_meta.unwrap();
                    self.preview_content = Default::default();
                    self.preview_content = self.pretty_metadata(&meta);
                    self.preview_content += Line::styled(SEP, Style::default().fg(self.cs.dim));
                    let pretty_listing = self.pretty_dir_list(&data);
                    for line in pretty_listing.lines.iter().take(20) {
                        self.preview_content += Line::from(line.clone());
                    }
                }
                aq::Kind::ImagePreview => {
                    let meta = item.res.data_meta.unwrap();
                    self.preview_content = Default::default();
                    self.preview_content = self.pretty_metadata(&meta);
                    self.preview_image = item.res.data_image;
                }
                aq::Kind::FilePreview => {
                    let data_text = match &item.res.data_file {
                        Some(d) => d.clone(),
                        None => Text::from("No data"),
                    };
                    let meta = item.res.data_meta.unwrap();
                    self.preview_content = Default::default();
                    self.preview_content = self.pretty_metadata(&meta);
                    for line in data_text.lines.iter().take(self.cfg.preview_limit) {
                        self.preview_content += Line::from(line.clone());
                    }
                }
                aq::Kind::FsOperation => {
                    if item.res.data_str.is_some() {
                        let data = match &item.res.data_str {
                            Some(d) => d.clone(),
                            None => "No data".to_string(),
                        };
                        output += &format!(
                            "Task #{}, rc:{} '{:#?}' -  {}",
                            item.id, item.res.rc, item.kind, data
                        );
                    } else {
                        output += &format!(
                            "Task #{}, rc:{} '{:#?}' - No data returned\n",
                            item.id, item.res.rc, item.kind
                        );
                    }
                    self.set_output("Async Tasks", &output);
                    self.update_listing();
                    self.update_results();
                }
            }
        }
    }

    async fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
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
            // Async handling
            self.handle_async().await; // Should not block
            // Render the UI
            terminal.draw(|f| self.render(f))?;
            if event::poll(std::time::Duration::from_millis(self.cfg.input_poll))? {
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
                            LoopReturn::Ok => {}
                        }
                        continue;
                    }
                    // Before key press handling
                    let input_changed = self.input_search(modifiers, code);
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
                        LoopReturn::Ok => {}
                    }
                    // After key press handling
                    let sel_changed = self.update_focused();
                    if sel_changed {
                        self.update_preview();
                    }
                }
            }
        } // End loop

        Ok(())
    }
    fn render(&mut self, frame: &mut Frame) {
        fn lines_to_percent(lines: usize) -> u16 {
            let term_height = terminal::size().map(|(_, h)| h as usize).unwrap_or(24);
            let percent = (lines as f32 / term_height as f32) * 100.0;
            percent.min(100.0) as u16
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

        // Loading indicator
        // let loading_arr = ["", "", "", "", "", ""];
        let loading_arr = ["", "", ""];
        let loading_index = (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis()
            / self.cfg.input_poll as u128
            % loading_arr.len() as u128) as usize;

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
        if self.search_buf.is_empty() {
            input_str = "Type to search...".to_string();
            input_color = self.cs.dim;
        } else {
            input_str = self.search_buf.clone();
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
                    "┤{})))  [ {} / {} ]",
                    APP_NAME.to_uppercase(),
                    self.results.len(),
                    self.listing.len(),
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(self.cs.search_border)),
        );

        // Results list
        let mut results_pretty = self.pretty_dir_list(&self.results);
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
            format!("[{}]", nf::BOMB)
        } else {
            "".to_string()
        };
        let hidden_str = if !self.cfg.show_hidden {
            format!("[{}]", nf::EYEN)
        } else {
            "".to_string()
        };
        let loading_str_listing = if self.loading_listing {
            loading_arr[loading_index].to_string()
        } else {
            "".to_string()
        };
        let list_title = format!(
            "|{}{}{} {}",
            explode_str,
            hidden_str,
            util::fpath(&self.cwd),
            loading_str_listing
        );
        let list_widget = List::new(results_pretty).block(
            Block::default()
                .title(list_title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(self.cs.listing_border)),
        );
        let mut state = ListState::default();
        if !self.results.is_empty() {
            state.select(Some(self.focus_index as usize));
        }

        // Preview box
        let loading_str_preview = if self.loading_preview {
            loading_arr[loading_index].to_string()
        } else {
            "".to_string()
        };
        let preview_widget = Paragraph::new(self.preview_content.clone())
            .block(
                Block::default()
                    .title(format!(
                        "{} m(0)_(0)m | {} {} ",
                        nf::LOOK,
                        self.focused.name,
                        loading_str_preview
                    ))
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
        let mut loading_str_status = nf::WAIT.to_string();
        if self.async_queue.pending_count() > 0 {
            loading_str_status = loading_arr[loading_index].to_string();
        }
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
        let status_text = format!(
            " {} {} | {} {} | {} {} | {}",
            loading_str_status,
            self.async_queue.pending_count(),
            nf::DUDE,
            self.whoami,
            nf::MSEL,
            multi_count,
            hhmmss
        );
        let status_widget =
            Paragraph::new(status_text).style(Style::default().fg(self.cs.misc).bg(Color::Black));
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
                    y: self.lay_preview_area.y + 5,
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
        let popup_width = if area.width < threshold { 90 } else { 50 };
        if self.show_command_window {
            let popup_area = centered_rect(popup_width, lines_to_percent(3), area);
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
            let popup_area = centered_rect(popup_width, 90, area);
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
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    log!("======= Starting application =======");
    color_eyre::install()?;
    util::cls();
    enable_raw_mode()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(std::io::stdout()))?;

    let mut app = App::new();

    app.run(&mut terminal).await?;

    disable_raw_mode()?;
    util::cls();
    println!("{} exited successfully.", APP_NAME);
    Ok(())
}
