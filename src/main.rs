#![allow(unused_imports)]
mod event_handler;
use std::time::{Duration, Instant};
mod ui;
mod utils;

use ratatui::prelude::{Color, Line, Modifier, Span, Style};
use ratatui::text::ToText;
use ratatui::widgets::{ListItem, ListState};
use std::cmp::Ordering;
use std::{fs, path::PathBuf};
use utils::{get_state_data, move_file, recursively_copy_dir};

#[derive(Debug, Clone, PartialEq)]
pub enum FsEntryType {
    File,
    Directory,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FsEntry {
    name: String,
    entry_type: FsEntryType,
    size: u64,
    file_permission: u32,
    is_selected: bool,
}

#[derive(Debug, Clone)]
pub enum FileContent {
    Text(String),
    Binary(String),
}

#[derive(Debug, Clone)]
pub enum PreviewContent {
    File(FileContent),
    Directory(Vec<FsEntry>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum PopupType {
    Delete(String),
    Rename(String),
    Create(String),
    Copy(String),
    Move(String),
    None,
}

#[derive(Debug, Clone)]
pub struct Notification {
    message: String,
    created_at: Instant,
    duration: Duration,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Move,
    Copy,
    None,
}

#[derive(Debug, Clone)]
pub struct FilePane {
    path: PathBuf,
    entries: Vec<FsEntry>,
    selection: ListState, // Cursor
    old_selection: ListState,
    notify: Option<Notification>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Selected {
    Left,
    Right,
}

#[derive(Debug)]
pub struct FileManager {
    left_pane: FilePane,
    right_pane: FilePane,
    selected_pane: Selected,

    input_buffer: String,
    cursor_pos: u16,
    search_prefix: String,
    
    notify: Option<Notification>,
    popup: PopupType,
}

impl FilePane {
    fn new(path: PathBuf, entries: Vec<FsEntry>) -> Self {
        Self {
            path,
            entries,
            selection: ListState::default().with_selected(Some(0)),
            old_selection: ListState::default().with_selected(Some(0)),
            notify: None,
        }
    }

    fn list_files(current_entries: &Vec<FsEntry>, cursor_index: Option<usize>) -> Vec<ListItem> {
        let list_current_items: Vec<ListItem> = current_entries
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let (bar, bar_style) = if entry.is_selected {
                    ("▌", Style::default().fg(Color::Yellow))
                } else {
                    (" ", Style::default())
                };

                let icon = match entry.entry_type {
                    FsEntryType::Directory => "📁",
                    FsEntryType::File => "📄",
                };

                let is_cursor_row = cursor_index == Some(index);

                let text = Line::from(vec![
                    Span::styled(bar, bar_style),
                    Span::raw(" "),
                    Span::styled(
                        format!("{} {}", icon, entry.name),
                        if is_cursor_row {
                            Style::default()
                                .bg(Color::Blue)
                                .fg(Color::Black)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                        },
                    ),
                ]);

                ListItem::new(text)
            })
            .collect();
        list_current_items
    }

    fn show_notification(&mut self, message: String) {
        self.notify = Some(Notification {
            message,
            created_at: Instant::now(),
            duration: Duration::from_secs(3),
        });
    }

    fn clear_expired_notifications(&mut self) {
        if let Some(noti) = &self.notify {
            if noti.created_at.elapsed() >= noti.duration {
                self.notify = None;
            }
        }
    }

    fn enter_directory(&mut self, new_path: PathBuf) {
        match get_state_data(&new_path) {
            Ok(entries) => {
                self.path = new_path;
                self.entries = entries;
            }
            Err(e) => self.show_notification(e.to_string()),
        }
    }

    fn refresh_directory(&mut self) {
        match get_state_data(&self.path) {
            Ok(entries) => {
                self.entries = entries;
            }
            Err(e) => self.show_notification(e.to_string()),
        }
    }

    fn navigate_down(&mut self) {
        self.selection.select_next();
    }

    fn navigate_up(&mut self) {
        if self.selection.selected().unwrap_or(0) == 0 {
            self.selection.select(Some(0));
        }
        self.selection.select_previous();
    }

    fn has_selected(&self) -> bool {
        self.entries.iter().any(|e| e.is_selected == true)
    }

    fn get_selected_paths(&self) -> Vec<PathBuf> {
        if !self.has_selected() {
            if let Some(entry) = self.get_selected_index_entry() {
                vec![self.path.join(&entry.name)]
            } else {
                vec![]
            }
        } else {
            self.entries
                .iter()
                .filter(|entry| entry.is_selected)
                .filter_map(|entry| self.path.join(&entry.name).canonicalize().ok())
                .collect()
        }
    }
/*
    fn select_current(&mut self) {
        if let Some(index) = self.selection.selected() {
            if let Some(entry) = self.entries.get_mut(index) {
                entry.is_selected = true;
            }
        }
    }
*/
    fn deselect_all(&mut self) {
        for entry in &mut self.entries {
            entry.is_selected = false;
        }
        self.refresh_directory();
    }

    fn get_selected_index_entry(&self) -> Option<&FsEntry> {
        self.selection
            .selected()
            .and_then(|index| self.entries.get(index))
    }

    fn navigate_to_parent(&mut self) {
        if let Some(parent_path) = self.path.parent() {
            self.enter_directory(parent_path.to_path_buf());
            self.selection = ListState::default().with_selected(Some(0));
        }
    }

    fn navigate_to_child(&mut self) {
        if let Some(entry) = self.get_selected_index_entry() {
            if let FsEntryType::Directory = entry.entry_type {
                let mut path = self.path.clone();
                path.push(&entry.name);
                self.enter_directory(path);
                self.selection = ListState::default().with_selected(Some(0));
            }
        }
    }
} // FilePane

impl FileManager {
    fn new(start_path: &PathBuf) -> Result<Self, std::io::Error> {
        let left_entries = get_state_data(start_path).unwrap();
        let right_entries = get_state_data(start_path).unwrap();

        let state = Self {
            left_pane: FilePane::new(start_path.clone(), left_entries),
            right_pane: FilePane::new(start_path.clone(), right_entries),

            input_buffer: String::new(),
            cursor_pos: 0,
            search_prefix: "".to_string(),
            
            selected_pane: Selected::Right,
            notify: None,
            popup: PopupType::None,
        };

        Ok(state)
    }

    fn selected_pane(&self) -> &FilePane {
        if self.selected_pane == Selected::Left {
            &self.left_pane
        } else {
            &self.right_pane
        }
    }

    fn other_pane(&self) -> &FilePane {
        if self.selected_pane == Selected::Left {
            &self.right_pane
        } else {
            &self.left_pane
        }
    }

    fn selected_pane_mut(&mut self) -> &mut FilePane {
        if self.selected_pane == Selected::Left {
            &mut self.left_pane
        } else {
            &mut self.right_pane
        }
    }

    fn other_pane_mut(&mut self) -> &mut FilePane {
        if self.selected_pane == Selected::Left {
            &mut self.right_pane
        } else {
            &mut self.left_pane
        }
    }

    fn delete_selected(&mut self) {
        for path in &self.selected_pane().get_selected_paths() {
            let result = if path.is_file() {
                fs::remove_file(path)
            } else {
                fs::remove_dir_all(path)
            };

            if result.is_ok() {
                self.popup = PopupType::None;
                self.selected_pane_mut().refresh_directory();
            } else if let Err(err) = result {
                self.show_notification(format!("Failed to delete {:?}: {}", path, err));
            }
        }
    }

    fn rename_selected(&mut self, input: &str) {
        if let Some(entry) = self.selected_pane().get_selected_index_entry() {
            let path = self.selected_pane().path.clone();
            let old_path = path.join(&entry.name);
            let new_path = path.join(input.trim_end_matches('/'));

            if fs::rename(&old_path, &new_path).is_ok() {
                self.selected_pane_mut().refresh_directory();
                self.popup = PopupType::None;
            }
        }
    }

    fn create_entry(&mut self, input: String) {
        let is_directory = input.ends_with('/');
        let trimmed_input = input.trim_end_matches('/');
        let mut segments: Vec<&str> = trimmed_input.split('/').collect();

        if let Some(name) = segments.pop() {
            let mut path = self.selected_pane().path.clone();
            for segment in segments {
                path.push(segment);
            }

            if let Err(e) = fs::create_dir_all(&path) {
                self.show_notification(format!("Error creating directories: {e}").as_str());
                return;
            }

            path.push(name);

            if is_directory {
                self.create_directory(path);
            } else {
                self.create_file(path);
            }
        }
        self.close_confirmation_popup();
    }

    fn create_directory(&mut self, path: PathBuf) {
        match fs::create_dir_all(&path) {
            Ok(_) => self.on_create_success(),
            Err(e) => self.show_notification(e.to_string()),
        }
    }

    fn create_file(&mut self, path: PathBuf) {
        match fs::File::create(&path) {
            Ok(_) => self.on_create_success(),
            Err(e) => self.show_notification(e.to_string()),
        }
    }

    fn on_create_success(&mut self) {
        self.left_pane.refresh_directory();
        self.right_pane.refresh_directory();
        self.input_buffer.clear();
    }

    fn show_notification<S: AsRef<str>>(&mut self, message: S) {
        self.notify = Some(Notification {
            message: message.as_ref().to_string(),
            created_at: Instant::now(),
            duration: Duration::from_secs(3),
        });
    }

    fn clear_expired_notifications(&mut self) {
        if let Some(noti) = &self.notify {
            if noti.created_at.elapsed() >= noti.duration {
                self.notify = None;
            }
        }
    }


    fn copy_selected_to_other_pane(&mut self) {
        let src_entries = self.selected_pane().get_selected_paths();
        for src in src_entries {
            let dst = self.other_pane().path.join(src.file_name().unwrap());
            if src.is_file() {
                if let Err(e) = fs::copy(src, &dst) {
                    self.show_notification(e.to_string())
                }
            } else if src.is_dir() {
                if let Err(e) = recursively_copy_dir(&src, &dst) {
                    self.show_notification(e.to_string())
                }
            }
        }
        
        self.close_confirmation_popup();
        self.other_pane_mut().refresh_directory();
    }

    fn move_selected_to_other_pane(&mut self) {
        let src_entries = self.selected_pane().get_selected_paths();
        for src in src_entries {
            let dst = self.other_pane().path.join(src.file_name().unwrap());
            if src.is_file() {
                if fs::copy(&src, &dst).is_ok() {
                    if let Err(e) = fs::remove_file(&src) {
                        self.show_notification(e.to_string())
                    }
                }
            } else if src.is_dir() {
                if let Err(e) = move_file(&src, &dst) {
                    self.show_notification(e.to_string())
                }
            }
        }

        self.close_confirmation_popup();
        self.selected_pane_mut().refresh_directory();
        self.other_pane_mut().refresh_directory();
    }

    fn close_confirmation_popup(&mut self) {
        self.popup = PopupType::None;
    }
} // FileManager

fn main() -> std::io::Result<()> {
    let terminal = ratatui::init();

    let home_dir = std::env::var("HOME").unwrap_or(".".to_string());

    let mut start_dir = PathBuf::from("~");
    if start_dir.cmp(&PathBuf::from("~")).is_eq() {
        start_dir = PathBuf::from(&home_dir);
    }
    let absolute_path = start_dir.canonicalize().expect("Failed to resolve path");

    let exit_result = FileManager::new(&absolute_path).unwrap().run(terminal);

    ratatui::restore();
    exit_result
}
