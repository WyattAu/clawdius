//! Vim-style keybindings for TUI

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VimMode {
    #[default]
    Normal,
    Insert,
    Visual,
    Command,
}

#[derive(Debug, Clone)]
pub enum VimAction {
    ChangeMode(VimMode),
    Move(Motion),
    Delete(Motion),
    Yank(Motion),
    Put(Placement),
    Search(String),
    Command(String),
    Scroll(ScrollDirection),
    Undo,
    Redo,
    Quit,
    Save,
    SubmitInput,
    None,
}

#[derive(Debug, Clone)]
pub enum Motion {
    CharLeft,
    CharRight,
    LineUp,
    LineDown,
    WordForward,
    WordBackward,
    LineStart,
    LineEnd,
    FileStart,
    FileEnd,
    Line(usize),
}

#[derive(Debug, Clone)]
pub enum ScrollDirection {
    HalfPageUp,
    HalfPageDown,
    PageUp,
    PageDown,
}

#[derive(Debug, Clone)]
pub enum Placement {
    Before,
    After,
}

pub struct VimKeymap {
    mode: VimMode,
    pending_keys: Vec<KeyEvent>,
    command_buffer: String,
    search_buffer: String,
}

impl Default for VimKeymap {
    fn default() -> Self {
        Self::new()
    }
}

impl VimKeymap {
    pub fn new() -> Self {
        Self {
            mode: VimMode::Normal,
            pending_keys: Vec::new(),
            command_buffer: String::new(),
            search_buffer: String::new(),
        }
    }

    pub fn mode(&self) -> VimMode {
        self.mode
    }

    pub fn mode_indicator(&self) -> &'static str {
        match self.mode {
            VimMode::Normal => "-- NORMAL --",
            VimMode::Insert => "-- INSERT --",
            VimMode::Visual => "-- VISUAL --",
            VimMode::Command => "-- COMMAND --",
        }
    }

    pub fn handle(&mut self, key: KeyEvent) -> VimAction {
        match self.mode {
            VimMode::Normal => self.handle_normal(key),
            VimMode::Insert => self.handle_insert(key),
            VimMode::Visual => self.handle_visual(key),
            VimMode::Command => self.handle_command(key),
        }
    }

    fn handle_normal(&mut self, key: KeyEvent) -> VimAction {
        if !self.pending_keys.is_empty() {
            return self.handle_pending(key);
        }

        match key.code {
            KeyCode::Char('h') => VimAction::Move(Motion::CharLeft),
            KeyCode::Char('j') => VimAction::Move(Motion::LineDown),
            KeyCode::Char('k') => VimAction::Move(Motion::LineUp),
            KeyCode::Char('l') => VimAction::Move(Motion::CharRight),
            KeyCode::Char('w') => VimAction::Move(Motion::WordForward),
            KeyCode::Char('b') => VimAction::Move(Motion::WordBackward),
            KeyCode::Char('0') => VimAction::Move(Motion::LineStart),
            KeyCode::Char('$') => VimAction::Move(Motion::LineEnd),
            KeyCode::Char('g') => {
                self.pending_keys.push(key);
                VimAction::None
            },
            KeyCode::Char('G') => VimAction::Move(Motion::FileEnd),
            KeyCode::Char('i') => VimAction::ChangeMode(VimMode::Insert),
            KeyCode::Char('a') => {
                self.pending_keys.push(key);
                VimAction::None
            },
            KeyCode::Char('o') => {
                self.pending_keys.push(key);
                VimAction::None
            },
            KeyCode::Char('O') => {
                self.pending_keys.push(key);
                VimAction::None
            },
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                VimAction::Scroll(ScrollDirection::HalfPageDown)
            },
            KeyCode::Char('d') => {
                self.pending_keys.push(key);
                VimAction::None
            },
            KeyCode::Char('y') => {
                self.pending_keys.push(key);
                VimAction::None
            },
            KeyCode::Char('p') => VimAction::Put(Placement::After),
            KeyCode::Char('P') => VimAction::Put(Placement::Before),
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                VimAction::Scroll(ScrollDirection::HalfPageUp)
            },
            KeyCode::Char('u') => VimAction::Undo,
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => VimAction::Redo,
            KeyCode::Char('r') => {
                self.pending_keys.push(key);
                VimAction::None
            },
            KeyCode::Char(':') => {
                self.mode = VimMode::Command;
                self.command_buffer.clear();
                VimAction::ChangeMode(VimMode::Command)
            },
            KeyCode::Char('/') => {
                self.mode = VimMode::Command;
                self.search_buffer.clear();
                self.search_buffer.push('/');
                VimAction::ChangeMode(VimMode::Command)
            },
            KeyCode::Char('z') => {
                self.pending_keys.push(key);
                VimAction::None
            },
            KeyCode::Char('q') => VimAction::Quit,
            KeyCode::Up => VimAction::Move(Motion::LineUp),
            KeyCode::Down => VimAction::Move(Motion::LineDown),
            KeyCode::Left => VimAction::Move(Motion::CharLeft),
            KeyCode::Right => VimAction::Move(Motion::CharRight),
            KeyCode::PageUp => VimAction::Scroll(ScrollDirection::PageUp),
            KeyCode::PageDown => VimAction::Scroll(ScrollDirection::PageDown),
            _ => VimAction::None,
        }
    }

    fn handle_pending(&mut self, key: KeyEvent) -> VimAction {
        let pending = self.pending_keys.clone();
        self.pending_keys.clear();

        match pending.as_slice() {
            [KeyEvent {
                code: KeyCode::Char('g'),
                ..
            }] => match key.code {
                KeyCode::Char('g') => VimAction::Move(Motion::FileStart),
                KeyCode::Char('e') => VimAction::Move(Motion::WordBackward),
                _ => VimAction::None,
            },
            [KeyEvent {
                code: KeyCode::Char('a'),
                ..
            }] => {
                self.mode = VimMode::Insert;
                VimAction::ChangeMode(VimMode::Insert)
            },
            [KeyEvent {
                code: KeyCode::Char('o'),
                ..
            }] => {
                self.mode = VimMode::Insert;
                VimAction::ChangeMode(VimMode::Insert)
            },
            [KeyEvent {
                code: KeyCode::Char('O'),
                ..
            }] => {
                self.mode = VimMode::Insert;
                VimAction::ChangeMode(VimMode::Insert)
            },
            [KeyEvent {
                code: KeyCode::Char('d'),
                ..
            }] => match key.code {
                KeyCode::Char('d') => VimAction::Delete(Motion::LineDown),
                KeyCode::Char('w') => VimAction::Delete(Motion::WordForward),
                KeyCode::Char('b') => VimAction::Delete(Motion::WordBackward),
                KeyCode::Char('$') => VimAction::Delete(Motion::LineEnd),
                KeyCode::Char('0') => VimAction::Delete(Motion::LineStart),
                _ => VimAction::None,
            },
            [KeyEvent {
                code: KeyCode::Char('y'),
                ..
            }] => match key.code {
                KeyCode::Char('y') => VimAction::Yank(Motion::LineDown),
                KeyCode::Char('w') => VimAction::Yank(Motion::WordForward),
                KeyCode::Char('$') => VimAction::Yank(Motion::LineEnd),
                _ => VimAction::None,
            },
            [KeyEvent {
                code: KeyCode::Char('z'),
                ..
            }] => match key.code {
                KeyCode::Char('z') => VimAction::Scroll(ScrollDirection::HalfPageUp),
                KeyCode::Char('t') => VimAction::Scroll(ScrollDirection::HalfPageUp),
                KeyCode::Char('b') => VimAction::Scroll(ScrollDirection::HalfPageDown),
                _ => VimAction::None,
            },
            _ => VimAction::None,
        }
    }

    fn handle_insert(&mut self, key: KeyEvent) -> VimAction {
        match key.code {
            KeyCode::Esc => {
                self.mode = VimMode::Normal;
                VimAction::ChangeMode(VimMode::Normal)
            },
            KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if c == 'c' {
                    self.mode = VimMode::Normal;
                    VimAction::ChangeMode(VimMode::Normal)
                } else {
                    VimAction::None
                }
            },
            KeyCode::Enter => VimAction::SubmitInput,
            KeyCode::Backspace => VimAction::Move(Motion::CharLeft),
            _ => VimAction::None,
        }
    }

    fn handle_visual(&mut self, key: KeyEvent) -> VimAction {
        match key.code {
            KeyCode::Esc => {
                self.mode = VimMode::Normal;
                VimAction::ChangeMode(VimMode::Normal)
            },
            KeyCode::Char('v') => {
                self.mode = VimMode::Normal;
                VimAction::ChangeMode(VimMode::Normal)
            },
            KeyCode::Char('h') => VimAction::Move(Motion::CharLeft),
            KeyCode::Char('j') => VimAction::Move(Motion::LineDown),
            KeyCode::Char('k') => VimAction::Move(Motion::LineUp),
            KeyCode::Char('l') => VimAction::Move(Motion::CharRight),
            KeyCode::Char('w') => VimAction::Move(Motion::WordForward),
            KeyCode::Char('b') => VimAction::Move(Motion::WordBackward),
            KeyCode::Char('y') => VimAction::Yank(Motion::CharRight),
            KeyCode::Char('d' | 'x') => VimAction::Delete(Motion::CharRight),
            _ => VimAction::None,
        }
    }

    fn handle_command(&mut self, key: KeyEvent) -> VimAction {
        match key.code {
            KeyCode::Esc => {
                self.mode = VimMode::Normal;
                self.command_buffer.clear();
                self.search_buffer.clear();
                VimAction::ChangeMode(VimMode::Normal)
            },
            KeyCode::Enter => {
                let cmd = if self.search_buffer.starts_with('/') {
                    let search = self.search_buffer[1..].to_string();
                    self.search_buffer.clear();
                    self.mode = VimMode::Normal;
                    return VimAction::Search(search);
                } else {
                    let cmd = self.command_buffer.clone() + &self.search_buffer;
                    self.command_buffer.clear();
                    self.search_buffer.clear();
                    self.mode = VimMode::Normal;
                    cmd
                };
                self.execute_command(&cmd)
            },
            KeyCode::Backspace => {
                if !self.search_buffer.is_empty() && self.search_buffer != "/" {
                    self.search_buffer.pop();
                } else if !self.command_buffer.is_empty() {
                    self.command_buffer.pop();
                } else {
                    self.mode = VimMode::Normal;
                    return VimAction::ChangeMode(VimMode::Normal);
                }
                VimAction::None
            },
            KeyCode::Char(c) => {
                if self.search_buffer.starts_with('/') || self.search_buffer == "/" {
                    self.search_buffer.push(c);
                } else {
                    self.command_buffer.push(c);
                }
                VimAction::None
            },
            _ => VimAction::None,
        }
    }

    fn execute_command(&mut self, cmd: &str) -> VimAction {
        match cmd.trim() {
            "q" | "quit" => VimAction::Quit,
            "q!" => VimAction::Quit,
            "w" | "write" | "save" => VimAction::Save,
            "wq" | "x" => VimAction::Save,
            "visual" | "v" => VimAction::ChangeMode(VimMode::Visual),
            "insert" | "i" => VimAction::ChangeMode(VimMode::Insert),
            "normal" | "n" => VimAction::ChangeMode(VimMode::Normal),
            cmd if cmd.starts_with('/') => VimAction::Search(cmd[1..].to_string()),
            cmd if cmd.chars().all(|c| c.is_ascii_digit()) => {
                let line = cmd.parse().unwrap_or(1);
                VimAction::Move(Motion::Line(line))
            },
            _ => VimAction::None,
        }
    }

    pub fn command_buffer(&self) -> &str {
        if self.search_buffer.starts_with('/') {
            &self.search_buffer
        } else {
            &self.command_buffer
        }
    }
}
