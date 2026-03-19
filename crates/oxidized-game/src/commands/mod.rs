//! Brigadier-compatible command framework.
//!
//! Provides a command graph (dispatcher), builder DSL, argument types,
//! tab-completion, and serialization to the `ClientboundCommandsPacket`
//! wire format. Commands are registered via a fluent builder API and
//! executed by parsing player input against the graph.

pub mod arguments;
pub mod context;
pub mod dispatcher;
pub mod nodes;
pub mod pagination;
pub mod selector;
pub mod serializer;
pub mod source;

// Core command implementations
mod cmd_difficulty;
mod cmd_effect;
mod cmd_gamemode;
mod cmd_gamerule;
mod cmd_give;
mod cmd_help;
mod cmd_kick;
mod cmd_kill;
mod cmd_list;
mod cmd_say;
mod cmd_seed;
mod cmd_setblock;
mod cmd_stop;
mod cmd_time;
mod cmd_tp;
mod cmd_weather;

pub use arguments::{ArgumentType, StringKind};
pub use context::{CommandContext, ParsedArgument, StringRange};
pub use dispatcher::CommandDispatcher;
pub use nodes::{ArgumentCommandNode, CommandNode, LiteralCommandNode, RootCommandNode};
pub use pagination::PaginatedMessage;
pub use serializer::{CommandNodeData, CommandTreeData};
pub use selector::{SelectorKind, SelectorTarget};
pub use source::{CommandSourceKind, CommandSourceStack};

/// The command system hub: registers all commands, provides dispatch and
/// tab-completion, and serializes the command tree for the client.
pub struct Commands {
    dispatcher: CommandDispatcher<CommandSourceStack>,
}

impl Commands {
    /// Creates a new command system with all vanilla commands registered.
    pub fn new() -> Self {
        let mut d = CommandDispatcher::new();
        cmd_stop::register(&mut d);
        cmd_tp::register(&mut d);
        cmd_gamemode::register(&mut d);
        cmd_give::register(&mut d);
        cmd_kill::register(&mut d);
        cmd_time::register(&mut d);
        cmd_weather::register(&mut d);
        cmd_say::register(&mut d);
        cmd_list::register(&mut d);
        cmd_kick::register(&mut d);
        cmd_difficulty::register(&mut d);
        cmd_help::register(&mut d);
        cmd_seed::register(&mut d);
        cmd_setblock::register(&mut d);
        cmd_effect::register(&mut d);
        cmd_gamerule::register(&mut d);
        Self { dispatcher: d }
    }

    /// Parse and execute a command string (without leading `/`).
    pub fn dispatch(&self, input: &str, source: CommandSourceStack) -> Result<i32, CommandError> {
        let input = input.trim_start_matches('/');
        let parse = self.dispatcher.parse(input, source)?;
        self.dispatcher.execute(&parse)
    }

    /// Collect tab-completions for the given partial input.
    pub fn completions(
        &self,
        input: &str,
        source: &CommandSourceStack,
    ) -> Vec<context::Suggestion> {
        let player_names = source.server.online_player_names();
        self.dispatcher
            .get_completions(input, source, &player_names)
    }

    /// Serialize the command tree for `ClientboundCommandsPacket`.
    pub fn serialize_tree(&self, source: &CommandSourceStack) -> CommandTreeData {
        self.dispatcher.serialize_tree(source)
    }

    /// Returns a reference to the underlying dispatcher.
    pub fn dispatcher(&self) -> &CommandDispatcher<CommandSourceStack> {
        &self.dispatcher
    }
}

impl Default for Commands {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors from command parsing or execution.
#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    /// The command could not be parsed (unknown command, bad syntax).
    #[error("{0}")]
    Parse(String),
    /// The command was parsed but execution failed.
    #[error("{0}")]
    Execution(String),
}
