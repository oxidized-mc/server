//! Brigadier-compatible command framework — re-exported from `oxidized-commands`
//! plus game-specific extensions (source, selector, pagination, implementations).
//!
//! The core command engine lives in the standalone `oxidized-commands` crate.
//! This module re-exports it and adds Minecraft-specific types (CommandSourceStack,
//! entity selectors, command implementations).

// Re-export the core command library modules so existing `crate::commands::*`
// paths continue to work.
pub mod argument_access {
    //! Typed argument getters — re-exports from `oxidized_commands` plus
    //! game-specific accessors.

    pub use oxidized_commands::argument_access::*;

    use crate::commands::CommandError;
    use crate::commands::source::CommandSourceStack;
    use oxidized_commands::context::{ArgumentResult, CommandContext};

    /// Gets a gamemode argument by name, resolved to the game's [`GameType`].
    ///
    /// # Errors
    ///
    /// Returns [`CommandError::Parse`] if no argument named `name` exists or
    /// the argument is not a game mode.
    pub fn get_gamemode(
        ctx: &CommandContext<CommandSourceStack>,
        name: &str,
    ) -> Result<oxidized_protocol::types::game_type::GameType, CommandError> {
        let s = oxidized_commands::get_gamemode_str(ctx, name)?;
        oxidized_protocol::types::game_type::GameType::by_name(s)
            .ok_or_else(|| CommandError::Parse(format!("Unknown game mode: '{s}'")))
    }

    /// Gets a block position argument by name. Resolves relative/local coordinates
    /// using the source position.
    pub fn get_block_pos(
        ctx: &CommandContext<CommandSourceStack>,
        name: &str,
    ) -> Result<(i32, i32, i32), CommandError> {
        let arg = ctx
            .arguments
            .get(name)
            .map(|a| &a.result)
            .ok_or_else(|| CommandError::Parse(format!("No argument named '{name}'")))?;
        match arg {
            ArgumentResult::BlockPos(x, y, z) => Ok((*x, *y, *z)),
            ArgumentResult::Coordinates(coords) => {
                Ok(coords.resolve_block_pos(ctx.source.position, ctx.source.rotation))
            },
            _ => Err(CommandError::Parse(format!(
                "Argument '{name}' is not a block position"
            ))),
        }
    }

    /// Gets resolved entity targets from a string argument, handling selectors.
    pub fn get_entities(
        ctx: &CommandContext<CommandSourceStack>,
        name: &str,
    ) -> Result<Vec<crate::commands::selector::SelectorTarget>, CommandError> {
        let input = oxidized_commands::get_string(ctx, name)?;
        crate::commands::selector::resolve_entities(input, &ctx.source)
    }

    /// Gets a single entity target from a string argument.
    pub fn get_entity(
        ctx: &CommandContext<CommandSourceStack>,
        name: &str,
    ) -> Result<crate::commands::selector::SelectorTarget, CommandError> {
        let targets = get_entities(ctx, name)?;
        if targets.len() != 1 {
            return Err(CommandError::Parse(format!(
                "Expected exactly one entity, got {}",
                targets.len()
            )));
        }
        let mut iter = targets.into_iter();
        iter.next()
            .ok_or_else(|| CommandError::Parse("Expected exactly one entity, got 0".to_string()))
    }

    /// Gets a player name from a string argument (for `GameProfile` args).
    pub fn get_game_profile(
        ctx: &CommandContext<CommandSourceStack>,
        name: &str,
    ) -> Result<crate::commands::selector::SelectorTarget, CommandError> {
        get_entity(ctx, name)
    }

    /// Gets a vec3 argument by name. Resolves relative/local coordinates
    /// using the source position.
    pub fn get_vec3(
        ctx: &CommandContext<CommandSourceStack>,
        name: &str,
    ) -> Result<(f64, f64, f64), CommandError> {
        let arg = ctx
            .arguments
            .get(name)
            .map(|a| &a.result)
            .ok_or_else(|| CommandError::Parse(format!("No argument named '{name}'")))?;
        match arg {
            ArgumentResult::Vec3(x, y, z) => Ok((*x, *y, *z)),
            ArgumentResult::Coordinates(coords) => {
                Ok(coords.resolve(ctx.source.position, ctx.source.rotation))
            },
            _ => Err(CommandError::Parse(format!(
                "Argument '{name}' is not a vec3"
            ))),
        }
    }

    /// Resolves an entity selector argument and invokes `action` for each target.
    ///
    /// # Errors
    ///
    /// Returns [`CommandError`] if the selector cannot be resolved or if `action`
    /// returns an error for any target.
    pub fn for_each_target<F>(
        ctx: &CommandContext<CommandSourceStack>,
        selector_name: &str,
        mut action: F,
    ) -> Result<i32, CommandError>
    where
        F: FnMut(
            &CommandSourceStack,
            &crate::commands::selector::SelectorTarget,
        ) -> Result<(), CommandError>,
    {
        let targets = get_entities(ctx, selector_name)?;
        let mut count = 0;
        for target in &targets {
            action(&ctx.source, target)?;
            count += 1;
        }
        Ok(count)
    }
}

pub mod argument_parser {
    //! Argument parsing — re-exports from `oxidized_commands`.
    pub use oxidized_commands::argument_parser::*;
}
pub mod arguments {
    //! Argument types — re-exports from `oxidized_commands`.
    pub use oxidized_commands::arguments::*;
}
pub mod context {
    //! Command context types — re-exports from `oxidized_commands`.
    pub use oxidized_commands::context::*;
}
pub mod coordinates {
    //! Coordinate parsing — re-exports from `oxidized_commands`.
    pub use oxidized_commands::coordinates::*;
}
pub mod dispatcher {
    //! Command dispatcher — re-exports from `oxidized_commands`.
    pub use oxidized_commands::dispatcher::*;
}
pub mod nodes {
    //! Command node types — re-exports from `oxidized_commands` plus
    //! `CommandSourceStack` convenience methods via extension traits.

    pub use oxidized_commands::nodes::*;

    use crate::commands::source::CommandSourceStack;

    /// Extension trait providing permission-based builder methods for
    /// `LiteralBuilder<CommandSourceStack>`.
    pub trait LiteralBuilderExt {
        /// Marks this command as requiring operator status (permission level ≥ 2).
        ///
        /// Console sources always pass this check.
        fn requires_op(self) -> Self;

        /// Marks this command as requiring a specific permission level.
        ///
        /// Console sources always pass this check.
        /// Common levels: 2 = gamemaster, 3 = admin, 4 = owner.
        fn requires_op_level(self, level: u32) -> Self;
    }

    impl LiteralBuilderExt for LiteralBuilder<CommandSourceStack> {
        fn requires_op(self) -> Self {
            self.requires(|s: &CommandSourceStack| s.has_permission(2))
        }

        fn requires_op_level(self, level: u32) -> Self {
            self.requires(move |s: &CommandSourceStack| s.has_permission(level))
        }
    }

    /// Extension trait providing permission-based builder methods for
    /// `ArgumentBuilder<CommandSourceStack>`.
    pub trait ArgumentBuilderExt {
        /// Marks this argument as requiring operator status (permission level ≥ 2).
        ///
        /// Console sources always pass this check.
        fn requires_op(self) -> Self;

        /// Marks this argument as requiring a specific permission level.
        ///
        /// Console sources always pass this check.
        /// Common levels: 2 = gamemaster, 3 = admin, 4 = owner.
        fn requires_op_level(self, level: u32) -> Self;
    }

    impl ArgumentBuilderExt for ArgumentBuilder<CommandSourceStack> {
        fn requires_op(self) -> Self {
            self.requires(|s: &CommandSourceStack| s.has_permission(2))
        }

        fn requires_op_level(self, level: u32) -> Self {
            self.requires(move |s: &CommandSourceStack| s.has_permission(level))
        }
    }
}
pub mod serializer {
    //! Command tree serializer — re-exports from `oxidized_commands`.
    pub use oxidized_commands::serializer::*;
}
pub mod string_reader {
    //! String reader — re-exports from `oxidized_commands`.
    pub use oxidized_commands::string_reader::*;
}

// Game-specific modules (not in oxidized-commands)
mod impls;
pub mod pagination;
pub mod selector;
pub mod source;

pub use argument_access::{
    for_each_target, get_block_pos, get_bool, get_double, get_entities, get_entity, get_float,
    get_game_profile, get_gamemode, get_integer, get_long, get_string, get_time, get_vec3,
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

/// Re-export the core `CommandError` type from the library.
pub use oxidized_commands::CommandError;

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
    ///
    /// # Errors
    ///
    /// Returns [`CommandError`] if parsing or execution fails.
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

    /// Registers additional commands via a callback that receives the
    /// underlying dispatcher. Intended for plugin or extension code that
    /// needs to add commands after the built-in set is registered.
    pub fn register(&mut self, f: impl FnOnce(&mut CommandDispatcher<CommandSourceStack>)) {
        f(&mut self.dispatcher);
    }
}

impl Default for Commands {
    fn default() -> Self {
        Self::new()
    }
}
