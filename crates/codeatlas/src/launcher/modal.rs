//! The launcher's modal: a bordered, navigable frame for picking the
//! repository and toggling open code — the interaction Memnoc asked for
//! after walking the plain prompts on macOS ("way too clunky"), then
//! sharpened by their Linux walk: a visible `../` row (a footer key
//! nobody reads is not an affordance; a row is), and a confirm step
//! before launch, because the passive checkbox let its own author sail
//! past open code — the forced decision the old interview had, restored.
//!
//! Drawn by hand over `crossterm`, no TUI framework — the same call
//! ADR-0011 made for the dashboard: one small surface does not buy a
//! layout engine. The modal is a pure state machine ([`Modal::handle`])
//! driven by abstract keys and answering with abstract steps, so every
//! interaction is unit-testable without a terminal; the terminal shell
//! ([`run_modal`]) owns raw mode, the alternate screen, colour, and
//! restoring everything on every exit path including panic. Colour comes
//! from the terminal's own 16-colour palette, so the frame wears the
//! reader's theme rather than shipping one.
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

use crossterm::{cursor, event, execute, style::Stylize, terminal};

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

/// One row of the picker list.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Row {
    /// The directory being listed — choosing it is choosing "here".
    Here,
    /// The visible way up; Enter and `l` both climb.
    Up,
    /// A subdirectory, by name.
    Sub(String),
}

/// The modal's whole state.
pub struct Modal {
    pub dir: PathBuf,
    pub entries: Vec<String>,
    pub cursor: usize,
    pub open_code: bool,
    pub typing: Option<String>,
    pub error: Option<String>,
    /// `Some` while the confirm frame is up: the path awaiting a final
    /// Enter, with open code stated loudly beside it.
    pub confirming: Option<PathBuf>,
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
            confirming: None,
            home,
        }
    }

    /// The list as the reader sees it: here, up (when a parent exists),
    /// then the subdirectories.
    pub fn rows(&self) -> Vec<Row> {
        let mut rows = vec![Row::Here];
        if self.dir.parent().is_some() {
            rows.push(Row::Up);
        }
        rows.extend(self.entries.iter().cloned().map(Row::Sub));
        rows
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
        self.confirming = None;
    }

    pub fn handle(&mut self, key: Key) -> Step {
        // The confirm frame: the one moment that cannot be sailed past.
        if let Some(chosen) = self.confirming.clone() {
            return match key {
                Key::Enter => Step::Choose(chosen),
                Key::ToggleOpenCode => {
                    self.open_code = !self.open_code;
                    Step::Stay
                }
                Key::Quit | Key::Out => {
                    self.confirming = None;
                    Step::Stay
                }
                _ => Step::Stay,
            };
        }
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
                    let resolved = resolve_path(&raw, self.home.as_deref(), &self.dir);
                    self.typing = None;
                    self.confirming = Some(resolved);
                    Step::Stay
                }
                Key::TypePath | Key::Quit => {
                    self.typing = None;
                    self.error = None;
                    Step::Stay
                }
                _ => Step::Stay,
            };
        }
        let rows = self.rows();
        match key {
            Key::Down => {
                if self.cursor + 1 < rows.len() {
                    self.cursor += 1;
                }
                Step::Stay
            }
            Key::Up => {
                self.cursor = self.cursor.saturating_sub(1);
                Step::Stay
            }
            Key::In => match &rows[self.cursor] {
                Row::Here => Step::Stay,
                Row::Up => self.climb(),
                Row::Sub(name) => Step::List(self.dir.join(name)),
            },
            Key::Out => self.climb(),
            Key::Enter => match &rows[self.cursor] {
                // Enter opens folders — the convention every file manager
                // taught the reader's fingers (Memnoc pressed Enter on
                // `Code/` expecting its projects, and got a selection).
                // The pinned `.` row is the one place Enter means "map
                // here": every press walks deeper until you have arrived.
                Row::Up => self.climb(),
                Row::Here => {
                    self.confirming = Some(self.dir.clone());
                    Step::Stay
                }
                Row::Sub(name) => Step::List(self.dir.join(name)),
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

    fn climb(&mut self) -> Step {
        match self.dir.parent() {
            Some(parent) => Step::List(parent.to_path_buf()),
            None => Step::Stay,
        }
    }
}

/// What a rendered line is, so the shell can colour by meaning and the
/// tests can read the text without fighting escape codes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Border,
    Title,
    Where,
    Cursor,
    Row,
    Option,
    Input,
    Error,
    Footer,
    Blank,
    Confirm,
}

pub struct Line {
    pub role: Role,
    pub text: String,
}

fn clip(text: &str, width: usize) -> String {
    let count = text.chars().count();
    if count <= width {
        return text.to_string();
    }
    let kept: String = text.chars().take(width.saturating_sub(1)).collect();
    format!("{kept}…")
}

fn line(role: Role, content: &str) -> Line {
    Line {
        role,
        text: content.to_string(),
    }
}

/// Renders the whole modal as roled lines — pure, so tests read text and
/// roles as data; clipping is the paint layer's business, so nothing here
/// is lost until a terminal genuinely cannot hold it. `visible` is how
/// many list rows the frame shows at once; the window slides to keep the
/// cursor visible.
pub fn draw(modal: &Modal, visible: usize) -> Vec<Line> {
    let mut lines = Vec::new();
    lines.push(line(Role::Title, "CODEATLAS — map a repository"));
    lines.push(line(Role::Blank, ""));

    if let Some(chosen) = &modal.confirming {
        let open = if modal.open_code {
            "OPEN CODE ON — dashboard may show file source"
        } else {
            "OPEN CODE OFF — dashboard shows the map only"
        };
        lines.push(line(Role::Where, &format!("ready: {}", chosen.display())));
        lines.push(line(Role::Blank, ""));
        lines.push(line(Role::Confirm, open));
        if let Some(error) = &modal.error {
            lines.push(line(Role::Error, &format!("! {error}")));
        }
        lines.push(line(Role::Blank, ""));
        lines.push(line(Role::Footer, "Enter go · o flip open code · Esc back"));
        return lines;
    }

    lines.push(line(Role::Where, &format!("in {}", modal.dir.display())));
    lines.push(line(Role::Blank, ""));
    let rows = modal.rows();
    let first = modal
        .cursor
        .saturating_sub(visible - 1)
        .min(rows.len().saturating_sub(visible));
    for (index, row) in rows.iter().enumerate().skip(first).take(visible) {
        let marker = if index == modal.cursor { "▸" } else { " " };
        let label = match row {
            Row::Here => ". (map this directory)".to_string(),
            Row::Up => "../ (up)".to_string(),
            Row::Sub(name) => format!("{name}/"),
        };
        let role = if index == modal.cursor {
            Role::Cursor
        } else {
            Role::Row
        };
        lines.push(line(role, &format!(" {marker} {label}")));
    }
    if rows.len() > first + visible {
        lines.push(line(
            Role::Row,
            &format!("   … {} more below", rows.len() - (first + visible)),
        ));
    }

    lines.push(line(Role::Blank, ""));
    let checkbox = if modal.open_code { "[x]" } else { "[ ]" };
    lines.push(line(
        Role::Option,
        &format!("{checkbox} open code in dashboard"),
    ));

    if let Some(buffer) = &modal.typing {
        lines.push(line(Role::Input, &format!("path: {buffer}▏")));
    }
    if let Some(error) = &modal.error {
        lines.push(line(Role::Error, &format!("! {error} — try again")));
    }

    lines.push(line(Role::Blank, ""));
    if modal.typing.is_some() {
        lines.push(line(Role::Footer, "Enter continue · Esc back to list"));
    } else {
        lines.push(line(Role::Footer, "j/k move · Enter open · h up · / type"));
        lines.push(line(
            Role::Footer,
            "Enter on . maps here · o open code · q quit",
        ));
    }
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

/// One bordered, coloured frame line. Colours are the terminal's own —
/// `cyan` and `dark grey` here are whatever the reader's theme says they
/// are, which is how the frame matches the screenshot taste it came from
/// without shipping a palette.
fn paint(l: &Line, inner: usize) -> String {
    let clipped = clip(&l.text, inner);
    let pad = inner - clipped.chars().count();
    let padded = format!("{clipped}{:pad$}", "");
    let body = match l.role {
        Role::Title => padded.bold().cyan().to_string(),
        Role::Cursor => padded.black().on_cyan().to_string(),
        Role::Confirm => padded.bold().cyan().to_string(),
        Role::Where => padded.dark_grey().to_string(),
        Role::Error => padded.red().to_string(),
        Role::Footer => padded.dark_grey().to_string(),
        Role::Option | Role::Input | Role::Row | Role::Blank | Role::Border => padded,
    };
    format!("{} {} {}", "│".dark_grey(), body, "│".dark_grey())
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
        // Sized and centred against the terminal as it is *now* — a
        // resize event falls through the key filter below and lands
        // back here, so the frame follows the window.
        let (cols, rows) = terminal::size().unwrap_or((80, 24));
        let inner = (cols as usize).saturating_sub(8).clamp(24, 72);
        let visible = (rows as usize).saturating_sub(14).clamp(4, 16);
        let lines = draw(&modal, visible);
        let frame_width = inner + 4;
        let frame_height = lines.len() + 2;
        let x = (cols as usize).saturating_sub(frame_width) / 2;
        let y = (rows as usize).saturating_sub(frame_height) / 2;

        let mut err = io::stderr();
        execute!(err, terminal::Clear(terminal::ClearType::All))?;
        let top = format!("┌{}┐", "─".repeat(inner + 2));
        let bottom = format!("└{}┘", "─".repeat(inner + 2));
        execute!(err, cursor::MoveTo(x as u16, y as u16))?;
        write!(err, "{}", top.as_str().dark_grey())?;
        for (i, l) in lines.iter().enumerate() {
            execute!(err, cursor::MoveTo(x as u16, (y + 1 + i) as u16))?;
            write!(err, "{}", paint(l, inner))?;
        }
        execute!(err, cursor::MoveTo(x as u16, (y + 1 + lines.len()) as u16))?;
        write!(err, "{}", bottom.as_str().dark_grey())?;
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

    fn frame(modal: &Modal) -> String {
        draw(modal, 9)
            .iter()
            .map(|l| l.text.clone())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Enter on a directory, then Enter on the confirm frame.
    fn choose(modal: &mut Modal) -> Step {
        assert_eq!(modal.handle(Key::Enter), Step::Stay, "no confirm frame");
        modal.handle(Key::Enter)
    }

    #[test]
    fn the_pinned_first_row_chooses_here_through_the_confirm() {
        let mut modal = modal_over(&["alpha", "beta"]);
        assert_eq!(choose(&mut modal), Step::Choose(PathBuf::from("/repos")));
    }

    #[test]
    fn the_up_row_is_visible_and_enter_on_it_climbs_instead_of_choosing() {
        let mut modal = modal_over(&["alpha"]);
        assert!(frame(&modal).contains("../ (up)"), "no visible way up");
        modal.handle(Key::Down); // onto ../
        assert_eq!(modal.handle(Key::Enter), Step::List(PathBuf::from("/")));
    }

    #[test]
    fn navigation_clamps_at_both_ends() {
        let mut modal = modal_over(&["alpha", "beta"]);
        modal.handle(Key::Up); // already at the top
        assert_eq!(modal.cursor, 0);
        for _ in 0..5 {
            modal.handle(Key::Down); // past the end: ., .., alpha, beta
        }
        assert_eq!(modal.cursor, 3, "cursor ran past the last row");
    }

    #[test]
    fn enter_on_a_subdirectory_opens_it_instead_of_choosing_it() {
        // The file-manager convention, learned from Memnoc pressing Enter
        // on `Code/` and getting a selection instead of their projects:
        // Enter walks in; only the pinned `.` row maps here.
        let mut modal = modal_over(&["alpha", "beta"]);
        for _ in 0..3 {
            modal.handle(Key::Down); // onto beta
        }
        assert_eq!(
            modal.handle(Key::Enter),
            Step::List(PathBuf::from("/repos/beta")),
            "Enter on a folder must open it, never select it"
        );
        assert!(modal.confirming.is_none());
    }

    #[test]
    fn descending_and_climbing_ask_the_driver_for_listings() {
        let mut modal = modal_over(&["alpha"]);
        modal.handle(Key::Down);
        modal.handle(Key::Down); // ., .., alpha
        assert_eq!(
            modal.handle(Key::In),
            Step::List(PathBuf::from("/repos/alpha"))
        );
        modal.listed(PathBuf::from("/repos/alpha"), Vec::new());
        assert_eq!(modal.handle(Key::Out), Step::List(PathBuf::from("/repos")));
    }

    #[test]
    fn the_confirm_frame_states_open_code_loudly_and_o_flips_it_there() {
        let mut modal = modal_over(&[]);
        modal.handle(Key::Enter);
        assert!(
            frame(&modal).contains("OPEN CODE OFF"),
            "the confirm frame must state the choice: {}",
            frame(&modal)
        );
        modal.handle(Key::ToggleOpenCode);
        assert!(frame(&modal).contains("OPEN CODE ON"));
        assert_eq!(
            modal.handle(Key::Enter),
            Step::Choose(PathBuf::from("/repos"))
        );
        assert!(modal.open_code, "the flip did not survive the confirm");
    }

    #[test]
    fn esc_on_the_confirm_frame_returns_to_the_list_unchosen() {
        let mut modal = modal_over(&["alpha"]);
        modal.handle(Key::Enter);
        assert!(modal.confirming.is_some());
        modal.handle(Key::Quit);
        assert!(modal.confirming.is_none(), "Esc did not back out");
        assert_eq!(modal.handle(Key::Quit), Step::Quit, "then quit quits");
    }

    #[test]
    fn typing_mode_collects_a_path_and_lands_on_the_confirm_frame() {
        let mut modal = modal_over(&["alpha"]);
        modal.handle(Key::TypePath);
        for c in "x/y".chars() {
            modal.handle(Key::Char(c));
        }
        modal.handle(Key::Backspace);
        modal.handle(Key::Char('z'));
        assert_eq!(modal.handle(Key::Enter), Step::Stay);
        assert_eq!(
            modal.confirming,
            Some(PathBuf::from("/repos/x/z")),
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
    fn a_rejected_path_falls_back_to_the_list_with_an_honest_error() {
        let mut modal = modal_over(&[]);
        modal.handle(Key::Enter);
        modal.reject(Path::new("/nope"));
        assert!(
            modal.confirming.is_none(),
            "a reject must leave the confirm"
        );
        assert!(
            frame(&modal).contains("no directory at /nope"),
            "the rejection is invisible: {}",
            frame(&modal)
        );
    }

    #[test]
    fn the_frame_carries_the_cursor_marker_and_the_keybind_footer() {
        let modal = modal_over(&["alpha", "beta"]);
        let text = frame(&modal);
        assert!(text.contains("▸ . (map this directory)"));
        assert!(text.contains("alpha/"));
        assert!(text.contains("j/k move"), "footer missing: {text}");
        assert!(
            text.contains("Enter on . maps here"),
            "the selection rule must be spoken, not implied: {text}"
        );
        let cursor_role = draw(&modal, 9)
            .iter()
            .find(|l| l.text.contains(". (map this directory)"))
            .map(|l| l.role);
        assert_eq!(
            cursor_role,
            Some(Role::Cursor),
            "the highlighted row must carry the cursor role for the shell to colour"
        );
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
