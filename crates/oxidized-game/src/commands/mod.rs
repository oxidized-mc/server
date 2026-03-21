//! Brigadier-compatible command framework.
//!
//! Provides a command graph (dispatcher), builder DSL, argument types,
//! tab-completion, and serialization to the `ClientboundCommandsPacket`
//! wire format. Commands are registered via a fluent builder API and
//! executed by parsing player input against the graph.

pub mod argument_access;
pub mod argument_parser;
pub mod arguments;
pub mod context;
pub mod coordinates;
pub mod dispatcher;
pub mod nodes;
pub mod pagination;
pub mod selector;
pub mod serializer;
pub mod source;
pub mod string_reader;
mod impls;

pub use argument_access::{
    get_block_pos, get_bool, get_double, get_entities, get_entity, get_float, get_game_profile,
    get_gamemode, get_integer, get_long, get_string, get_time, get_vec3,
};
pub use argument_parser::parse_argument;
pub use arguments::{ArgumentType, StringKind};
pub use context::{CommandContext, ParsedArgument, StringRange};
pub use coordinates::{CoordinateKind, Coordinates, EntityAnchorKind, WorldCoordinate};
pub use dispatcher::CommandDispatcher;
pub use nodes::{ArgumentCommandNode, CommandNode, LiteralCommandNode, RootCommandNode};
pub use pagination::PaginatedMessage;
pub use selector::{EntitySelector, SelectorFilters, SelectorKind, SelectorTarget};
pub use serializer::{CommandNodeData, CommandTreeData};
pub use source::{CommandSourceKind, CommandSourceStack};
pub use string_reader::StringReader;

/// The command system hub: registers all commands, provides dispatch and
/// tab-completion, and serializes the command tree for the client.
pub struct Commands {
    dispatcher: CommandDispatcher<CommandSourceStack>,
}

impl Commands {
    /// Creates a new command system with all vanilla commands registered.
    pub fn new() -> Self {
        let mut d = CommandDispatcher::new();
        impls::register_all(&mut d);
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
