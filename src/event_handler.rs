use crate::{FileManager, PopupType, Selected};
use crossterm::event::{self, Event, KeyCode, KeyModifiers, ModifierKeyCode};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::DefaultTerminal;
use std::io;
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
                    match popup {
                        PopupType::Delete(_) => {
                            match key.code {
                                KeyCode::Char('n') => {
                                    self.close_confirmation_popup();
                                }
                                KeyCode::Char('y') => self.delete_selected(),
                                _ => {}
                            }
                            continue;
                        }
                        PopupType::Rename(_) => {
                            match key.code {
                                KeyCode::Char(input) => {
                                    self.input_buffer.push(input);
                                }
                                // Append character to input
                                KeyCode::Backspace => {
                                    self.input_buffer.pop();
                                } // Remove last character
                                KeyCode::Enter => {
                                    let tmp = self.input_buffer.clone();
                                    self.input_buffer.clear();
                                    self.rename_selected(tmp.as_str());
                                }
                                KeyCode::Esc => self.close_confirmation_popup(),
                                _ => {}
                            }
                            continue;
                        }
                        PopupType::Create(_) => {
                            match key.code {
                                KeyCode::Char(c) => {
                                    self.input_buffer.push(c);
                                }
                                // Append character to input
                                KeyCode::Backspace => {
                                    self.input_buffer.pop();
                                } // Remove last character
                                KeyCode::Enter => {
                                    self.create_entry(self.input_buffer.clone());
                                }
                                KeyCode::Esc => self.close_confirmation_popup(),
                                _ => {}
                            }
                            continue;
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
                            continue;
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
                            continue;
                        }
                        PopupType::None => {}
                    }
                    match key.code {
                        KeyCode::Tab => {
                            self.handle_tab_normal_mode();
                        }
                        KeyCode::F(10) => break,
                        KeyCode::Down => self.selected_pane_mut().navigate_down(),
                        KeyCode::Up => self.selected_pane_mut().navigate_up(),
                        KeyCode::Left | KeyCode::Backspace => {
                            self.selected_pane_mut().navigate_to_parent()
                        }
                        KeyCode::Right | KeyCode::Enter => {
                            self.selected_pane_mut().navigate_to_child()
                        }
                        KeyCode::F(8) => {
                            self.popup = PopupType::Delete("Delete item".to_string());
                        }
                        /* SHIFT + F(2)  => {
                            if !self.selected_pane().entries.is_empty() {
                                self.popup = PopupType::Rename
                            }
                        }*/
                        KeyCode::F(2) => {
                            self.popup = PopupType::Create("Create Item".to_string());
                        }
                        // TODO Copy/Move from active pane to the inactive
                        KeyCode::F(5) => {
                            self.popup = PopupType::Copy("Copy selected".to_string());
                        }
                        KeyCode::F(6) => self.popup = PopupType::Move("Move selected".to_string()),
                        KeyCode::Esc => self.selected_pane_mut().deselect_all(),
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
                        //TODO
                        // F9 Menu
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
            }
        }

        Ok(())
    }

    fn handle_tab_normal_mode(&mut self) {
        // Switch between left and right pane
        self.selected_pane = if self.selected_pane == Selected::Left {
            self.left_pane.old_curser_pos = self.left_pane.selection.selected();
            self.left_pane.selection.select(None);
            if self.right_pane.old_curser_pos.is_none() {
                self.right_pane.selection.select_first();
            } else {
                self.right_pane
                    .selection
                    .select(self.right_pane.old_curser_pos);
            }
            Selected::Right
        } else {
            self.right_pane.old_curser_pos = self.right_pane.selection.selected();
            self.right_pane.selection.select(None);
            if self.left_pane.old_curser_pos.is_none() {
                self.left_pane.selection.select_first();
            } else {
                self.left_pane
                    .selection
                    .select(self.left_pane.old_curser_pos);
            }
            Selected::Left
        }
    }
}
