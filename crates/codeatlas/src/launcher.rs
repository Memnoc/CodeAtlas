//! The interactive launcher: what a bare `codeatlas` at a real terminal
//! runs instead of printing usage.
//!
//! One audience, one moment: a person who just downloaded the binary and
//! runs it with nothing after its name. The launcher asks for a repository
//! path and one yes/no, then scans, serves, and opens the browser itself
//! once the served port actually answers. Everything about it is gated on
//! that audience: in any non-terminal context — scripts, pipes, CI —
//! [`should_run`] declines and bare invocation prints clap's usage exactly
//! as it did before this module existed.
//!
//! The launcher offers enrich and ask (since 0.1.6, from Memnoc's macOS
//! walk) through one backend only: the reader's own `claude` login,
//! `cli:claude`, so CodeAtlas still never handles a credential here and
//! there is no flag to pick the API-key path. In a build without that
//! backend the two rows do not exist and the launcher is exactly the
//! no-key path it was — identical in a sealed build.

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
    /// Buy prose through the reader's `claude` login before serving.
    /// Only ever true in a build with the `agent-cli` feature.
    pub enrich: bool,
    /// Serve with Ask, through the same login. Same rule.
    pub ask: bool,
}

/// The one provider the launcher can ever select: the reader's own CLI
/// login. Fixed here rather than chosen, which is the whole point.
#[cfg(feature = "agent-cli")]
fn cli_choice() -> crate::enrich::ProviderChoice<'static> {
    crate::enrich::ProviderChoice {
        spec: Some(crate::enrich::agent_cli::SPEC),
        model: None,
    }
}

/// What the launcher says when the port is already taken, *before* it
/// spends a scan on a repository it will then fail to serve: the
/// 2026-09-08 walk recorded the honest bind failure after the scan, and
/// the 2026-09-28 one asked why the browser opened at all. Now neither
/// happens — the reader is told first, and told what to do.
pub fn port_in_use_message(port: u16) -> String {
    format!(
        "port {port} on 127.0.0.1 is already in use — most likely another \
         codeatlas is serving there. Stop it (Ctrl-C in its terminal, or \
         `pkill -x codeatlas`) and run this again, or open \
         http://127.0.0.1:{port}/ to see what is already being served."
    )
}

/// The questions, over injectable ends so tests drive them with buffers
/// where a real run holds the terminal. `None` means the reader closed
/// stdin — backing out is not an error. `offers_model` adds the enrich and
/// ask questions, exactly as the modal adds its rows, and is the compiled
/// truth in a real run.
pub fn interview(
    input: &mut dyn BufRead,
    out: &mut dyn Write,
    home: Option<&Path>,
    cwd: &Path,
    offers_model: bool,
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
    #[cfg(feature = "agent-cli")]
    let Some((enrich, ask)) = model_questions(input, out, offers_model)? else {
        return Ok(None);
    };
    #[cfg(not(feature = "agent-cli"))]
    let (enrich, ask) = {
        let _ = offers_model;
        (false, false)
    };
    Ok(Some(Choices {
        root,
        open_code,
        enrich,
        ask,
    }))
}

/// The enrich and ask questions, compiled only with the backend like the
/// modal's rows: the sealed byte-probe must find no trace of the CLI's
/// name. `None` means stdin closed at one of them.
#[cfg(feature = "agent-cli")]
fn model_questions(
    input: &mut dyn BufRead,
    out: &mut dyn Write,
    offers_model: bool,
) -> io::Result<Option<(bool, bool)>> {
    if !offers_model {
        return Ok(Some((false, false)));
    }
    let _ = write!(
        out,
        "enrich first — buy prose through your `claude` login? [y/N]: "
    );
    let _ = out.flush();
    let Some(line) = read_line(input)? else {
        return Ok(None);
    };
    let enrich = parse_yes(&line);
    let _ = write!(
        out,
        "ask — answer questions in the dashboard through the same login? [y/N]: "
    );
    let _ = out.flush();
    let Some(line) = read_line(input)? else {
        return Ok(None);
    };
    Ok(Some((enrich, parse_yes(&line))))
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
    port_answers_at(PORT)
}

fn port_answers_at(port: u16) -> bool {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_ok()
}

/// The whole flow: interview → scan → (enrich) → serve, with the browser
/// opened by a helper thread once the served port genuinely answers. If
/// the port already answers *before* the scan, something else owns it —
/// the launcher says so and stops, spending nothing and opening nothing
/// at someone else's server.
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
        Err(_) => match interview(
            &mut input,
            &mut out,
            home.as_deref(),
            &cwd,
            cfg!(feature = "agent-cli"),
        ) {
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

    if port_answers() {
        eprintln!("error: {}", port_in_use_message(PORT));
        return ExitCode::FAILURE;
    }

    let graph = match crate::build_and_save_map(&choices.root) {
        Ok(graph) => graph,
        Err(err) => {
            eprintln!("error: {err:#}");
            return ExitCode::FAILURE;
        }
    };

    // Enrichment through the reader's own login, the same call `scan
    // --enrich --provider cli:claude` makes. A failure leaves the
    // structural map intact (story 14), and the launcher says so and
    // serves it anyway: the reader asked for a dashboard, and a map
    // without prose is still that.
    #[cfg(feature = "agent-cli")]
    if choices.enrich {
        let mut graph = graph;
        match crate::enrich::run(&choices.root, &mut graph, cli_choice()) {
            Ok(crate::enrich::Outcome::NothingToEnrich) => {
                eprintln!("nothing to enrich: every slot is already enriched or the map is empty");
            }
            Ok(crate::enrich::Outcome::Enriched(count)) => eprintln!("enriched {count} slots"),
            Err(err) => eprintln!("error: {err:#} (the structural map is intact — serving it)"),
        }
    }
    #[cfg(not(feature = "agent-cli"))]
    let _ = graph;

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

    let options = crate::serve::ServeOptions {
        port: PORT,
        #[cfg(feature = "agent-cli")]
        ask: choices.ask.then(cli_choice),
        #[cfg(not(feature = "agent-cli"))]
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
        let result = interview(&mut input, &mut out, home, cwd, false);
        (result, String::from_utf8(out).unwrap())
    }

    #[cfg(feature = "agent-cli")]
    #[test]
    fn with_the_cli_backend_offered_two_more_questions_follow_and_default_to_no() {
        let repo = tempfile::tempdir().unwrap();
        let ask = |script: String| {
            let mut input = Cursor::new(script);
            let mut out: Vec<u8> = Vec::new();
            let result = interview(&mut input, &mut out, None, Path::new("/"), true);
            (result.unwrap(), String::from_utf8(out).unwrap())
        };
        let (choices, prompts) = ask(format!("{}\nn\ny\ny\n", repo.path().display()));
        let choices = choices.unwrap();
        assert!(choices.enrich && choices.ask);
        assert!(
            prompts.contains("`claude` login"),
            "each question must say what it reaches a model through: {prompts:?}"
        );
        let (choices, _) = ask(format!("{}\n\n\n\n", repo.path().display()));
        let choices = choices.unwrap();
        assert!(
            !choices.enrich && !choices.ask,
            "an empty answer must not opt in"
        );
        // Closing stdin at either new question backs out like the others.
        let (choices, _) = ask(format!("{}\nn\n", repo.path().display()));
        assert!(choices.is_none());
    }

    #[test]
    fn a_taken_port_is_named_before_anything_is_scanned() {
        // The check itself, against a real listener on an OS-chosen port,
        // and the sentence the launcher prints: it names the port, the
        // likely owner, and both ways out.
        let listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        assert!(port_answers_at(port));
        drop(listener);
        assert!(!port_answers_at(port));
        let message = port_in_use_message(port);
        assert!(message.contains(&format!("port {port}")));
        assert!(message.contains("another"), "{message}");
        assert!(message.contains("pkill -x codeatlas"), "{message}");
        assert!(
            message.contains(&format!("http://127.0.0.1:{port}/")),
            "{message}"
        );
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
