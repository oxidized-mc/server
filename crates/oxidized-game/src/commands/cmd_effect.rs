//! `/effect` command — give or clear status effects.

use crate::commands::arguments::ArgumentType;
use crate::commands::context::{CommandContext, get_integer, get_string};
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::{argument, literal};
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/effect` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("effect")
            .requires(|s: &CommandSourceStack| s.has_permission(2))
            // /effect give <targets> <effect> [seconds] [amplifier] [hideParticles]
            .then(
                literal("give").then(
                    argument(
                        "targets",
                        ArgumentType::Entity {
                            single: false,
                            player_only: false,
                        },
                    )
                    .then(
                        argument(
                            "effect",
                            ArgumentType::Resource {
                                registry: "minecraft:mob_effect".to_string(),
                            },
                        )
                        .executes(|ctx: &CommandContext<CommandSourceStack>| {
                            let targets = get_string(ctx, "targets")?;
                            let effect = get_string(ctx, "effect")?;
                            ctx.source.send_success(
                                &Component::text(format!("Applied effect {effect} to {targets}")),
                                true,
                            );
                            Ok(1)
                        })
                        .then(
                            argument(
                                "seconds",
                                ArgumentType::Integer {
                                    min: Some(1),
                                    max: Some(1_000_000),
                                },
                            )
                            .executes(
                                |ctx: &CommandContext<CommandSourceStack>| {
                                    let targets = get_string(ctx, "targets")?;
                                    let effect = get_string(ctx, "effect")?;
                                    let seconds = get_integer(ctx, "seconds")?;
                                    ctx.source.send_success(
                                        &Component::text(format!(
                                            "Applied effect {effect} for {seconds}s to {targets}"
                                        )),
                                        true,
                                    );
                                    Ok(1)
                                },
                            ),
                        ),
                    ),
                ),
            )
            // /effect clear [targets] [effect]
            .then(
                literal("clear")
                    .executes(|ctx: &CommandContext<CommandSourceStack>| {
                        ctx.source
                            .send_success(&Component::text("Removed every effect"), true);
                        Ok(1)
                    })
                    .then(
                        argument(
                            "targets",
                            ArgumentType::Entity {
                                single: false,
                                player_only: false,
                            },
                        )
                        .executes(
                            |ctx: &CommandContext<CommandSourceStack>| {
                                let targets = get_string(ctx, "targets")?;
                                ctx.source.send_success(
                                    &Component::text(format!(
                                        "Removed every effect from {targets}"
                                    )),
                                    true,
                                );
                                Ok(1)
                            },
                        ),
                    ),
            ),
    );
}
