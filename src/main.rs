mod event_handler;
use std::time::{Duration, Instant};
mod ui;
mod utils;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use std::{fs, path::PathBuf, sync::mpsc, thread};
use std::cell::RefCell;
use std::mem::zeroed;
use std::rc::Rc;
use std::sync::Arc;
use ratatui::Frame;
use ratatui::layout::Alignment;
use ratatui::prelude::{Color, Line, Modifier, Span, Style};
use ratatui::text::ToText;
use ratatui::widgets::BorderType::Rounded;
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

#[derive(Debug, Clone)]
pub struct FilePane {
    path: PathBuf,
    entries: Vec<FsEntry>,
    selection: ListState,
    mode: InteractionMode,
    notify: Option<Notification>,
    popup: PopupType,
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

    clipboard: Clipboard,
    
    input_buffer: String,
    notify: Option<Notification>,
}   
 

impl FilePane {
    fn new(path: PathBuf, entries: Vec<FsEntry>) -> Self {
        Self {
            path,
            entries,
            selection: ListState::default().with_selected(Some(0)),
            mode: InteractionMode::Normal,
            notify: None,
            popup: PopupType::None,
        }
    }


    fn list_files<'a>(current_entries: &'a Vec<FsEntry>, clipboard_action: &Action, cursor_index: Option<usize>) -> Vec<ListItem<'a>> {
        let list_current_items: Vec<ListItem> = current_entries
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let (bar, bar_style) = if entry.is_selected {
                    match clipboard_action {
                        Action::Move => ("▌", Style::default().fg(Color::Red)),
                        Action::Copy => ("▌", Style::default().fg(Color::Green)),
                        Action::None => ("▌", Style::default().fg(Color::Yellow)),
                    }
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
            Ok((entries, parent_path)) => {
                self.path = new_path;
                self.entries = entries;
            }
            Err(e) => self.show_notification(e.to_string()),
        }
    }
    
    fn refresh_directory(&mut self) {
        match get_state_data(&self.path) {
            Ok((entries, parent_path)) => {
                self.entries = entries;
            }
            Err(e) => self.show_notification(e.to_string()),
        }
    }

    fn navigate_down(&mut self) {
        self.selection.select_next();
        if self.selection.selected().unwrap_or(0) >= self.entries.len() {
            self.selection.select(Some(0));
        }
        self.select_current();
    }

    fn navigate_up(&mut self) {
        let len = self.entries.len();
        if self.selection.selected().unwrap_or(0) == 0 {
            self.selection.select(Some(len));
        }
        self.selection.select_previous();
        self.select_current();
    }

    fn get_selected_paths(&self) -> Vec<PathBuf> {
        self.entries
            .iter()
            .filter(|entry| entry.is_selected)
            .filter_map(|entry| self.path.join(&entry.name).canonicalize().ok())
            .collect()
    }

    fn select_current(&mut self) {
        if let InteractionMode::MultiSelect = self.mode {
            if let Some(index) = self.selection.selected() {
                if let Some(entry) = self.entries.get_mut(index) {
                    entry.is_selected = true;
                }
            }
        }
    }

    fn deselect_all(&mut self) {
        if let InteractionMode::Normal = self.mode {
            for entry in &mut self.entries {
                entry.is_selected = false;
            }
            self.refresh_directory();
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

        self.refresh_directory();
        self.toggle_confirmation_popup();
        self.mode = InteractionMode::Normal;
    }

    fn toggle_confirmation_popup(&mut self) {
        if !self.entries.is_empty() {
            self.popup = match self.popup {
                PopupType::Confirm => PopupType::None,
                _ => PopupType::Confirm,
            };
        }
    }

    fn get_selected_index_entry(&self) -> Option<&FsEntry> {
        self.selection
            .selected()
            .and_then(|index| self.entries.get(index))
    }

    fn delete_selected(&mut self) {
        if let Some(entry) = self.get_selected_index_entry() {
            let path = self.path.join(&entry.name);
            let result = match entry.entry_type {
                FsEntryType::File => fs::remove_file(&path),
                FsEntryType::Directory => fs::remove_dir_all(&path),
            };

            if result.is_ok() {
                self.popup = PopupType::None;
                self.refresh_directory();
            } else if let Err(err) = result {
                self.show_notification(format!("Failed to delete {:?}: {}", path, err));
            }
        }
    }


    fn rename_selected(&mut self, input: &str) {
        if let Some(entry) = self.get_selected_index_entry() {
            let old_path = self.path.join(&entry.name);
            let new_path = self.path.join(input.trim_end_matches('/'));

            if fs::rename(&old_path, &new_path).is_ok() {
                self.refresh_directory();
                // TODO Where to hold the input buffer of the popup?
                // self.input_buffer.clear();
                self.popup = PopupType::None;
            }
        }
    }

    fn navigate_to_parent(&mut self) {
        if let Some(parent_path) = self.path.parent() {
            self.enter_directory (parent_path.to_path_buf());
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


    /*
    fn set_clipboard_entries(&mut self) {
        if self.left_pane.mode == InteractionMode::Normal
            && !self.left_pane.entries.iter().any(|entry| entry.is_selected)
        {
            if let Some(current_selection) = self.left_pane.selection.selected() {
                if let Some(selected_item) = self.left_pane.entries.get_mut(current_selection) {
                    selected_item.is_selected = true;
                }
            }

            if let Some(entry) = self.get_selected_index_entry() {
                self.clipboard.paths = vec![
                    self.left_pane
                        .path
                        .join(&entry.name)
                        .canonicalize()
                        .unwrap(),
                ];
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
*/
} // FilePane

impl FileManager {
    fn new(start_path: &PathBuf) -> Result<Self, std::io::Error> {
        let (left_entries, parent_path) = get_state_data(start_path).unwrap();
        let (right_entries, _) = get_state_data(start_path).unwrap();

        let state = Self {
            left_pane: FilePane::new(start_path.clone(), left_entries),
            right_pane: FilePane::new(start_path.clone(), right_entries),

            clipboard: Clipboard {
                paths: vec![],
                action: Action::None,
            },
            input_buffer: String::new(),
            selected_pane: Selected::Right,
            notify: None,
        };

        Ok(state)
    }

    fn selected_pane(&self) -> &FilePane {
        if self.selected_pane == Selected::Left {
            &self.left_pane
        } else  {
            &self.right_pane
        }
    }

    fn selected_pane_mut(&mut self) -> &mut FilePane {
        if self.selected_pane == Selected::Left {
            &mut self.left_pane
        } else  {
            &mut self.right_pane
        }
    }


    /*fn refresh_preview_with_text_file(&mut self, content: String) {
        self.right_entries = PreviewContent::File(FileContent::Text(content));
        self.update_parent_selection();
    }

    fn refresh_preview_with_binary_file(&mut self, content: String) {
        self.right_entries = PreviewContent::File(FileContent::Binary(content));
    }*/


    fn create_entry(&mut self, input: String) {
        let is_directory = input.ends_with('/');
        let trimmed_input = input.trim_end_matches('/');
        let mut segments: Vec<&str> = trimmed_input.split('/').collect();

        if let Some(name) = segments.pop() {
            let mut path = self.left_pane.path.clone();
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
        //TODO how to communicate popup stuff?
        //self.popup = PopupType::None;
    }

    fn show_notification<S: AsRef<str>>(&mut self, message: S) {
        self.notify = Some(Notification {
            message: message.as_ref().to_string(),
            created_at: Instant::now(),
            duration: Duration::from_secs(3),
        });
    }


    fn paste_clipboard(&mut self) {
        let clipboard = self.clipboard.clone();
        for src in clipboard.paths {
            let dst = self.selected_pane().path.join(src.file_name().unwrap());
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
        self.selected_pane_mut().refresh_directory();
        
        self.clipboard.action = Action::None
    }
    
}  // FileManager

fn main() -> std::io::Result<()> {
    let terminal = ratatui::init();

    let start_dir = PathBuf::from("/home/krase");
    let absolute_path = start_dir.canonicalize().expect("Failed to resolve path");

    let exit_result = FileManager::new(&absolute_path).unwrap().run(terminal);
    
    ratatui::restore();
    exit_result
}
