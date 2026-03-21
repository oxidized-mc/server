//! Interactive server console with tab-completion.
//!
//! Uses `rustyline` to provide a readline-style interface with command
//! name and argument completion powered by the Brigadier command tree.

use std::sync::Arc;

use oxidized_game::commands::source::{CommandSourceKind, CommandSourceStack, ServerHandle};
use oxidized_protocol::chat::Component;
use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{Editor, Helper};
use tracing::debug;

use crate::network::ServerContext;

/// Rustyline helper providing tab-completion for server commands.
struct ConsoleHelper {
    server_ctx: Arc<ServerContext>,
}

impl ConsoleHelper {
    /// Build a console [`CommandSourceStack`] for completion lookups.
    fn console_source(&self) -> CommandSourceStack {
        CommandSourceStack {
            source: CommandSourceKind::Console,
            position: (0.0, 0.0, 0.0),
            rotation: (0.0, 0.0),
            permission_level: 4,
            display_name: "Server".to_string(),
            server: self.server_ctx.clone(),
            feedback_sender: Arc::new(|_: &Component| {}),
            silent: true,
        }
    }
}

impl Completer for ConsoleHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &rustyline::Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let input = &line[..pos];
        let source = self.console_source();
        let suggestions = self.server_ctx.commands.completions(input, &source);

        if suggestions.is_empty() {
            return Ok((pos, Vec::new()));
        }

        let start = suggestions[0].range.start;
        let pairs = suggestions
            .into_iter()
            .map(|s| Pair {
                display: s.text.clone(),
                replacement: s.text,
            })
            .collect();

        Ok((start, pairs))
    }
}

impl Hinter for ConsoleHelper {
    type Hint = String;
}

impl Highlighter for ConsoleHelper {}

impl Validator for ConsoleHelper {}

impl Helper for ConsoleHelper {}

/// Runs the interactive console loop with tab-completion.
///
/// Blocks until stdin is closed, an unrecoverable error occurs, or
/// Ctrl+C is pressed (which triggers server shutdown). Must be called
/// from a dedicated OS thread — **not** a Tokio async task.
#[allow(clippy::needless_pass_by_value)] // Arc is moved into the function intentionally
pub fn run_console_loop(server_ctx: Arc<ServerContext>) {
    let helper = ConsoleHelper {
        server_ctx: server_ctx.clone(),
    };

    let config = rustyline::Config::builder()
        .auto_add_history(true)
        .build();

    let mut rl = match Editor::with_config(config) {
        Ok(editor) => editor,
        Err(e) => {
            tracing::error!(error = %e, "Failed to initialize console editor");
            return;
        },
    };
    rl.set_helper(Some(helper));

    loop {
        match rl.readline("> ") {
            Ok(line) => {
                let line = line.trim().to_string();
                if line.is_empty() {
                    continue;
                }

                let source = CommandSourceStack {
                    source: CommandSourceKind::Console,
                    position: (0.0, 0.0, 0.0),
                    rotation: (0.0, 0.0),
                    permission_level: 4,
                    display_name: "Server".to_string(),
                    server: server_ctx.clone(),
                    feedback_sender: Arc::new(|component: &Component| {
                        println!("{component}");
                    }),
                    silent: false,
                };

                match server_ctx.commands.dispatch(&line, source) {
                    Ok(_) => {
                        debug!(command = %line, "Console command executed");
                    },
                    Err(e) => {
                        eprintln!("Error: {e}");
                    },
                }
            },
            Err(ReadlineError::Interrupted) => {
                // Ctrl+C — trigger graceful shutdown.
                server_ctx.request_shutdown();
                break;
            },
            Err(ReadlineError::Eof) => break,
            Err(e) => {
                debug!(error = %e, "Console read error");
                break;
            },
        }
    }
}
