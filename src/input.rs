//! Vim-style keyboard navigation.
//!
//! Modal input handling: Normal, Insert, Command, Search, Follow modes.
//! Vim-like keybindings for page navigation, link following, and URL entry.

/// Input mode for the browser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Page navigation: scrolling, link following, tab management.
    Normal,
    /// Text input: address bar, form fields.
    Insert,
    /// `:` prefixed commands (`:open`, `:tabopen`, etc.).
    Command,
    /// `/` forward search or `?` backward search.
    Search,
    /// `f` link hint labels displayed.
    Follow,
}

/// Actions that the browser can perform.
#[derive(Debug, Clone, PartialEq)]
pub enum BrowserAction {
    // -- Scrolling --
    ScrollDown(u32),
    ScrollUp(u32),
    ScrollLeft(u32),
    ScrollRight(u32),
    HalfPageDown,
    HalfPageUp,
    PageDown,
    PageUp,
    ScrollToTop,
    ScrollToBottom,

    // -- Navigation --
    GoBack,
    GoForward,
    Reload,
    Stop,

    // -- Link following --
    FollowLink,
    FollowLinkNewTab,

    // -- URL entry --
    OpenUrl,
    OpenUrlNewTab,

    // -- Tabs --
    NewTab,
    CloseTab,
    NextTab,
    PrevTab,
    GotoTab(usize),

    // -- Clipboard --
    CopyUrl,
    OpenFromClipboard,

    // -- Search --
    SearchForward,
    SearchBackward,
    NextMatch,
    PrevMatch,

    // -- Command mode --
    EnterCommandMode,
    ExecuteCommand(String),

    // -- Mode switching --
    EnterInsertMode,
    ExitToNormal,

    // -- Text editing (Insert mode) --
    InsertChar(char),
    DeleteBack,
    DeleteForward,
    MoveCursorLeft,
    MoveCursorRight,
    MoveCursorHome,
    MoveCursorEnd,
    SubmitInput,
    CancelInput,

    // -- Bookmarks --
    ToggleBookmark,
    ShowBookmarks,

    // -- History --
    ShowHistory,

    // -- Misc --
    Quit,
    Help,
    Noop,
}

/// Tracks pending key sequences (e.g., "gg", "gt").
#[derive(Debug)]
pub struct InputHandler {
    mode: Mode,
    /// Pending key buffer for multi-key sequences.
    pending: String,
    /// Command line buffer (for : and / modes).
    command_buffer: String,
    /// Cursor position in the command buffer.
    cursor_pos: usize,
    /// Whether search is forward (/) or backward (?).
    search_forward: bool,
}

impl InputHandler {
    /// Create a new input handler in Normal mode.
    #[must_use]
    pub fn new() -> Self {
        Self {
            mode: Mode::Normal,
            pending: String::new(),
            command_buffer: String::new(),
            cursor_pos: 0,
            search_forward: true,
        }
    }

    /// Get the current mode.
    #[must_use]
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Get the current command buffer content.
    #[must_use]
    pub fn command_buffer(&self) -> &str {
        &self.command_buffer
    }

    /// Get cursor position in command buffer.
    #[must_use]
    pub fn cursor_pos(&self) -> usize {
        self.cursor_pos
    }

    /// Get the mode indicator string for the status bar.
    #[must_use]
    pub fn mode_indicator(&self) -> &str {
        match self.mode {
            Mode::Normal => "NORMAL",
            Mode::Insert => "INSERT",
            Mode::Command => "COMMAND",
            Mode::Search => if self.search_forward { "SEARCH /" } else { "SEARCH ?" },
            Mode::Follow => "FOLLOW",
        }
    }

    /// Process a key input and return the resulting action.
    pub fn handle_key(&mut self, key: char, ctrl: bool, shift: bool) -> BrowserAction {
        match self.mode {
            Mode::Normal => self.handle_normal(key, ctrl, shift),
            Mode::Insert => self.handle_insert(key, ctrl),
            Mode::Command => self.handle_command(key, ctrl),
            Mode::Search => self.handle_search(key, ctrl),
            Mode::Follow => self.handle_follow(key),
        }
    }

    /// Process a special key (Enter, Escape, Backspace, etc.).
    pub fn handle_special_key(&mut self, key: SpecialKey) -> BrowserAction {
        match self.mode {
            Mode::Normal => self.handle_normal_special(key),
            Mode::Insert | Mode::Command | Mode::Search => {
                self.handle_input_special(key)
            }
            Mode::Follow => {
                if key == SpecialKey::Escape {
                    self.mode = Mode::Normal;
                    self.pending.clear();
                }
                BrowserAction::ExitToNormal
            }
        }
    }

    fn handle_normal(&mut self, key: char, ctrl: bool, shift: bool) -> BrowserAction {
        // Ctrl shortcuts.
        if ctrl {
            return match key {
                't' => BrowserAction::NewTab,
                'w' => BrowserAction::CloseTab,
                'd' => BrowserAction::HalfPageDown,
                'u' => BrowserAction::HalfPageUp,
                'f' => BrowserAction::PageDown,
                'b' => BrowserAction::PageUp,
                'c' => BrowserAction::Stop,
                'q' => BrowserAction::Quit,
                _ => BrowserAction::Noop,
            };
        }

        // Check for pending multi-key sequences.
        if !self.pending.is_empty() {
            self.pending.push(key);
            let seq = self.pending.clone();
            self.pending.clear();
            return match seq.as_str() {
                "gg" => BrowserAction::ScrollToTop,
                "gt" => BrowserAction::NextTab,
                "gT" => BrowserAction::PrevTab,
                "yy" => BrowserAction::CopyUrl,
                _ => BrowserAction::Noop,
            };
        }

        // Single-key commands.
        match key {
            'j' => BrowserAction::ScrollDown(3),
            'k' => BrowserAction::ScrollUp(3),
            'h' => BrowserAction::ScrollLeft(3),
            'l' => BrowserAction::ScrollRight(3),
            'd' => BrowserAction::HalfPageDown,
            'u' => BrowserAction::HalfPageUp,
            'G' => BrowserAction::ScrollToBottom,
            'g' => {
                self.pending.push('g');
                BrowserAction::Noop
            }
            'y' => {
                self.pending.push('y');
                BrowserAction::Noop
            }
            'f' => {
                self.mode = Mode::Follow;
                BrowserAction::FollowLink
            }
            'F' => {
                self.mode = Mode::Follow;
                BrowserAction::FollowLinkNewTab
            }
            'o' => {
                self.mode = Mode::Insert;
                self.command_buffer.clear();
                self.cursor_pos = 0;
                BrowserAction::OpenUrl
            }
            'O' => {
                self.mode = Mode::Insert;
                self.command_buffer.clear();
                self.cursor_pos = 0;
                BrowserAction::OpenUrlNewTab
            }
            'H' => BrowserAction::GoBack,
            'L' => BrowserAction::GoForward,
            'r' => BrowserAction::Reload,
            't' => BrowserAction::NewTab,
            'x' => BrowserAction::CloseTab,
            'n' => BrowserAction::NextMatch,
            'N' => BrowserAction::PrevMatch,
            'p' => BrowserAction::OpenFromClipboard,
            'i' => {
                self.mode = Mode::Insert;
                BrowserAction::EnterInsertMode
            }
            '/' => {
                self.mode = Mode::Search;
                self.search_forward = true;
                self.command_buffer.clear();
                self.cursor_pos = 0;
                BrowserAction::SearchForward
            }
            '?' => {
                self.mode = Mode::Search;
                self.search_forward = false;
                self.command_buffer.clear();
                self.cursor_pos = 0;
                BrowserAction::SearchBackward
            }
            ':' => {
                self.mode = Mode::Command;
                self.command_buffer.clear();
                self.cursor_pos = 0;
                BrowserAction::EnterCommandMode
            }
            'b' if shift => BrowserAction::ShowBookmarks,
            'B' => BrowserAction::ToggleBookmark,
            _ => BrowserAction::Noop,
        }
    }

    fn handle_normal_special(&mut self, key: SpecialKey) -> BrowserAction {
        self.pending.clear();
        match key {
            SpecialKey::Escape => BrowserAction::ExitToNormal,
            SpecialKey::Enter => BrowserAction::Noop,
            _ => BrowserAction::Noop,
        }
    }

    fn handle_insert(&mut self, key: char, ctrl: bool) -> BrowserAction {
        if ctrl {
            return match key {
                'c' | '[' => {
                    self.mode = Mode::Normal;
                    BrowserAction::CancelInput
                }
                'a' => {
                    self.cursor_pos = 0;
                    BrowserAction::MoveCursorHome
                }
                'e' => {
                    self.cursor_pos = self.command_buffer.len();
                    BrowserAction::MoveCursorEnd
                }
                'w' => {
                    // Delete word backward.
                    let old = self.command_buffer.clone();
                    let pos = self.cursor_pos;
                    let before = &old[..pos];
                    let after = &old[pos..];
                    let new_pos = before.rfind(' ').map(|p| p + 1).unwrap_or(0);
                    self.command_buffer = format!("{}{}", &before[..new_pos], after);
                    self.cursor_pos = new_pos;
                    BrowserAction::Noop
                }
                _ => BrowserAction::Noop,
            };
        }

        self.command_buffer.insert(self.cursor_pos, key);
        self.cursor_pos += 1;
        BrowserAction::InsertChar(key)
    }

    fn handle_input_special(&mut self, key: SpecialKey) -> BrowserAction {
        match key {
            SpecialKey::Escape => {
                self.mode = Mode::Normal;
                self.command_buffer.clear();
                self.cursor_pos = 0;
                BrowserAction::CancelInput
            }
            SpecialKey::Enter => {
                let cmd = self.command_buffer.clone();
                self.command_buffer.clear();
                self.cursor_pos = 0;

                if self.mode == Mode::Command {
                    self.mode = Mode::Normal;
                    BrowserAction::ExecuteCommand(cmd)
                } else if self.mode == Mode::Search {
                    self.mode = Mode::Normal;
                    BrowserAction::ExecuteCommand(format!(
                        "{}{}",
                        if self.search_forward { "/" } else { "?" },
                        cmd
                    ))
                } else {
                    self.mode = Mode::Normal;
                    BrowserAction::SubmitInput
                }
            }
            SpecialKey::Backspace => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                    self.command_buffer.remove(self.cursor_pos);
                }
                BrowserAction::DeleteBack
            }
            SpecialKey::Delete => {
                if self.cursor_pos < self.command_buffer.len() {
                    self.command_buffer.remove(self.cursor_pos);
                }
                BrowserAction::DeleteForward
            }
            SpecialKey::Left => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                }
                BrowserAction::MoveCursorLeft
            }
            SpecialKey::Right => {
                if self.cursor_pos < self.command_buffer.len() {
                    self.cursor_pos += 1;
                }
                BrowserAction::MoveCursorRight
            }
            SpecialKey::Home => {
                self.cursor_pos = 0;
                BrowserAction::MoveCursorHome
            }
            SpecialKey::End => {
                self.cursor_pos = self.command_buffer.len();
                BrowserAction::MoveCursorEnd
            }
            _ => BrowserAction::Noop,
        }
    }

    fn handle_command(&mut self, key: char, ctrl: bool) -> BrowserAction {
        self.handle_insert(key, ctrl)
    }

    fn handle_search(&mut self, key: char, ctrl: bool) -> BrowserAction {
        self.handle_insert(key, ctrl)
    }

    fn handle_follow(&mut self, key: char) -> BrowserAction {
        if key.is_ascii_alphanumeric() {
            self.pending.push(key);
            // In follow mode, the pending buffer accumulates hint label chars.
            // The browser will resolve this to a link index.
            BrowserAction::Noop
        } else {
            self.mode = Mode::Normal;
            self.pending.clear();
            BrowserAction::ExitToNormal
        }
    }

    /// Get the follow-mode pending hint label.
    #[must_use]
    pub fn follow_hint(&self) -> &str {
        &self.pending
    }

    /// Set the input buffer contents (e.g., pre-fill with current URL).
    pub fn set_buffer(&mut self, content: &str) {
        self.command_buffer = content.to_string();
        self.cursor_pos = content.len();
    }

    /// Force mode change.
    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
        self.pending.clear();
    }
}

impl Default for InputHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Special (non-character) keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialKey {
    Enter,
    Escape,
    Backspace,
    Delete,
    Tab,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
}

/// Parse a command string into an action.
#[must_use]
pub fn parse_command(cmd: &str) -> BrowserAction {
    let parts: Vec<&str> = cmd.trim().splitn(2, ' ').collect();
    let command = parts[0];
    let arg = parts.get(1).copied().unwrap_or("");

    match command {
        "open" | "o" => {
            if arg.is_empty() {
                BrowserAction::OpenUrl
            } else {
                BrowserAction::ExecuteCommand(format!("open {arg}"))
            }
        }
        "tabopen" | "topen" | "to" => {
            if arg.is_empty() {
                BrowserAction::OpenUrlNewTab
            } else {
                BrowserAction::ExecuteCommand(format!("tabopen {arg}"))
            }
        }
        "bookmark" | "bm" => BrowserAction::ToggleBookmark,
        "bookmarks" | "bmarks" => BrowserAction::ShowBookmarks,
        "history" | "hist" => BrowserAction::ShowHistory,
        "quit" | "q" => BrowserAction::Quit,
        "reload" | "r" => BrowserAction::Reload,
        "back" => BrowserAction::GoBack,
        "forward" => BrowserAction::GoForward,
        "help" | "h" => BrowserAction::Help,
        _ => BrowserAction::Noop,
    }
}

/// Generate hint labels for a given number of links (a, b, ..., z, aa, ab, ...).
#[must_use]
pub fn generate_hint_labels(count: usize) -> Vec<String> {
    let chars: Vec<char> = "asdfghjkl".chars().collect();
    let base = chars.len();
    let mut labels = Vec::with_capacity(count);

    for i in 0..count {
        let mut label = String::new();
        let n = i;

        if count <= base {
            label.push(chars[n % base]);
        } else {
            // Two-character labels.
            let first = n / base;
            let second = n % base;
            if first < base {
                label.push(chars[first]);
            }
            label.push(chars[second]);
        }

        labels.push(label);
    }

    labels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_mode_scroll() {
        let mut handler = InputHandler::new();
        assert_eq!(handler.mode(), Mode::Normal);
        assert_eq!(handler.handle_key('j', false, false), BrowserAction::ScrollDown(3));
        assert_eq!(handler.handle_key('k', false, false), BrowserAction::ScrollUp(3));
    }

    #[test]
    fn normal_mode_gg_sequence() {
        let mut handler = InputHandler::new();
        let action1 = handler.handle_key('g', false, false);
        assert_eq!(action1, BrowserAction::Noop); // Waiting for second key.
        let action2 = handler.handle_key('g', false, false);
        assert_eq!(action2, BrowserAction::ScrollToTop);
    }

    #[test]
    fn normal_mode_gt_sequence() {
        let mut handler = InputHandler::new();
        handler.handle_key('g', false, false);
        let action = handler.handle_key('t', false, false);
        assert_eq!(action, BrowserAction::NextTab);
    }

    #[test]
    fn enter_command_mode() {
        let mut handler = InputHandler::new();
        handler.handle_key(':', false, false);
        assert_eq!(handler.mode(), Mode::Command);
    }

    #[test]
    fn enter_search_mode() {
        let mut handler = InputHandler::new();
        handler.handle_key('/', false, false);
        assert_eq!(handler.mode(), Mode::Search);
    }

    #[test]
    fn escape_returns_to_normal() {
        let mut handler = InputHandler::new();
        handler.handle_key(':', false, false);
        assert_eq!(handler.mode(), Mode::Command);
        handler.handle_special_key(SpecialKey::Escape);
        assert_eq!(handler.mode(), Mode::Normal);
    }

    #[test]
    fn insert_mode_typing() {
        let mut handler = InputHandler::new();
        handler.set_mode(Mode::Insert);
        handler.handle_key('h', false, false);
        handler.handle_key('i', false, false);
        assert_eq!(handler.command_buffer(), "hi");
    }

    #[test]
    fn command_execution() {
        let mut handler = InputHandler::new();
        handler.handle_key(':', false, false);
        handler.handle_key('q', false, false);
        let action = handler.handle_special_key(SpecialKey::Enter);
        assert_eq!(action, BrowserAction::ExecuteCommand("q".to_string()));
    }

    #[test]
    fn parse_command_open() {
        let action = parse_command("open https://example.com");
        assert_eq!(
            action,
            BrowserAction::ExecuteCommand("open https://example.com".to_string())
        );
    }

    #[test]
    fn parse_command_quit() {
        let action = parse_command("quit");
        assert_eq!(action, BrowserAction::Quit);
    }

    #[test]
    fn hint_label_generation() {
        let labels = generate_hint_labels(5);
        assert_eq!(labels.len(), 5);
        // All labels should be unique.
        let unique: std::collections::HashSet<_> = labels.iter().collect();
        assert_eq!(unique.len(), 5);
    }

    #[test]
    fn ctrl_keys_in_normal() {
        let mut handler = InputHandler::new();
        assert_eq!(handler.handle_key('t', true, false), BrowserAction::NewTab);
        assert_eq!(handler.handle_key('w', true, false), BrowserAction::CloseTab);
        assert_eq!(handler.handle_key('q', true, false), BrowserAction::Quit);
    }

    #[test]
    fn mode_indicator() {
        let mut handler = InputHandler::new();
        assert_eq!(handler.mode_indicator(), "NORMAL");
        handler.set_mode(Mode::Insert);
        assert_eq!(handler.mode_indicator(), "INSERT");
        handler.set_mode(Mode::Command);
        assert_eq!(handler.mode_indicator(), "COMMAND");
    }
}
