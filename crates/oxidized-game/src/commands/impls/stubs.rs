//! Stub registrations for commands that are not yet implemented.
//!
//! Each entry maps a command name (plus optional aliases), description,
//! and required permission level. Running a stub command reports
//! `"/<name> is not yet implemented"` to the player.

use crate::commands::context::CommandContext;
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::literal;
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// (name, description, permission_level, aliases)
const STUBS: &[(&str, &str, u32, &[&str])] = &[
    ("advancement", "Grant or revoke advancements", 2, &[]),
    ("attribute", "Query or modify entity attributes", 2, &[]),
    ("ban", "Ban a player from the server", 3, &[]),
    ("ban-ip", "Ban an IP address from the server", 3, &[]),
    ("banlist", "List banned players and IPs", 3, &[]),
    ("bossbar", "Manage boss bars", 2, &[]),
    ("clear", "Clear items from inventory", 2, &[]),
    ("clone", "Clone blocks from one region to another", 2, &[]),
    ("damage", "Deal damage to entities", 2, &[]),
    ("data", "Get, merge, modify, or remove NBT data", 2, &[]),
    ("datapack", "Manage data packs", 2, &[]),
    ("debug", "Start or stop a debug profiling session", 3, &[]),
    ("defaultgamemode", "Set the default game mode", 2, &[]),
    ("deop", "Revoke operator status", 3, &[]),
    ("enchant", "Enchant items", 2, &[]),
    ("execute", "Execute a command with modified context", 2, &[]),
    ("experience", "Add or query player experience", 2, &["xp"]),
    ("fill", "Fill a region with a specific block", 2, &[]),
    ("fillbiome", "Fill a region with a specific biome", 2, &[]),
    ("forceload", "Force chunks to stay loaded", 2, &[]),
    ("function", "Run a function", 2, &[]),
    ("item", "Manipulate items in inventories", 2, &[]),
    ("jfr", "Start or stop JFR profiling", 4, &[]),
    ("locate", "Locate structures, biomes, or POIs", 2, &[]),
    ("loot", "Drop or give loot from a loot table", 2, &[]),
    ("me", "Send an action message", 0, &[]),
    ("msg", "Send a private message", 0, &["tell", "w"]),
    ("op", "Grant operator status", 3, &[]),
    ("pardon", "Remove a player from the ban list", 3, &[]),
    ("pardon-ip", "Remove an IP from the ban list", 3, &[]),
    ("particle", "Create particles", 2, &[]),
    ("perf", "Capture performance metrics", 4, &[]),
    ("place", "Place features, structures, or templates", 2, &[]),
    ("playsound", "Play a sound", 2, &[]),
    ("publish", "Open the server to LAN", 4, &[]),
    ("raid", "Manage raids", 3, &[]),
    (
        "random",
        "Generate random values or manage sequences",
        2,
        &[],
    ),
    ("recipe", "Give or take recipes", 2, &[]),
    ("reload", "Reload data packs and functions", 2, &[]),
    ("ride", "Mount or dismount entities", 2, &[]),
    ("rotate", "Rotate entities", 2, &[]),
    ("save-all", "Save the server to disk", 4, &[]),
    ("save-off", "Disable automatic saving", 4, &[]),
    ("save-on", "Enable automatic saving", 4, &[]),
    ("schedule", "Schedule a function to run later", 2, &[]),
    ("scoreboard", "Manage scoreboards and objectives", 2, &[]),
    ("setworldspawn", "Set the world spawn point", 2, &[]),
    ("spawnpoint", "Set a player spawn point", 2, &[]),
    ("spectate", "Make a spectator spectate an entity", 2, &[]),
    ("spreadplayers", "Spread players around a point", 2, &[]),
    ("stopsound", "Stop playing sounds", 2, &[]),
    ("summon", "Summon an entity", 2, &[]),
    ("tag", "Manage entity tags", 2, &[]),
    ("team", "Manage teams", 2, &[]),
    ("teammsg", "Send a message to team members", 0, &["tm"]),
    ("tellraw", "Send a JSON text message", 2, &[]),
    ("tick", "Control the tick rate", 3, &[]),
    ("title", "Manage titles displayed to players", 2, &[]),
    ("transfer", "Transfer players to another server", 3, &[]),
    ("trigger", "Modify a trigger scoreboard objective", 0, &[]),
    ("whitelist", "Manage the server whitelist", 3, &[]),
    ("worldborder", "Manage the world border", 2, &[]),
];

/// Registers a single stub command.
fn register_one(
    d: &mut CommandDispatcher<CommandSourceStack>,
    name: &'static str,
    description: &'static str,
    permission_level: u32,
) {
    d.register(
        literal(name)
            .description(description)
            .requires(move |s: &CommandSourceStack| s.has_permission(permission_level))
            .executes(move |ctx: &CommandContext<CommandSourceStack>| {
                ctx.source
                    .send_failure(&Component::translatable("commands.help.failed", vec![]));
                ctx.source
                    .send_failure(&Component::text(format!("/{name} is not yet implemented")));
                Ok(0)
            }),
    );
}

/// Registers all unimplemented stub commands (and their aliases).
pub(super) fn register_all(d: &mut CommandDispatcher<CommandSourceStack>) {
    for &(name, description, perm, aliases) in STUBS {
        register_one(d, name, description, perm);
        for &alias in aliases {
            register_one(d, alias, description, perm);
        }
    }
}
