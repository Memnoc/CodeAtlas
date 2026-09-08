//! The interactive launcher: what a bare `codeatlas` at a real terminal
//! runs instead of printing usage.
//!
//! One audience, one moment: a person who just downloaded the binary and
//! runs it with nothing after its name. The launcher asks for a repository
//! path and one yes/no, then scans, serves, and opens the browser itself
//! once the served port actually answers. Everything about it is gated on
//! that audience: in any non-terminal context — scripts, pipes, CI —
//! [`should_run`] declines and bare invocation prints clap's usage exactly
//! as it did before this module existed. The model flags are deliberately
//! not offered here: the launcher is the no-key path, and it behaves
//! identically in a sealed build.

pub mod modal;

use std::io::{self, BufRead, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{Duration, Instant};

/// The port the launcher serves on — the same default the `serve`
/// subcommand documents, so the URL a reader learns here is the URL the
/// rest of the documentation talks about.
const PORT: u16 = 4173;

/// How long the browser-opening thread waits for the served port to answer
/// before giving up silently. Serve binds in milliseconds; the budget is
/// generous because a browser opened at a dead URL is worse than no
/// browser at all.
const OPEN_BUDGET: Duration = Duration::from_secs(5);
const OPEN_POLL: Duration = Duration::from_millis(100);

/// The gate: a bare invocation, and a human on both ends of the terminal —
/// the prompt reads stdin and writes stderr, so both must be real.
pub fn should_run(arg_count: usize, stdin_is_tty: bool, stderr_is_tty: bool) -> bool {
    arg_count == 1 && stdin_is_tty && stderr_is_tty
}

/// What the interview settles.
pub struct Choices {
    pub root: PathBuf,
    pub open_code: bool,
}

/// The two questions, over injectable ends so tests drive them with
/// buffers where a real run holds the terminal. `None` means the reader
/// closed stdin — backing out is not an error.
pub fn interview(
    input: &mut dyn BufRead,
    out: &mut dyn Write,
    home: Option<&Path>,
    cwd: &Path,
) -> io::Result<Option<Choices>> {
    let _ = writeln!(out, "CodeAtlas — map a repository and serve its dashboard");
    let root = loop {
        let _ = write!(out, "repository path [.]: ");
        let _ = out.flush();
        let Some(line) = read_line(input)? else {
            return Ok(None);
        };
        let candidate = resolve_path(&line, home, cwd);
        match candidate.canonicalize() {
            Ok(dir) if dir.is_dir() => break dir,
            _ => {
                let _ = writeln!(
                    out,
                    "no directory at {} — try again (Ctrl-C quits)",
                    candidate.display()
                );
            }
        }
    };
    let _ = write!(out, "show mapped files' source in the dashboard? [y/N]: ");
    let _ = out.flush();
    let open_code = match read_line(input)? {
        Some(line) => parse_yes(&line),
        None => return Ok(None),
    };
    Ok(Some(Choices { root, open_code }))
}

fn read_line(input: &mut dyn BufRead) -> io::Result<Option<String>> {
    let mut line = String::new();
    if input.read_line(&mut line)? == 0 {
        return Ok(None);
    }
    Ok(Some(line.trim().to_string()))
}

/// Empty means "here"; `~` and `~/…` expand against the home directory;
/// anything relative resolves against the directory the launcher was run
/// from, so validation never depends on the process's own idea of cwd.
fn resolve_path(raw: &str, home: Option<&Path>, cwd: &Path) -> PathBuf {
    if raw.is_empty() {
        return cwd.to_path_buf();
    }
    if raw == "~"
        && let Some(home) = home
    {
        return home.to_path_buf();
    }
    if let Some(rest) = raw.strip_prefix("~/")
        && let Some(home) = home
    {
        return home.join(rest);
    }
    cwd.join(raw)
}

fn parse_yes(line: &str) -> bool {
    matches!(line.to_ascii_lowercase().as_str(), "y" | "yes")
}

/// The one program the launcher spawns beyond what the documented flags
/// name: the OS URL-opener — fixed by name per platform, never
/// configurable, handed exactly the served loopback URL, and an
/// environment with no API key in it. `serve` itself never builds this;
/// only the interactive launcher does.
pub fn opener(url: &str) -> Command {
    let mut cmd = Command::new(if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    });
    cmd.arg(url);
    cmd.env_remove("ANTHROPIC_API_KEY");
    cmd
}

fn port_answers() -> bool {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, PORT));
    TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_ok()
}

/// The whole flow: interview → scan → serve, with the browser opened by a
/// helper thread once the served port genuinely answers. If the port
/// already answers *before* serve starts, something else owns it — serve
/// will refuse with its own port-in-use message, and no browser opens at
/// someone else's server.
pub fn run() -> ExitCode {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut out = io::stderr();
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let cwd = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::FAILURE;
        }
    };
    // The modal first; the plain interview only when the terminal refuses
    // raw mode — a reader gets one of the two, never neither.
    let choices = match modal::run_modal(&cwd, home.clone()) {
        Ok(Some(choices)) => choices,
        Ok(None) => {
            eprintln!("no repository chosen — nothing to map");
            return ExitCode::SUCCESS;
        }
        Err(_) => match interview(&mut input, &mut out, home.as_deref(), &cwd) {
            Ok(Some(choices)) => choices,
            Ok(None) => {
                eprintln!("\nno path given — nothing to map");
                return ExitCode::SUCCESS;
            }
            Err(err) => {
                eprintln!("error: {err}");
                return ExitCode::FAILURE;
            }
        },
    };

    if let Err(err) = crate::build_and_save_map(&choices.root) {
        eprintln!("error: {err:#}");
        return ExitCode::FAILURE;
    }

    if !port_answers() {
        let url = format!("http://{}:{PORT}/", Ipv4Addr::LOCALHOST);
        std::thread::spawn(move || {
            let deadline = Instant::now() + OPEN_BUDGET;
            while Instant::now() < deadline {
                if port_answers() {
                    let _ = opener(&url).spawn();
                    return;
                }
                std::thread::sleep(OPEN_POLL);
            }
        });
    }

    let options = crate::serve::ServeOptions {
        port: PORT,
        ask: None,
        open_code: choices.open_code,
    };
    match crate::serve::serve(&choices.root, options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn interview_over(
        script: &str,
        home: Option<&Path>,
        cwd: &Path,
    ) -> (io::Result<Option<Choices>>, String) {
        let mut input = Cursor::new(script.to_string());
        let mut out: Vec<u8> = Vec::new();
        let result = interview(&mut input, &mut out, home, cwd);
        (result, String::from_utf8(out).unwrap())
    }

    #[test]
    fn a_path_and_a_yes_settle_both_choices() {
        let repo = tempfile::tempdir().unwrap();
        let script = format!("{}\ny\n", repo.path().display());
        let (result, _) = interview_over(&script, None, Path::new("/"));
        let choices = result.unwrap().unwrap();
        assert_eq!(choices.root, repo.path().canonicalize().unwrap());
        assert!(choices.open_code);
    }

    #[test]
    fn empty_input_means_here_and_the_default_answer_is_no() {
        let cwd = tempfile::tempdir().unwrap();
        let (result, _) = interview_over("\n\n", None, cwd.path());
        let choices = result.unwrap().unwrap();
        assert_eq!(choices.root, cwd.path().canonicalize().unwrap());
        assert!(!choices.open_code, "an empty answer must not opt in");
    }

    #[test]
    fn a_tilde_expands_against_the_home_directory() {
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir(home.path().join("repo")).unwrap();
        let (result, _) = interview_over("~/repo\nn\n", Some(home.path()), Path::new("/"));
        let choices = result.unwrap().unwrap();
        assert_eq!(
            choices.root,
            home.path().join("repo").canonicalize().unwrap()
        );
    }

    #[test]
    fn a_wrong_path_draws_an_honest_retry_and_the_next_answer_lands() {
        let repo = tempfile::tempdir().unwrap();
        let script = format!("definitely/not/here\n{}\nn\n", repo.path().display());
        let (result, prompts) = interview_over(&script, None, Path::new("/"));
        assert!(
            prompts.contains("no directory at"),
            "the retry never said what was wrong: {prompts:?}"
        );
        let choices = result.unwrap().unwrap();
        assert_eq!(choices.root, repo.path().canonicalize().unwrap());
    }

    #[test]
    fn closing_stdin_backs_out_at_either_question() {
        let (at_path, _) = interview_over("", None, Path::new("/"));
        assert!(at_path.unwrap().is_none());
        let cwd = tempfile::tempdir().unwrap();
        let (at_yes_no, _) = interview_over("\n", None, cwd.path());
        assert!(at_yes_no.unwrap().is_none());
    }

    #[test]
    fn only_a_spoken_yes_opts_in() {
        for yes in ["y", "Y", "yes", "YES"] {
            assert!(parse_yes(yes), "{yes:?} should opt in");
        }
        for no in ["", "n", "N", "no", "nah", "sure"] {
            assert!(!parse_yes(no), "{no:?} must not opt in");
        }
    }

    #[test]
    fn the_gate_wants_a_bare_invocation_and_a_terminal_on_both_ends() {
        assert!(should_run(1, true, true));
        assert!(!should_run(2, true, true), "arguments mean a typed command");
        assert!(!should_run(1, false, true), "a piped stdin is a script");
        assert!(!should_run(1, true, false), "a piped stderr is a script");
    }

    #[test]
    fn the_opener_is_the_platform_opener_with_the_loopback_url_and_no_api_key() {
        let cmd = opener("http://127.0.0.1:4173/");
        let expected = if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        assert_eq!(cmd.get_program().to_string_lossy(), expected);
        let args: Vec<_> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            vec!["http://127.0.0.1:4173/".to_string()],
            "the opener takes exactly the served URL and nothing else"
        );
        assert!(
            cmd.get_envs()
                .any(|(key, value)| key == "ANTHROPIC_API_KEY" && value.is_none()),
            "the API key must be stripped from the opener's environment"
        );
    }
}
