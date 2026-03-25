//! `/effect` command — give or clear status effects.
//!
//! TODO: Requires a status-effect system on entities (ECS components for
//! active effects, tick-based duration, amplifier, particle visibility).
//! Also needs `ClientboundUpdateMobEffectPacket` / `ClientboundRemoveMobEffectPacket`.

use crate::commands::arguments::ArgumentType;
use crate::commands::context::CommandContext;
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::{argument, literal};
use crate::commands::source::CommandSourceStack;

/// Registers the `/effect` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("effect")
            .description("Add or remove status effects")
            .requires_op()
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
                    .executes(|_ctx: &CommandContext<CommandSourceStack>| {
                        // TODO: Clear all effects from command sender
                        Err(crate::commands::CommandError::NotImplemented(
                            "effect clear".into(),
                        ))
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
                            |_ctx: &CommandContext<CommandSourceStack>| {
                                // TODO: Clear all effects from targets
                                Err(crate::commands::CommandError::NotImplemented(
                                    "effect clear".into(),
                                ))
                            },
                        ),
                    ),
            ),
    );
}

fn effect_give(
    _ctx: &CommandContext<CommandSourceStack>,
) -> Result<i32, crate::commands::CommandError> {
    // TODO: Apply status effect to targets — requires a status-effect
    // system on entities (ECS components for active effects, tick-based
    // duration, amplifier) and ClientboundUpdateMobEffectPacket.
    Err(crate::commands::CommandError::NotImplemented(
        "effect give".into(),
    ))
}
