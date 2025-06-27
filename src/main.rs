mod event_handler;
use std::time::{Duration, Instant};
mod ui;
mod utils;
use ratatui::widgets::ListState;
use std::{fs, path::PathBuf, sync::mpsc, thread};
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
    Confirm,
    Rename,
    Create,
    None,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InteractionMode {
    Normal,
    MultiSelect,
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
pub struct Clipboard {
    paths: Vec<PathBuf>,
    action: Action,
}

#[derive(Debug)]
pub struct FileManager {
    parent_view: ParentView,
    
    left_path: PathBuf,
    right_path: PathBuf,
    
    left_entries: Vec<FsEntry>,
    right_entries: Vec<FsEntry>,

    selection_left: ListState,
    selection_right: ListState,
    mode_left: InteractionMode,
    mode_right: InteractionMode,
    
    notify: Option<Notification>,
    clipboard: Clipboard,
    input_buffer: String,
    popup: PopupType,
}

#[derive(Debug)]
pub struct ParentView {
    entries: Vec<FsEntry>,
    path: Option<PathBuf>,
    selection: ListState,
}

impl FileManager {
    fn new(start_path: &PathBuf) -> Result<Self, std::io::Error> {
        let (entries, parent_path, parent_entries) = get_state_data(start_path).unwrap();
        let (right_entries, _, _) = get_state_data(start_path).unwrap();

        let mut state = Self {
            parent_view: ParentView {
                path: parent_path,
                entries: parent_entries,
                selection: ListState::default(),
            },
            left_path: start_path.clone(),
            left_entries: entries,
            selection_left: ListState::default().with_selected(Some(0)),
            mode_left: InteractionMode::Normal,
            
            right_path: start_path.clone(),
            right_entries: right_entries,
            selection_right: ListState::default().with_selected(Some(0)),
            mode_right: InteractionMode::Normal,
            
            notify: None,
            clipboard: Clipboard {
                paths: vec![],
                action: Action::None,
            },
            input_buffer: String::new(),
            popup: PopupType::None,
        };

        state.refresh_right_view();
        state.update_parent_selection();
        Ok(state)
    }

    fn refresh_left_directory(&mut self, new_path: PathBuf) {
        match get_state_data(&new_path) {
            Ok((entries, parent_path, parent_entries)) => {
                self.left_path = new_path;
                self.left_entries = entries;
                self.parent_view.path = parent_path;
                self.parent_view.entries = parent_entries;
            }
            Err(e) => self.show_notification(e.to_string()),
        }
    }

    fn refresh_right_directory(&mut self, new_path: PathBuf) {
        match get_state_data(&new_path) {
            Ok((entries, parent_path, parent_entries)) => {
                //self.current_path = new_path;
                self.right_entries = entries;
                //self.parent_view.path = parent_path;
                //self.parent_view.entries = parent_entries;
            }
            Err(e) => self.show_notification(e.to_string()),
        }
    }

    /*fn refresh_preview_with_text_file(&mut self, content: String) {
        self.right_entries = PreviewContent::File(FileContent::Text(content));
        self.update_parent_selection();
    }

    fn refresh_preview_with_binary_file(&mut self, content: String) {
        self.right_entries = PreviewContent::File(FileContent::Binary(content));
    }*/

    fn delete_selected(&mut self) {
        if let Some(entry) = self.get_selected_index_entry() {
            let path = self.left_path.join(&entry.name);
            let result = match entry.entry_type {
                FsEntryType::File => fs::remove_file(&path),
                FsEntryType::Directory => fs::remove_dir_all(&path),
            };

            if result.is_ok() {
                self.popup = PopupType::None;
                self.refresh_left_directory(self.left_path.clone());
                self.refresh_right_directory(self.right_path.clone());
            } else if let Err(err) = result {
                self.show_notification(format!("Failed to delete {:?}: {}", path, err));
            }
        }
    }

    fn delete_multiple(&mut self) {
        for path in self.get_selected_paths() {
            let _ = if path.is_file() {
                fs::remove_file(path)
            } else {
                fs::remove_dir_all(path)
            };
        }

        self.refresh_left_directory(self.left_path.clone());
        self.toggle_confirmation_popup();
        self.mode_left = InteractionMode::Normal;
    }

    fn rename_selected(&mut self, input: &mut str) {
        if let Some(entry) = self.get_selected_index_entry() {
            let old_path = self.left_path.join(&entry.name);
            let new_path = self.left_path.join(input.trim_end_matches('/'));

            if fs::rename(&old_path, &new_path).is_ok() {
                self.refresh_left_directory(self.left_path.clone());
                self.input_buffer.clear();
                self.popup = PopupType::None;
            }
        }
    }

    fn create_entry(&mut self, input: String) {
        let is_directory = input.ends_with('/');
        let trimmed_input = input.trim_end_matches('/');
        let mut segments: Vec<&str> = trimmed_input.split('/').collect();

        if let Some(name) = segments.pop() {
            let mut path = self.left_path.clone();
            for segment in segments {
                path.push(segment);
            }

            if let Err(e) = fs::create_dir_all(&path) {
                self.show_notification(format!("Error creating directories: {e}"));
                return;
            }

            path.push(name);

            if is_directory {
                self.create_directory(path);
            } else {
                self.create_file(path);
            }
        }
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
        self.refresh_left_directory(self.left_path.clone());
        self.refresh_right_directory(self.right_path.clone());
        self.input_buffer.clear();
        self.popup = PopupType::None;
    }

    fn set_clipboard_entries(&mut self) {
        if self.mode_left == InteractionMode::Normal
            && !self.left_entries.iter().any(|entry| entry.is_selected)
        {
            if let Some(current_selection) = self.selection_left.selected() {
                if let Some(selected_item) = self.left_entries.get_mut(current_selection) {
                    selected_item.is_selected = true;
                }
            }

            if let Some(entry) = self.get_selected_index_entry() {
                self.clipboard.paths =
                    vec![self.left_path.join(&entry.name).canonicalize().unwrap()];
            }
        } else {
            self.clipboard.paths = self.get_selected_paths();
        }
    }

    fn copy_selected_entries(&mut self) {
        self.clipboard.action = Action::Copy;
        self.set_clipboard_entries();
    }

    fn move_selected_entries(&mut self) {
        self.clipboard.action = Action::Move;
        self.set_clipboard_entries();
    }

    fn paste_clipboard(&mut self) {
        let clipboard = self.clipboard.clone();
        for src in clipboard.paths {
            let dst = self.left_path.join(src.file_name().unwrap());
            if src.is_file() {
                match self.clipboard.action {
                    Action::Move => {
                        if fs::copy(&src, &dst).is_ok() {
                            if let Err(e) = fs::remove_file(&src) {
                                self.show_notification(e.to_string())
                            }
                        };
                    }
                    Action::Copy => {
                        if let Err(e) = fs::copy(src, &dst) {
                            self.show_notification(e.to_string())
                        }
                    }
                    _ => {}
                }
            } else if src.is_dir() {
                match self.clipboard.action {
                    Action::Move => {
                        if let Err(e) = move_file(&src, &dst) {
                            self.show_notification(e.to_string())
                        }
                    }
                    Action::Copy => {
                        if let Err(e) = recursively_copy_dir(&src, &dst) {
                            self.show_notification(e.to_string())
                        }
                    }
                    _ => {}
                }
            }
        }
        self.refresh_left_directory(self.left_path.clone());
        self.clipboard.action = Action::None
    }

    fn get_selected_paths(&self) -> Vec<PathBuf> {
        self.left_entries
            .iter()
            .filter(|entry| entry.is_selected)
            .filter_map(|entry| self.left_path.join(&entry.name).canonicalize().ok())
            .collect()
    }

    fn refresh_right_view(&mut self) {
        // TODO remove
    }

    fn select_current(&mut self) {
        if let InteractionMode::MultiSelect = self.mode_left {
            if let Some(index) = self.selection_left.selected() {
                if let Some(entry) = self.left_entries.get_mut(index) {
                    entry.is_selected = true;
                }
            }
        }
    }

    fn deselect_all(&mut self) {
        if let InteractionMode::Normal = self.mode_left {
            for entry in &mut self.left_entries {
                entry.is_selected = false;
            }
            self.refresh_left_directory(self.left_path.clone());
        }
    }

    fn navigate_down(&mut self) {
        self.selection_left.select_next();
        if self.selection_left.selected().unwrap_or(0) >= self.left_entries.len() {
            self.selection_left.select(Some(0));
        }
        self.select_current();
        self.refresh_right_view();
    }

    fn navigate_up(&mut self) {
        let len = self.left_entries.len();
        if self.selection_left.selected().unwrap_or(0) == 0 {
            self.selection_left.select(Some(len));
        }
        self.selection_left.select_previous();
        self.select_current();
        self.refresh_right_view();
    }

    fn update_parent_selection(&mut self) {
        if let Some(current_name) = self.left_path.file_name().map(|n| n.to_string_lossy()) {
            if let Some(index) = self
                .parent_view
                .entries
                .iter()
                .position(|entry| entry.name == current_name)
            {
                self.parent_view.selection = ListState::default().with_selected(Some(index));
            }
        }
    }

    fn navigate_to_parent(&mut self) {
        if let Some(ref parent_path) = self.parent_view.path {
            self.refresh_left_directory(parent_path.clone());
            self.selection_left = self.parent_view.selection.clone();
            self.refresh_right_view();
        }
    }

    fn navigate_to_child(&mut self) {
        if let Some(entry) = self.get_selected_index_entry() {
            if let FsEntryType::Directory = entry.entry_type {
                let mut path = self.left_path.clone();
                path.push(&entry.name);
                self.refresh_left_directory(path);
                self.parent_view.selection = self.selection_left.clone();
                self.selection_left = ListState::default().with_selected(Some(0));
                self.refresh_right_view();
            }
        }
    }

    fn toggle_confirmation_popup(&mut self) {
        if !self.left_entries.is_empty() {
            self.popup = match self.popup {
                PopupType::Confirm => PopupType::None,
                _ => PopupType::Confirm,
            };
        }
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

    fn get_selected_index_entry(&self) -> Option<&FsEntry> {
        self.selection_left
            .selected()
            .and_then(|index| self.left_entries.get(index))
    }
}

fn main() -> std::io::Result<()> {
    let terminal = ratatui::init();

    let start_dir = PathBuf::from("/home/krase");
    let absolute_path = start_dir.canonicalize().expect("Failed to resolve path");

    let exit_result = FileManager::new(&absolute_path).unwrap().run(terminal);

    ratatui::restore();
    exit_result
}
