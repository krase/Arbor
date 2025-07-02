use crate::utils::{bottom_right_area, format_size, mode_to_string, popup_area};
use crate::{Action, FileManager, FilePane, FsEntryType, InteractionMode, PopupType};
use ratatui::prelude::*;
use ratatui::{
    Frame,
    layout::{Constraint, Flex},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, BorderType::Rounded, Borders, Clear, List, Paragraph, Wrap},
};

impl FilePane {
    fn render(&mut self, f: &mut Frame, clipboard_action: Action, layout: Rect) {
        let cursor_index = self.selection.selected();

        let list_items = Self::list_files(&self.entries, &clipboard_action, cursor_index);

        let block = Block::bordered()
            .border_type(Rounded)
            .borders(Borders::ALL)
            .title(self.path.to_string_lossy());

        let empty_lists = Paragraph::new("No Files")
            .alignment(Alignment::Center)
            .block(block.clone());

        let entry_lists = List::new(list_items)
            .highlight_style(
                Style::default().bg(Color::Blue), //     .fg(Color::Black)
            )
            .add_modifier(Modifier::BOLD)
            .block(block.clone());

        if entry_lists.is_empty() {
            f.render_widget(&empty_lists, layout);
        } else {
            f.render_stateful_widget(entry_lists, layout, &mut self.selection);
        }
    }
}

impl FileManager {
    pub fn render(&mut self, f: &mut Frame) {
        let v_layout =
            Layout::vertical([Constraint::Min(5), Constraint::Length(1)]).split(f.area());

        /*let bars_layout = Layout::horizontal(
            [Constraint::Percentage(50), Constraint::Percentage(50)]
        ).split(v_layout[1]);*/

        let h_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(v_layout[0]);

        self.left_pane
            .render(f, self.clipboard.action.clone(), h_layout[0]);
        self.right_pane
            .render(f, self.clipboard.action.clone(), h_layout[1]);

        let selected_pane = self.selected_pane();
        if let PopupType::Confirm(_m) = &selected_pane.popup {
            Self::render_confirmation(f, selected_pane);
        }

        if let PopupType::Rename = &selected_pane.popup {
            let input = &self.input_buffer;
            let input_paragraph = Paragraph::new(input.clone()).block(
                Block::bordered()
                    .border_type(Rounded)
                    .title("Rename")
                    .blue(),
            );

            let area = popup_area(f.area(), 30, 20);

            f.render_widget(Clear, area);
            f.render_widget(input_paragraph, area);
        }

        if let PopupType::Create = &selected_pane.popup {
            let input = &self.input_buffer;
            let input_paragraph = Paragraph::new(input.clone()).block(
                Block::bordered()
                    .border_type(Rounded)
                    .title("Create:")
                    .blue(),
            );

            let area = popup_area(f.area(), 30, 10);

            f.render_widget(Clear, area);
            f.render_widget(input_paragraph, area);
        }

        // Render notification if available
        if let Some(noti) = &selected_pane.notify {
            let area = bottom_right_area(v_layout[1], 35, 5);

            let block = Block::bordered()
                .border_type(Rounded)
                .title("Notification")
                .style(Style::default().fg(Color::Yellow));

            let text = Paragraph::new(&*noti.message)
                .style(Style::default().fg(Color::Yellow))
                .bg(Color::Black)
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true })
                .block(block);

            f.render_widget(Clear, area);
            f.render_widget(text, area);
        }

        let bottom_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(v_layout[1]);

        let mut size_display = Span::raw("");

        if let Some(entry) = selected_pane.get_selected_index_entry() {
            if entry.entry_type == FsEntryType::File {
                let size = format_size(entry.size);
                size_display = Span::styled(
                    format!(" | Size: {}", size),
                    Style::default().fg(Color::LightMagenta),
                );
            }
        }

        let mode_display = match self.selected_pane().mode {
            InteractionMode::Normal => Span::styled(
                "🔵 Mode: Normal",
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            ),
            InteractionMode::MultiSelect => Span::styled(
                "🟢 Mode: Multi-Select",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        };

        // Combine mode + size
        let combined_info = Line::from(vec![mode_display, size_display]);

        let mode_paragraph = Paragraph::new(combined_info)
            .block(Block::default().borders(Borders::NONE))
            .alignment(Alignment::Left);

        f.render_widget(mode_paragraph, bottom_layout[0]);

        let mut per_display = Span::raw("");
        if let Some(entry) = selected_pane.get_selected_index_entry() {
            let permission = entry.file_permission;

            let permisson_str = mode_to_string(permission);
            per_display = Span::styled(
                format!("Permisson: {} ", permisson_str),
                Style::default().fg(Color::LightCyan),
            );
        }

        let per_paragraph = Paragraph::new(per_display)
            .block(Block::default().borders(Borders::NONE))
            .alignment(Alignment::Right);

        f.render_widget(per_paragraph, bottom_layout[1]);
    }

    fn render_confirmation(f: &mut Frame, selected_pane: &FilePane) {
        if let PopupType::Confirm(msg) = &selected_pane.popup {
            let mut confirm_file_list = Paragraph::new("").wrap(Wrap { trim: false });

            match selected_pane.mode {
                InteractionMode::Normal => {
                    if let Some(index) = selected_pane.selection.selected() {
                        if let Some(file) = selected_pane.entries.get(index) {
                            let name = file.name.clone();
                            let path = selected_pane.path.join(name).to_string_lossy().to_string();

                            confirm_file_list = Paragraph::new(path)
                                .alignment(Alignment::Left)
                                .wrap(Wrap { trim: false });
                        }
                    }
                }

                InteractionMode::MultiSelect => {
                    let selected_field = selected_pane.get_selected_paths();
                    let mut text = vec![Line::from("")];
                    for file in selected_field {
                        text.push(Line::from(file.to_string_lossy().to_string()));
                    }

                    confirm_file_list = Paragraph::new(text)
                        .alignment(Alignment::Left)
                        .wrap(Wrap { trim: false });
                }
            };

            let block = Block::bordered()
                .border_type(Rounded)
                .title(format!("Confirm your action: {}", msg))
                .blue();
            let area = popup_area(f.area(), 37, 40);

            let inner_area = block.inner(area);

            let popup_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints(vec![
                    Constraint::Percentage(90),
                    Constraint::Length(1),
                    Constraint::Percentage(10),
                ])
                .split(inner_area);

            let sub_section = Layout::default()
                .direction(Direction::Vertical)
                .constraints(vec![Constraint::Percentage(100)])
                .split(popup_layout[0]);

            let separator = Paragraph::new(Span::styled(
                "─".repeat(popup_layout[1].width as usize),
                Style::default().fg(Color::LightBlue),
            ));

            let seperator_layout = Layout::horizontal([Constraint::Percentage(95)])
                .flex(Flex::Center)
                .split(popup_layout[1]);

            let options = Paragraph::new("Yes(Y)")
                .block(Block::default().borders(Borders::NONE))
                .alignment(ratatui::layout::Alignment::Center);
            let options1 = Paragraph::new("No(N)")
                .block(Block::default().borders(Borders::NONE))
                .alignment(ratatui::layout::Alignment::Center);

            let section2 = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(popup_layout[2]);

            f.render_widget(Clear, area);
            f.render_widget(block, area);
            f.render_widget(confirm_file_list, sub_section[0]);
            f.render_widget(separator, seperator_layout[0]);
            f.render_widget(options, section2[0]);
            f.render_widget(options1, section2[1]);
        }
    }
}
