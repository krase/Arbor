use crate::{FileManager, PopupType, Selected};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers, ModifierKeyCode};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::DefaultTerminal;
use std::io;
use std::ops::Add;
use std::time::Duration;

impl FileManager {
    pub fn run(mut self, mut terminal: DefaultTerminal) -> io::Result<()> {
        let poll_interval = Duration::from_millis(200);
        self.left_pane.selection.select(None);

        loop {
            terminal.draw(|f| self.render(f))?;

            if event::poll(poll_interval)? {
                if let Event::Key(key) = event::read()? {
                    if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('o') {}
                    let popup = self.popup.clone();

                    if self.handle_popups(key, popup) { continue; }

                    match key.code {
                        KeyCode::Tab => {
                            self.handle_tab();
                        }
                        KeyCode::F(10) => break,
                        KeyCode::Down => self.selected_pane_mut().navigate_down(),
                        KeyCode::Up => {
                            if key.modifiers == KeyModifiers::ALT {
                                self.selected_pane_mut().navigate_to_parent();
                            } else {
                                self.selected_pane_mut().navigate_up();
                            }
                        },
                        KeyCode::Left=> {
                            self.selected_pane_mut().navigate_to_parent()
                        }
                        KeyCode::Right | KeyCode::Enter => {
                            self.selected_pane_mut().navigate_to_child()
                        }
                        KeyCode::F(8) => {
                            self.popup = PopupType::Delete("Delete item".to_string());
                        }
                        KeyCode::F(2) => {
                            self.popup = PopupType::Create("Create Item".to_string());
                        }
                        KeyCode::F(5) => {
                            self.popup = PopupType::Copy("Copy selected".to_string());
                        }
                        KeyCode::F(6) => {
                            if let Some(entry) = self.selected_pane().get_selected_index_entry() {
                                self.input_buffer = entry.name.clone();
                                self.cursor_pos = self.input_buffer.chars().count() as u16;
                            }
                            if key.modifiers == KeyModifiers::SHIFT {
                                if !self.selected_pane().entries.is_empty() {
                                    self.popup = PopupType::Rename("Rename item".to_string());
                                }
                            } else {
                                self.popup = PopupType::Move("Move selected".to_string())
                            }
                        },
                        KeyCode::Esc => {
                            self.selected_pane_mut().deselect_all();
                            self.search_prefix = "".to_string();
                        },
                        KeyCode::Backspace => {
                            self.search_prefix.pop();
                        }
                        KeyCode::Char(' ') => {
                            if let Some(current_selection) =
                                self.selected_pane().selection.selected()
                            {
                                if let Some(selected_item) =
                                    self.selected_pane_mut().entries.get_mut(current_selection)
                                {
                                    selected_item.is_selected ^= true;
                                }
                            }
                        }
                        KeyCode::Char(c) => {
                            //println!("--- {}", c);
                            self.search_prefix += format!("{}", c).as_str();
                            let entries = self.selected_pane().entries.iter().enumerate().find(|(pos, x)| {
                                x.name.starts_with(self.search_prefix.as_str())
                            });
                            //println!("{:?} {}", entries, &self.search_index);
                            if let Some((pos,x)) = entries {
                                self.selected_pane_mut().selection.select(Some(pos));
                            }
                            self.show_notification(format!("{}", self.search_prefix));
                        }
                        //TODO
                        // F1 Help
                        // F2 Function Menu
                        // F3 View
                        // F4 Edit
                        // F9 Menu Bar
                        _ => {}
                    }
                }
            } else {
                self.left_pane.clear_expired_notifications();
                self.right_pane.clear_expired_notifications();
                self.clear_expired_notifications();
            }
        }

        Ok(())
    }

    fn byte_index(&self, buffer: &String, pos: usize) -> usize {
        buffer
            .char_indices()
            .map(|(i, _)| i)
            .nth(pos)
            .unwrap_or(buffer.len())
    }

    fn insert_char(&mut self, new_char: char) {
        let index = self.byte_index(&self.input_buffer, self.cursor_pos as usize);
        self.input_buffer.insert(index, new_char);
        self.move_cursor_right();
    }

    fn move_cursor_left(&mut self) {
        self.cursor_pos = self.cursor_pos.saturating_sub(1);
        self.cursor_pos = self.cursor_pos.clamp(0, self.input_buffer.chars().count() as u16);
    }
    
    fn move_cursor_right(&mut self) {
        self.cursor_pos = self.cursor_pos.saturating_add(1);
        self.cursor_pos = self.cursor_pos.clamp(0, self.input_buffer.chars().count() as u16);
    }

    fn delete_char(&mut self) {
        let is_not_cursor_leftmost = self.cursor_pos != 0;
        if is_not_cursor_leftmost {
            // Method "remove" is not used on the saved text for deleting the selected char.
            // Reason: Using remove on String works on bytes instead of the chars.
            // Using remove would require special care because of char boundaries.

            let current_pos = self.cursor_pos as usize;
            let from_left_to_current_index = current_pos - 1;

            // Getting all characters before the selected character.
            let before_char_to_delete = self.input_buffer.chars().take(from_left_to_current_index);
            // Getting all characters after selected character.
            let after_char_to_delete = self.input_buffer.chars().skip(current_pos);

            // Put all characters together except the selected one.
            // By leaving the selected one out, it is forgotten and therefore deleted.
            self.input_buffer = before_char_to_delete.chain(after_char_to_delete).collect();
        }
    }


    fn handle_popups(&mut self, key: KeyEvent, popup: PopupType) -> bool {
        match popup {
            PopupType::Create(_) | PopupType::Rename(_) => {
                match key.code {
                    KeyCode::Left => {
                        self.move_cursor_left();
                    }
                    KeyCode::Right => {
                        self.move_cursor_right();
                    }
                    KeyCode::Backspace => {
                        self.delete_char();
                        self.move_cursor_left();
                    } // Remove last character
                    KeyCode::Delete => {
                        if (self.cursor_pos as usize) < self.input_buffer.len() {
                            self.move_cursor_right();
                            self.delete_char();
                            self.move_cursor_left();
                        }
                    }
                    KeyCode::Char(input) => {
                        // Append character to input
                        self.insert_char(input);
                    }
                    KeyCode::Enter => {
                        let tmp = self.input_buffer.clone();
                        self.input_buffer.clear();
                        self.rename_selected(tmp.as_str());
                    }
                    KeyCode::Esc => self.close_confirmation_popup(),
                    _ => {}
                }
                return true;
            }
            PopupType::Delete(_) => {
                match key.code {
                    KeyCode::Char('n') => {
                        self.close_confirmation_popup();
                    }
                    KeyCode::Char('y') => self.delete_selected(),
                    _ => {}
                }
                return true;
            }
            PopupType::Copy(_) => {
                match key.code {
                    KeyCode::Char('n') => {
                        self.close_confirmation_popup();
                    }
                    KeyCode::Char('y') => {
                        self.copy_selected_to_other_pane();
                    }
                    _ => {}
                }
                return true;
            }
            PopupType::Move(_) => {
                match key.code {
                    KeyCode::Char('n') => {
                        self.close_confirmation_popup();
                    }
                    KeyCode::Char('y') => {
                        self.move_selected_to_other_pane();
                    }
                    _ => {}
                }
                return true;
            }
            PopupType::None => {}
        }
        false
    }

    fn handle_tab(&mut self) {
        // Switch between left and right pane
        self.selected_pane = if self.selected_pane == Selected::Left {
            self.left_pane.old_selection = self.left_pane.selection.clone();
            self.left_pane.selection.select(None);
            *self.left_pane.selection.offset_mut() = self.left_pane.old_selection.offset();
            if self.right_pane.old_selection.selected().is_none() {
                self.right_pane.selection.select_first();
            } else {
                self.right_pane
                    .selection
                    .select(self.right_pane.old_selection.selected());
            }
            Selected::Right
        } else {
            self.right_pane.old_selection = self.right_pane.selection.clone();
            self.right_pane.selection.select(None);
            *self.right_pane.selection.offset_mut() = self.right_pane.old_selection.offset();
            if self.left_pane.old_selection.selected().is_none() {
                self.left_pane.selection.select_first();
            } else {
                self.left_pane
                    .selection
                    .select(self.left_pane.old_selection.selected());
            }
            Selected::Left
        }
    }
}
