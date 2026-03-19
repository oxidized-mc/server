//! `/gamemode` command — change game mode.
//!
//! TODO: Actually changing a player's gamemode requires updating their
//! `ServerPlayer.game_mode`, syncing abilities, and sending
//! `ClientboundGameEventPacket` to the client. Not yet available.

use crate::commands::arguments::ArgumentType;
use crate::commands::context::{CommandContext, get_gamemode, get_string};
use crate::commands::dispatcher::CommandDispatcher;
use crate::commands::nodes::{argument, literal};
use crate::commands::source::CommandSourceStack;
use oxidized_protocol::chat::Component;

/// Registers the `/gamemode` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("gamemode")
            .description("Sets a player's game mode")
            .requires(|s: &CommandSourceStack| s.has_permission(2))
            .then(
                argument("gamemode", ArgumentType::Gamemode)
                    // /gamemode <mode> — apply to self
                    .executes(|ctx: &CommandContext<CommandSourceStack>| {
                        let gm = get_gamemode(ctx, "gamemode")?;
                        // TODO: Actually set gamemode on the source player
                        ctx.source.send_success(
                            &Component::translatable(
                                "commands.gamemode.success.self",
                                vec![Component::translatable(gm.translation_key(), vec![])],
                            ),
                            true,
                        );
                        Ok(1)
                    })
                    // /gamemode <mode> <target>
                    .then(
                        argument(
                            "target",
                            ArgumentType::Entity {
                                single: false,
                                player_only: true,
                            },
                        )
                        .executes(
                            |ctx: &CommandContext<CommandSourceStack>| {
                                let gm = get_gamemode(ctx, "gamemode")?;
                                let target = get_string(ctx, "target")?;
                                // TODO: Actually set gamemode on the target player(s)
                                ctx.source.send_success(
                                    &Component::translatable(
                                        "commands.gamemode.success.other",
                                        vec![
                                            Component::text(target),
                                            Component::translatable(
                                                gm.translation_key(),
                                                vec![],
                                            ),
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
