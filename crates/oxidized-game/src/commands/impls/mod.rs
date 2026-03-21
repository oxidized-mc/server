//! Concrete command implementations.
//!
//! Each `cmd_*.rs` module registers one or more commands with the dispatcher.
//! `stubs.rs` provides placeholder commands for features not yet implemented.

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
mod stubs;

use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::source::CommandSourceStack;

/// Registers all implemented and stub commands with the dispatcher.
pub(crate) fn register_all(d: &mut CommandDispatcher<CommandSourceStack>) {
    cmd_stop::register(d);
    cmd_tp::register(d);
    cmd_gamemode::register(d);
    cmd_give::register(d);
    cmd_kill::register(d);
    cmd_time::register(d);
    cmd_weather::register(d);
    cmd_say::register(d);
    cmd_list::register(d);
    cmd_kick::register(d);
    cmd_difficulty::register(d);
    cmd_help::register(d);
    cmd_seed::register(d);
    cmd_setblock::register(d);
    cmd_effect::register(d);
    cmd_gamerule::register(d);
    stubs::register_all(d);
}
