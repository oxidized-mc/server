//! `/effect` command — give or clear status effects.
//!
//! TODO: Requires a status-effect system on entities (ECS components for
//! active effects, tick-based duration, amplifier, particle visibility).
//! Also needs `ClientboundUpdateMobEffectPacket` / `ClientboundRemoveMobEffectPacket`.

use crate::commands::arguments::ArgumentType;
use crate::commands::argument_access::{get_entities, get_integer, get_string};
use crate::commands::context::CommandContext;
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::{argument, literal};
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/effect` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("effect")
            .description("Add or remove status effects")
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
                        .executes(effect_give)
                        .then(
                            argument(
                                "seconds",
                                ArgumentType::Integer {
                                    min: Some(1),
                                    max: Some(1_000_000),
                                },
                            )
                            .executes(effect_give),
                        ),
                    ),
                ),
            )
            // /effect clear [targets] [effect]
            .then(
                literal("clear")
                    .executes(|ctx: &CommandContext<CommandSourceStack>| {
                        // TODO: Clear all effects from command sender
                        ctx.source.send_success(
                            &Component::translatable(
                                "commands.effect.clear.everything.success.single",
                                vec![Component::text(ctx.source.display_name.clone())],
                            ),
                            true,
                        );
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
                                let targets = get_entities(ctx, "targets")?;
                                // TODO: Clear all effects from targets
                                for target in &targets {
                                    ctx.source.send_success(
                                        &Component::translatable(
                                            "commands.effect.clear.everything.success.single",
                                            vec![Component::text(&target.name)],
                                        ),
                                        true,
                                    );
                                }
                                Ok(1)
                            },
                        ),
                    ),
            ),
    );
}

fn effect_give(
    ctx: &CommandContext<CommandSourceStack>,
) -> Result<i32, crate::commands::CommandError> {
    let targets = get_entities(ctx, "targets")?;
    let effect = get_string(ctx, "effect")?;
    let seconds = get_integer(ctx, "seconds").unwrap_or(30);
    // TODO: Apply status effect to targets
    // Vanilla arg order: effect name, target name, duration in seconds
    for target in &targets {
        ctx.source.send_success(
            &Component::translatable(
                "commands.effect.give.success.single",
                vec![
                    Component::text(effect),
                    Component::text(&target.name),
                    Component::text(seconds.to_string()),
                ],
            ),
            true,
        );
    }
    Ok(1)
}
