use crate::{FsEntry, FsEntryType};
use ratatui::{
    layout::{Constraint, Flex, Layout, Rect},
    style::{Color, Style},
    text::Span,
    widgets::ListItem,
};

use std::{fs, io};
use std::{os::unix::fs::MetadataExt, path::PathBuf};
use std::cmp::Ordering;

impl FsEntry {
    pub fn is_file(&self) -> bool {
        self.entry_type == FsEntryType::File
    }
    pub fn is_dir(&self) -> bool {
        self.entry_type == FsEntryType::Directory
    }
}

pub fn list_dir(p: &PathBuf) -> std::io::Result<Vec<FsEntry>> {
    let mut items = Vec::new();
    for entry in fs::read_dir(p)? {
        let entry = entry?;
        let meta = entry.metadata()?;
        let file_size = meta.size();
        let permission: u32 = meta.mode();
        let file_type = if meta.is_dir() {
            FsEntryType::Directory
        } else {
            FsEntryType::File
        };
        let item = FsEntry {
            name: entry.file_name().into_string().unwrap(),
            entry_type: file_type,
            size: file_size,
            file_permission: permission,
            is_selected: false,
        };
        items.push(item);
    }
    // Sort by type and by name. Directories come first
    items.sort_by(|a, b| match (a.is_dir(), b.is_dir()) {
        (true, true) => a.name.cmp(&b.name),
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        (false, false) => a.name.cmp(&b.name),
    });

    Ok(items)
}

#[allow(unused)]
pub fn recursively_copy_dir(src: &PathBuf, dst: &PathBuf) -> io::Result<()> {
    fs::create_dir_all(dst)?;

    if let Ok(entries) = fs::read_dir(src) {
        for entry in entries.flatten() {
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());

            if src_path.is_file() {
                fs::copy(&src_path, &dst_path)?;
            } else if src_path.is_dir() {
                recursively_copy_dir(&src_path, &dst_path)?;
            }
        }
    }
    Ok(())
}

#[allow(unused)]
pub fn move_file(src: &PathBuf, dst: &PathBuf) -> io::Result<()> {
    let result = recursively_copy_dir(src, dst);
    if result.is_ok() {
        if src.is_file() {
            fs::remove_file(src)?
        } else {
            fs::remove_dir_all(src)?
        }
    }
    result
}

#[allow(unused)]
pub fn read_valid_file(path: &PathBuf) -> io::Result<String> {
    if fs::metadata(path)?.len() == 0 {
        Ok("Empty File".to_string())
    } else {
        fs::read_to_string(path)
    }
}

pub fn mode_to_string(mode: u32) -> String {
    let mut result = String::new();

    // Each tuple is (bitmask, char to use if bit is set)
    let flags = [
        (0o400, 'r'),
        (0o200, 'w'),
        (0o100, 'x'), // user
        (0o040, 'r'),
        (0o020, 'w'),
        (0o010, 'x'), // group
        (0o004, 'r'),
        (0o002, 'w'),
        (0o001, 'x'), // others
    ];

    for (bit, ch) in flags {
        if mode & bit != 0 {
            result.push(ch);
        } else {
            result.push('-');
        }
    }

    result
}

pub fn get_state_data(start: &PathBuf) -> std::io::Result<Vec<FsEntry>> {
    let entries = list_dir(start)?;
    //let parent_path = start.parent().map(|p| p.to_path_buf());
    Ok(entries)
}

pub fn format_size(size: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    let size_f = size as f64;

    if size_f >= GB {
        format!("{:.2} GB", size_f / GB)
    } else if size_f >= MB {
        format!("{:.2} MB", size_f / MB)
    } else if size_f >= KB {
        format!("{:.2} KB", size_f / KB)
    } else {
        format!("{} B", size)
    }
}

//PopUp UI constructor
pub fn popup_area(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::vertical([Constraint::Percentage(percent_y)]).flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Percentage(percent_x)]).flex(Flex::Center);
    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}

pub fn bottom_right_area(area: Rect, width: u16, height: u16) -> Rect {
    let x = area.x + area.width.saturating_sub(width);
    let y = area.y + area.height.saturating_sub(height);
    Rect::new(x, y, width, height)
}
