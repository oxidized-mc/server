//! `/gamerule` command — query or set game rules.
//!
//! TODO: Requires a gamerule storage system (map of rule name → value),
//! propagation to clients, and per-rule side effects (e.g. `doDaylightCycle`
//! affects the tick loop). Vanilla registers each rule statically in
//! `GameRules.java`.

use crate::commands::arguments::{ArgumentType, StringKind};
use crate::commands::context::{CommandContext, get_string};
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::{argument, literal};
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/gamerule` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("gamerule")
            .description("Sets or queries a game rule value")
            .requires(|s: &CommandSourceStack| s.has_permission(2))
            // /gamerule <rule>
            .then(
                argument("rule", ArgumentType::String(StringKind::SingleWord))
                    .executes(|ctx: &CommandContext<CommandSourceStack>| {
                        let rule = get_string(ctx, "rule")?;
                        // TODO: Read actual gamerule value from storage
                        ctx.source.send_success(
                            &Component::translatable(
                                "commands.gamerule.query",
                                vec![
                                    Component::text(rule),
                                    Component::text("?"),
                                ],
                            ),
                            false,
                        );
                        Ok(1)
                    })
                    // /gamerule <rule> <value>
                    .then(
                        argument("value", ArgumentType::String(StringKind::SingleWord)).executes(
                            |ctx: &CommandContext<CommandSourceStack>| {
                                let rule = get_string(ctx, "rule")?;
                                let value = get_string(ctx, "value")?;
                                // TODO: Set gamerule value in storage + propagate
                                ctx.source.send_success(
                                    &Component::translatable(
                                        "commands.gamerule.set",
                                        vec![
                                            Component::text(rule),
                                            Component::text(value),
                                        ],
                                    ),
                                    true,
                                );
                                Ok(1)
                            },
                        ),
                    ),
            ),
    );
}
