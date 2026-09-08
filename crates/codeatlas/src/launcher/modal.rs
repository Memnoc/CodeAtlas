//! The launcher's modal: a bordered, navigable frame for picking the
//! repository and toggling open code — the interaction Memnoc asked for
//! after walking the plain prompts on macOS ("way too clunky").
//!
//! Drawn by hand over `crossterm`, no TUI framework — the same call
//! ADR-0011 made for the dashboard: one small surface does not buy a
//! layout engine. The modal is a pure state machine ([`Modal::handle`])
//! driven by abstract keys and answering with abstract steps, so every
//! interaction is unit-testable without a terminal; the terminal shell
//! ([`run_modal`]) owns raw mode, the alternate screen, and restoring
//! both on every exit path including panic.
//!
//! Privacy line, stated because a file picker invites the question: the
//! picker reads directory *names* beneath wherever the reader navigates,
//! holds them in memory for the frame being drawn, and writes and sends
//! nothing — no history, no recent list, no file outside the repository
//! eventually scanned. A persisted "recent repositories" list would be a
//! new retained artifact and is deliberately not built (recorded in the
//! V4 agenda as a decision, not an omission).

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crossterm::{cursor, event, execute, terminal};

use super::{Choices, resolve_path};

/// Abstract keys the state machine understands. The shell maps real
/// terminal events onto these; tests feed them directly.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    Up,
    Down,
    In,
    Out,
    Enter,
    ToggleOpenCode,
    TypePath,
    Char(char),
    Backspace,
    Quit,
}

/// What the state machine asks of its driver after a key.
#[derive(Debug, PartialEq, Eq)]
pub enum Step {
    /// Nothing but a redraw.
    Stay,
    /// Read the subdirectories of this path and call [`Modal::listed`].
    List(PathBuf),
    /// Validate this path; [`Modal::reject`] it or finish with it.
    Choose(PathBuf),
    /// The reader backed out.
    Quit,
}

/// The modal's whole state. `entries` are the subdirectory names of
/// `dir`; row 0 is always the pinned "this directory" row, so
/// `cursor == 0` chooses `dir` itself and `cursor - 1` indexes `entries`.
pub struct Modal {
    pub dir: PathBuf,
    pub entries: Vec<String>,
    pub cursor: usize,
    pub open_code: bool,
    pub typing: Option<String>,
    pub error: Option<String>,
    home: Option<PathBuf>,
}

impl Modal {
    pub fn new(dir: PathBuf, entries: Vec<String>, home: Option<PathBuf>) -> Self {
        Self {
            dir,
            entries,
            cursor: 0,
            open_code: false,
            typing: None,
            error: None,
            home,
        }
    }

    fn rows(&self) -> usize {
        self.entries.len() + 1
    }

    /// The driver read a directory listing after a [`Step::List`].
    pub fn listed(&mut self, dir: PathBuf, entries: Vec<String>) {
        self.dir = dir;
        self.entries = entries;
        self.cursor = 0;
        self.error = None;
    }

    /// The driver found a chosen path not to be a directory.
    pub fn reject(&mut self, path: &Path) {
        self.error = Some(format!("no directory at {}", path.display()));
    }

    pub fn handle(&mut self, key: Key) -> Step {
        if let Some(buffer) = &mut self.typing {
            return match key {
                Key::Char(c) => {
                    buffer.push(c);
                    Step::Stay
                }
                Key::Backspace => {
                    buffer.pop();
                    Step::Stay
                }
                Key::Enter => {
                    let raw = buffer.trim().to_string();
                    Step::Choose(resolve_path(&raw, self.home.as_deref(), &self.dir))
                }
                Key::TypePath | Key::Quit => {
                    self.typing = None;
                    self.error = None;
                    Step::Stay
                }
                _ => Step::Stay,
            };
        }
        match key {
            Key::Down => {
                if self.cursor + 1 < self.rows() {
                    self.cursor += 1;
                }
                Step::Stay
            }
            Key::Up => {
                self.cursor = self.cursor.saturating_sub(1);
                Step::Stay
            }
            Key::In => match self.cursor {
                0 => Step::Stay,
                n => Step::List(self.dir.join(&self.entries[n - 1])),
            },
            Key::Out => match self.dir.parent() {
                Some(parent) => Step::List(parent.to_path_buf()),
                None => Step::Stay,
            },
            Key::Enter => match self.cursor {
                0 => Step::Choose(self.dir.clone()),
                n => Step::Choose(self.dir.join(&self.entries[n - 1])),
            },
            Key::ToggleOpenCode => {
                self.open_code = !self.open_code;
                Step::Stay
            }
            Key::TypePath => {
                self.typing = Some(String::new());
                self.error = None;
                Step::Stay
            }
            Key::Quit => Step::Quit,
            Key::Char(_) | Key::Backspace => Step::Stay,
        }
    }
}

/// How many list rows the frame shows at once; the window slides to keep
/// the cursor visible.
const VISIBLE_ROWS: usize = 9;
const INNER_WIDTH: usize = 44;

fn clip(text: &str, width: usize) -> String {
    let count = text.chars().count();
    if count <= width {
        return text.to_string();
    }
    let kept: String = text.chars().take(width.saturating_sub(1)).collect();
    format!("{kept}…")
}

fn frame_line(content: &str) -> String {
    let clipped = clip(content, INNER_WIDTH);
    let pad = INNER_WIDTH - clipped.chars().count();
    format!("│ {clipped}{:pad$} │", "")
}

/// Renders the whole modal as plain lines — pure, so tests read it as
/// strings. The shell positions and prints them.
pub fn draw(modal: &Modal) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!("┌{}┐", "─".repeat(INNER_WIDTH + 2)));
    lines.push(frame_line("CODEATLAS — map a repository"));
    lines.push(frame_line(""));
    lines.push(frame_line(&format!("in {}", modal.dir.display())));

    let first = modal
        .cursor
        .saturating_sub(VISIBLE_ROWS - 1)
        .min(modal.rows().saturating_sub(VISIBLE_ROWS));
    for row in first..modal.rows().min(first + VISIBLE_ROWS) {
        let marker = if row == modal.cursor { "▸" } else { " " };
        let label = match row {
            0 => ". (this directory)".to_string(),
            n => format!("{}/", modal.entries[n - 1]),
        };
        lines.push(frame_line(&format!(" {marker} {label}")));
    }
    if modal.rows() > first + VISIBLE_ROWS {
        lines.push(frame_line(&format!(
            "   … {} more below",
            modal.rows() - (first + VISIBLE_ROWS)
        )));
    }

    lines.push(frame_line(""));
    let checkbox = if modal.open_code { "[x]" } else { "[ ]" };
    lines.push(frame_line(&format!("{checkbox} open code in dashboard")));

    if let Some(buffer) = &modal.typing {
        lines.push(frame_line(&format!("path: {buffer}▏")));
    }
    if let Some(error) = &modal.error {
        lines.push(frame_line(&format!("! {error} — try again")));
    }

    lines.push(frame_line(""));
    let footer = if modal.typing.is_some() {
        "Enter choose · Esc back to list"
    } else {
        "j/k move · l/h in/out · Enter choose · / type · o open code · q quit"
    };
    lines.push(frame_line(footer));
    lines.push(format!("└{}┘", "─".repeat(INNER_WIDTH + 2)));
    lines
}

/// Subdirectory names of `path`: directories only, dotfiles skipped,
/// sorted. Read fresh on every navigation, held in memory only.
pub fn list_dirs(path: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(path) else {
        return Vec::new();
    };
    let mut dirs: Vec<String> = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            (!name.starts_with('.')).then_some(name)
        })
        .collect();
    dirs.sort();
    dirs
}

/// Maps a real terminal event onto an abstract [`Key`], honouring the
/// mode: while typing, letters are letters.
fn map_key(event: &event::KeyEvent, typing: bool) -> Option<Key> {
    use event::{KeyCode, KeyModifiers};
    if event.kind != event::KeyEventKind::Press {
        return None;
    }
    if event.modifiers.contains(KeyModifiers::CONTROL) && event.code == KeyCode::Char('c') {
        return Some(Key::Quit);
    }
    // A newline character is Enter in every mode: input that crossed a
    // pty, or arrived before raw mode was up, comes cooked (`\r` →
    // `\n`), and a chooser that swallows it strands exactly the
    // scripted and pseudo-terminal readers (found by the pty walk).
    if matches!(event.code, KeyCode::Char('\n' | '\r')) {
        return Some(Key::Enter);
    }
    if typing {
        return match event.code {
            KeyCode::Enter => Some(Key::Enter),
            KeyCode::Esc => Some(Key::TypePath),
            KeyCode::Backspace => Some(Key::Backspace),
            KeyCode::Char(c) => Some(Key::Char(c)),
            _ => None,
        };
    }
    match event.code {
        KeyCode::Down | KeyCode::Char('j') => Some(Key::Down),
        KeyCode::Up | KeyCode::Char('k') => Some(Key::Up),
        KeyCode::Right | KeyCode::Char('l') => Some(Key::In),
        KeyCode::Left | KeyCode::Char('h') => Some(Key::Out),
        KeyCode::Enter => Some(Key::Enter),
        KeyCode::Char('o') => Some(Key::ToggleOpenCode),
        KeyCode::Char('/') => Some(Key::TypePath),
        KeyCode::Esc | KeyCode::Char('q') => Some(Key::Quit),
        _ => None,
    }
}

/// Restores the terminal on every exit path, panic included — a raw-mode
/// terminal left behind is worse than any error message.
struct Restore;

impl Drop for Restore {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(io::stderr(), terminal::LeaveAlternateScreen, cursor::Show);
    }
}

/// Runs the modal on the real terminal. `Err` means the terminal could
/// not host it (raw mode refused) — the caller falls back to the plain
/// interview rather than failing the launcher.
pub fn run_modal(start: &Path, home: Option<PathBuf>) -> io::Result<Option<Choices>> {
    terminal::enable_raw_mode()?;
    let _restore = Restore;
    execute!(io::stderr(), terminal::EnterAlternateScreen, cursor::Hide)?;

    let start = start.canonicalize().unwrap_or_else(|_| start.to_path_buf());
    let mut modal = Modal::new(start.clone(), list_dirs(&start), home);

    loop {
        let mut err = io::stderr();
        execute!(
            err,
            terminal::Clear(terminal::ClearType::All),
            cursor::MoveTo(0, 0)
        )?;
        for line in draw(&modal) {
            execute!(err, cursor::MoveToColumn(0))?;
            writeln!(err, "{line}")?;
        }
        err.flush()?;

        let event::Event::Key(key_event) = event::read()? else {
            continue;
        };
        let Some(key) = map_key(&key_event, modal.typing.is_some()) else {
            continue;
        };
        match modal.handle(key) {
            Step::Stay => {}
            Step::List(path) => {
                let entries = list_dirs(&path);
                modal.listed(path, entries);
            }
            Step::Choose(path) => match path.canonicalize() {
                Ok(dir) if dir.is_dir() => {
                    return Ok(Some(Choices {
                        root: dir,
                        open_code: modal.open_code,
                    }));
                }
                _ => modal.reject(&path),
            },
            Step::Quit => return Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modal_over(entries: &[&str]) -> Modal {
        Modal::new(
            PathBuf::from("/repos"),
            entries.iter().map(|s| s.to_string()).collect(),
            None,
        )
    }

    #[test]
    fn the_pinned_first_row_chooses_the_directory_being_listed() {
        let mut modal = modal_over(&["alpha", "beta"]);
        assert_eq!(
            modal.handle(Key::Enter),
            Step::Choose(PathBuf::from("/repos"))
        );
    }

    #[test]
    fn navigation_clamps_at_both_ends_and_enter_chooses_the_highlighted_dir() {
        let mut modal = modal_over(&["alpha", "beta"]);
        modal.handle(Key::Up); // already at the top
        assert_eq!(modal.cursor, 0);
        modal.handle(Key::Down);
        modal.handle(Key::Down);
        modal.handle(Key::Down); // past the end
        assert_eq!(modal.cursor, 2, "cursor ran past the last row");
        assert_eq!(
            modal.handle(Key::Enter),
            Step::Choose(PathBuf::from("/repos/beta"))
        );
    }

    #[test]
    fn descending_and_climbing_ask_the_driver_for_listings() {
        let mut modal = modal_over(&["alpha"]);
        modal.handle(Key::Down);
        assert_eq!(
            modal.handle(Key::In),
            Step::List(PathBuf::from("/repos/alpha"))
        );
        modal.listed(PathBuf::from("/repos/alpha"), Vec::new());
        assert_eq!(modal.handle(Key::Out), Step::List(PathBuf::from("/repos")));
    }

    #[test]
    fn descending_on_the_pinned_row_goes_nowhere() {
        let mut modal = modal_over(&["alpha"]);
        assert_eq!(modal.handle(Key::In), Step::Stay);
    }

    #[test]
    fn the_open_code_toggle_flips_and_the_frame_shows_it() {
        let mut modal = modal_over(&[]);
        assert!(!modal.open_code);
        modal.handle(Key::ToggleOpenCode);
        assert!(modal.open_code);
        let frame = draw(&modal).join("\n");
        assert!(
            frame.contains("[x] open code"),
            "checkbox unticked: {frame}"
        );
    }

    #[test]
    fn typing_mode_collects_a_path_and_enter_chooses_it_resolved() {
        let mut modal = modal_over(&["alpha"]);
        modal.handle(Key::TypePath);
        for c in "x/y".chars() {
            modal.handle(Key::Char(c));
        }
        modal.handle(Key::Backspace);
        modal.handle(Key::Char('z'));
        assert_eq!(
            modal.handle(Key::Enter),
            Step::Choose(PathBuf::from("/repos/x/z")),
            "typed paths resolve against the listed directory"
        );
    }

    #[test]
    fn while_typing_letters_are_letters_not_bindings() {
        let mut modal = modal_over(&[]);
        modal.handle(Key::TypePath);
        modal.handle(Key::Char('q'));
        modal.handle(Key::Char('j'));
        assert!(
            modal.typing.as_deref() == Some("qj"),
            "bindings ate typed text"
        );
    }

    #[test]
    fn a_rejected_path_draws_an_honest_error_in_the_frame() {
        let mut modal = modal_over(&[]);
        modal.reject(Path::new("/nope"));
        let frame = draw(&modal).join("\n");
        assert!(
            frame.contains("no directory at /nope"),
            "the rejection is invisible: {frame}"
        );
    }

    #[test]
    fn the_frame_carries_the_cursor_marker_and_the_keybind_footer() {
        let modal = modal_over(&["alpha", "beta"]);
        let frame = draw(&modal).join("\n");
        assert!(frame.contains("▸ . (this directory)"));
        assert!(frame.contains("alpha/"));
        assert!(frame.contains("j/k move"), "footer missing: {frame}");
    }

    #[test]
    fn a_cooked_newline_is_enter_in_both_modes() {
        use crossterm::event::{KeyCode, KeyEvent};
        let newline = KeyEvent::from(KeyCode::Char('\n'));
        let carriage = KeyEvent::from(KeyCode::Char('\r'));
        for typing in [false, true] {
            assert_eq!(super::map_key(&newline, typing), Some(Key::Enter));
            assert_eq!(super::map_key(&carriage, typing), Some(Key::Enter));
        }
    }

    #[test]
    fn list_dirs_reads_only_visible_directories_sorted() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("zeta")).unwrap();
        std::fs::create_dir(root.path().join("alpha")).unwrap();
        std::fs::create_dir(root.path().join(".hidden")).unwrap();
        std::fs::write(root.path().join("file.txt"), "x").unwrap();
        assert_eq!(list_dirs(root.path()), vec!["alpha", "zeta"]);
    }
}
