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
mod cmd_advancement;
mod cmd_attribute;
mod cmd_ban;
mod cmd_ban_ip;
mod cmd_banlist;
mod cmd_bossbar;
mod cmd_clear;
mod cmd_clone;
mod cmd_damage;
mod cmd_data;
mod cmd_datapack;
mod cmd_debug;
mod cmd_defaultgamemode;
mod cmd_deop;
mod cmd_difficulty;
mod cmd_effect;
mod cmd_enchant;
mod cmd_execute;
mod cmd_experience;
mod cmd_fill;
mod cmd_fillbiome;
mod cmd_forceload;
mod cmd_function;
mod cmd_gamemode;
mod cmd_gamerule;
mod cmd_give;
mod cmd_help;
mod cmd_item;
mod cmd_jfr;
mod cmd_kick;
mod cmd_kill;
mod cmd_list;
mod cmd_locate;
mod cmd_loot;
mod cmd_me;
mod cmd_msg;
mod cmd_op;
mod cmd_pardon;
mod cmd_pardon_ip;
mod cmd_particle;
mod cmd_perf;
mod cmd_place;
mod cmd_playsound;
mod cmd_publish;
mod cmd_raid;
mod cmd_random;
mod cmd_recipe;
mod cmd_reload;
mod cmd_ride;
mod cmd_rotate;
mod cmd_save_all;
mod cmd_save_off;
mod cmd_save_on;
mod cmd_say;
mod cmd_schedule;
mod cmd_scoreboard;
mod cmd_seed;
mod cmd_setblock;
mod cmd_setworldspawn;
mod cmd_spawnpoint;
mod cmd_spectate;
mod cmd_spreadplayers;
mod cmd_stop;
mod cmd_stopsound;
mod cmd_summon;
mod cmd_tag;
mod cmd_team;
mod cmd_teammsg;
mod cmd_tellraw;
mod cmd_tick;
mod cmd_time;
mod cmd_title;
mod cmd_tp;
mod cmd_transfer;
mod cmd_trigger;
mod cmd_weather;
mod cmd_whitelist;
mod cmd_worldborder;

pub use arguments::{ArgumentType, StringKind};
pub use context::{CommandContext, ParsedArgument, StringRange};
pub use dispatcher::CommandDispatcher;
pub use nodes::{ArgumentCommandNode, CommandNode, LiteralCommandNode, RootCommandNode};
pub use pagination::PaginatedMessage;
pub use selector::{SelectorKind, SelectorTarget};
pub use serializer::{CommandNodeData, CommandTreeData};
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
        // Implemented commands
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
        // Stub commands (not yet fully implemented)
        cmd_advancement::register(&mut d);
        cmd_attribute::register(&mut d);
        cmd_ban::register(&mut d);
        cmd_ban_ip::register(&mut d);
        cmd_banlist::register(&mut d);
        cmd_bossbar::register(&mut d);
        cmd_clear::register(&mut d);
        cmd_clone::register(&mut d);
        cmd_damage::register(&mut d);
        cmd_data::register(&mut d);
        cmd_datapack::register(&mut d);
        cmd_debug::register(&mut d);
        cmd_defaultgamemode::register(&mut d);
        cmd_deop::register(&mut d);
        cmd_enchant::register(&mut d);
        cmd_execute::register(&mut d);
        cmd_experience::register(&mut d);
        cmd_fill::register(&mut d);
        cmd_fillbiome::register(&mut d);
        cmd_forceload::register(&mut d);
        cmd_function::register(&mut d);
        cmd_item::register(&mut d);
        cmd_jfr::register(&mut d);
        cmd_locate::register(&mut d);
        cmd_loot::register(&mut d);
        cmd_me::register(&mut d);
        cmd_msg::register(&mut d);
        cmd_op::register(&mut d);
        cmd_pardon::register(&mut d);
        cmd_pardon_ip::register(&mut d);
        cmd_particle::register(&mut d);
        cmd_perf::register(&mut d);
        cmd_place::register(&mut d);
        cmd_playsound::register(&mut d);
        cmd_publish::register(&mut d);
        cmd_raid::register(&mut d);
        cmd_random::register(&mut d);
        cmd_recipe::register(&mut d);
        cmd_reload::register(&mut d);
        cmd_ride::register(&mut d);
        cmd_rotate::register(&mut d);
        cmd_save_all::register(&mut d);
        cmd_save_off::register(&mut d);
        cmd_save_on::register(&mut d);
        cmd_schedule::register(&mut d);
        cmd_scoreboard::register(&mut d);
        cmd_setworldspawn::register(&mut d);
        cmd_spawnpoint::register(&mut d);
        cmd_spectate::register(&mut d);
        cmd_spreadplayers::register(&mut d);
        cmd_stopsound::register(&mut d);
        cmd_summon::register(&mut d);
        cmd_tag::register(&mut d);
        cmd_team::register(&mut d);
        cmd_teammsg::register(&mut d);
        cmd_tellraw::register(&mut d);
        cmd_tick::register(&mut d);
        cmd_title::register(&mut d);
        cmd_transfer::register(&mut d);
        cmd_trigger::register(&mut d);
        cmd_whitelist::register(&mut d);
        cmd_worldborder::register(&mut d);
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
