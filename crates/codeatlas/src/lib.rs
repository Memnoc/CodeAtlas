pub mod diff;
pub mod enrich;
pub mod map;
pub mod parsers;
pub mod scan;
pub mod semantics;
pub mod serve;
pub mod share;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};

/// Which enrichment backend to use, and which model within it.
///
/// Flattened into both `scan` and `serve`: they select a backend the same
/// way through the same env var, and two copies of these definitions is two
/// places for the help text — which is built from what the binary compiled
/// in — to drift.
///
/// Both depend on the flag that turns the feature on, and that flag differs
/// (`--enrich`, `--ask`). Each carries the clap id `backend` so one
/// `requires` here resolves correctly in either subcommand; the ids are
/// internal, so the flags a reader types are still their own.
#[derive(Args)]
struct BackendArgs {
    /// Model for the enrichment backend. Like --provider, the help is built
    /// from what this binary compiled in.
    #[arg(long, requires = "backend", long_help = enrich::model_help())]
    model: Option<String>,
    /// Enrichment backend. Help text is built from the specs this build
    /// compiled in, so it never offers one the binary cannot select.
    #[arg(long, requires = "backend", long_help = enrich::provider_help())]
    provider: Option<String>,
}

impl BackendArgs {
    fn choice(&self) -> enrich::ProviderChoice<'_> {
        enrich::ProviderChoice {
            spec: self.provider.as_deref(),
            model: self.model.as_deref(),
        }
    }
}

#[derive(Parser)]
#[command(
    name = "codeatlas",
    version,
    about = "Map a codebase: structure and relationships"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Scan a repository and write .codeatlas/knowledge-graph.json
    Scan {
        /// Repository root (defaults to the current directory)
        path: Option<PathBuf>,
        /// After the structural scan, fill prose slots through an enrichment
        /// provider (ADR-0004). Which backends exist depends on how this
        /// binary was compiled — see --provider.
        #[arg(long, id = "backend")]
        enrich: bool,
        #[command(flatten)]
        backend: BackendArgs,
    },
    /// Overlay a git diff's impact on the map: changed nodes and their
    /// one-hop blast radius, written to .codeatlas/diff-overlay.json.
    /// Deterministic — git in, overlay out, no LLM, no network.
    Diff {
        /// Repository root holding .codeatlas/ (defaults to the current
        /// directory)
        path: Option<PathBuf>,
    },
    /// Print the JSON Schema of the map contract
    Schema,
    /// Export the map as one self-contained, redacted HTML file at
    /// .codeatlas/share.html — opens by double-click, no server, no
    /// external requests. LLM-enriched prose is redacted (allowlist over
    /// the map contract, ADR-0006); the artifact discloses what was
    /// removed.
    Share {
        /// Repository root holding .codeatlas/ (defaults to the current
        /// directory)
        path: Option<PathBuf>,
    },
    /// Serve the embedded dashboard and the local map on 127.0.0.1
    Serve {
        /// Repository root holding .codeatlas/ (defaults to the current
        /// directory)
        path: Option<PathBuf>,
        /// Port on 127.0.0.1 (0 lets the OS pick a free one). There is
        /// deliberately no --host: the server only ever binds loopback
        /// (ADR-0006).
        #[arg(long, default_value_t = 4173)]
        port: u16,
        /// Answer questions about the map at POST /api/ask (ADR-0009).
        /// Off by default: without it the server reaches nothing but
        /// loopback and local disk.
        #[arg(long, id = "backend", long_help = enrich::ask_help())]
        ask: bool,
        #[command(flatten)]
        backend: BackendArgs,
    },
}

pub fn run() -> ExitCode {
    match Cli::parse().command {
        Command::Scan {
            path,
            enrich,
            backend,
        } => {
            let root = path.unwrap_or_else(|| PathBuf::from("."));
            // The structural map is always built and saved first — with
            // stored annotations re-attached where content is unchanged
            // (ADR-0005) — so any enrichment failure leaves a complete map
            // behind (story 14).
            let result = scan::scan(&root).and_then(|mut graph| {
                enrich::AnnotationStore::load(&root).reattach(&root, &mut graph);
                scan::save(&root, &graph)?;
                Ok(graph)
            });
            let mut graph = match result {
                Ok(graph) => graph,
                Err(err) => {
                    eprintln!("error: {err:#}");
                    return ExitCode::FAILURE;
                }
            };
            // File nodes, not all nodes: the map holds a node per function
            // and class too, so `nodes.len()` reported roughly four times
            // the number of files scanned. Carried across five harden walks
            // as the only number the CLI states to a reader that is not
            // true; corrected here because ticket 34 was the next thing to
            // touch this function.
            let files = graph
                .nodes
                .iter()
                .filter(|n| n.kind == map::NodeKind::File)
                .count();
            eprintln!("mapped {files} files");
            if !enrich {
                return ExitCode::SUCCESS;
            }
            match enrich::run(&root, &mut graph, backend.choice()) {
                Ok(enrich::Outcome::NothingToEnrich) => {
                    eprintln!(
                        "nothing to enrich: every slot is already enriched \
                         or the map is empty"
                    );
                    ExitCode::SUCCESS
                }
                Ok(enrich::Outcome::Enriched(count)) => {
                    eprintln!("enriched {count} slots");
                    ExitCode::SUCCESS
                }
                Err(err) => {
                    eprintln!("error: {err:#} (the structural map is intact)");
                    ExitCode::FAILURE
                }
            }
        }
        Command::Diff { path } => {
            let root = path.unwrap_or_else(|| PathBuf::from("."));
            match diff::run(&root) {
                Ok(overlay) => {
                    eprintln!(
                        "{} changed nodes, {} affected — overlay at {}/{}",
                        overlay.changed.len(),
                        overlay.affected.len(),
                        scan::OUTPUT_DIR,
                        diff::OVERLAY_FILE
                    );
                    ExitCode::SUCCESS
                }
                Err(err) => {
                    eprintln!("error: {err:#}");
                    ExitCode::FAILURE
                }
            }
        }
        Command::Serve {
            path,
            port,
            ask,
            backend,
        } => {
            let root = path.unwrap_or_else(|| PathBuf::from("."));
            let options = serve::ServeOptions {
                port,
                ask: ask.then(|| backend.choice()),
            };
            match serve::serve(&root, options) {
                Ok(()) => ExitCode::SUCCESS,
                Err(err) => {
                    eprintln!("error: {err:#}");
                    ExitCode::FAILURE
                }
            }
        }
        Command::Share { path } => {
            let root = path.unwrap_or_else(|| PathBuf::from("."));
            match share::run(&root) {
                Ok(summary) => {
                    if summary.redacted.is_empty() {
                        eprintln!("nothing redacted: the map contains no LLM-enriched prose");
                    } else {
                        let fields: Vec<String> = summary
                            .redacted
                            .iter()
                            .map(|(field, count)| format!("{field} ×{count}"))
                            .collect();
                        eprintln!("redacted {}", fields.join(", "));
                    }
                    println!("share artifact at {}", summary.path.display());
                    ExitCode::SUCCESS
                }
                Err(err) => {
                    eprintln!("error: {err:#}");
                    ExitCode::FAILURE
                }
            }
        }
        Command::Schema => {
            let schema = map::contract_schema();
            println!("{}", serde_json::to_string_pretty(&schema).unwrap());
            ExitCode::SUCCESS
        }
    }
}
