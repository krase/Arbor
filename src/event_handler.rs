use crate::{FileManager, InteractionMode, PopupType, Selected};
use crossterm::event::{self, Event, KeyCode, ModifierKeyCode};
use ratatui::DefaultTerminal;
use std::io;
use std::time::Duration;
impl FileManager {
    pub fn run(mut self, mut terminal: DefaultTerminal) -> io::Result<()> {
        let poll_interval = Duration::from_millis(200);
        self.left_pane.selection.select(None);

        loop {
            terminal.draw(|f| self.render(f))?;

            //let mut selected_pane = self.selected_pane.borrow_mut();
            let popup = self.selected_pane().popup.clone();
            let mode = self.selected_pane().mode.clone();

            if event::poll(poll_interval)? {
                if let Event::Key(key) = event::read()? {
                    if let PopupType::Confirm(_) = popup {
                        match key.code {
                            KeyCode::Char('n') => {
                                self.selected_pane_mut().close_confirmation_popup();
                            }
                            KeyCode::Char('y') => match mode {
                                InteractionMode::Normal => {
                                    self.selected_pane_mut().delete_selected()
                                }
                                InteractionMode::MultiSelect => {
                                    self.selected_pane_mut().delete_multiple()
                                }
                            },
                            _ => {}
                        }
                        continue;
                    }

                    if let PopupType::Rename = popup {
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
                                self.selected_pane_mut().rename_selected(tmp.as_str());
                            }
                            KeyCode::Esc => self.selected_pane_mut().popup = PopupType::None,
                            _ => {}
                        }
                        continue;
                    }

                    if let PopupType::Create = popup {
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
                            KeyCode::Esc => self.selected_pane_mut().popup = PopupType::None,
                            _ => {}
                        }
                        continue;
                    }
                    if let InteractionMode::Normal = mode {
                        match key.code {
                            KeyCode::Tab => {
                                // Switch between left and right pane
                                self.selected_pane = if self.selected_pane == Selected::Left {
                                    self.left_pane.old_curser_pos =
                                        self.left_pane.selection.selected();
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
                                    self.right_pane.old_curser_pos =
                                        self.right_pane.selection.selected();
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
                            KeyCode::Char('q') | KeyCode::F(10) => break,
                            KeyCode::Char('j') | KeyCode::Down => {
                                self.selected_pane_mut().navigate_down()
                            }
                            KeyCode::Char('k') | KeyCode::Up => {
                                self.selected_pane_mut().navigate_up()
                            }
                            KeyCode::Char('h') | KeyCode::Left | KeyCode::Backspace => {
                                self.selected_pane_mut().navigate_to_parent()
                            }
                            KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => {
                                self.selected_pane_mut().navigate_to_child()
                            }
                            KeyCode::Char('d') | KeyCode::F(8) => {
                                self.selected_pane_mut()
                                    .open_confirmation_popup("Delete item");
                            }
                            KeyCode::Char('r') => {
                                if !self.selected_pane().entries.is_empty() {
                                    self.selected_pane_mut().popup = PopupType::Rename
                                }
                            }
                            KeyCode::Char('a') | KeyCode::F(2) => {
                                self.selected_pane_mut().popup = PopupType::Create
                            }
                            // TODO Copy/Move from active pane to the inactive
                            KeyCode::Char('y') | KeyCode::F(5) => self.copy_selected_entries(),
                            KeyCode::Char('x') | KeyCode::F(6) => self.move_selected_entries(),
                            KeyCode::Char('p') => self.paste_clipboard(),
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
                            KeyCode::Char('v') => {
                                self.selected_pane_mut().mode = InteractionMode::MultiSelect;
                                if let InteractionMode::MultiSelect = mode {
                                    if let Some(current_selection) =
                                        self.selected_pane().selection.selected()
                                    {
                                        if let Some(selected_item) = self
                                            .selected_pane_mut()
                                            .entries
                                            .get_mut(current_selection)
                                        {
                                            selected_item.is_selected = true;
                                        }
                                    }
                                }
                            }
                            // F9 Menu
                            // F1 Hilfe
                            // F2 Function Menu
                            // F3 View
                            // F4 Edit
                            // F9 Menu Bar
                            _ => {}
                        }
                    }
                    if let InteractionMode::MultiSelect = self.selected_pane().mode {
                        match key.code {
                            KeyCode::Char('j') | KeyCode::Down => {
                                self.selected_pane_mut().navigate_down()
                            }
                            KeyCode::Char('k') | KeyCode::Up => {
                                self.selected_pane_mut().navigate_up()
                            }
                            KeyCode::Char('d') => {
                                self.selected_pane_mut()
                                    .open_confirmation_popup("Delete multiple");
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
                            KeyCode::Char('q') => break,
                            KeyCode::Esc => self.selected_pane_mut().mode = InteractionMode::Normal,
                            _ => {}
                        }
                    }
                }
            } else {
                self.left_pane.clear_expired_notifications();
                self.right_pane.clear_expired_notifications();
            }
        }

        Ok(())
    }
}
