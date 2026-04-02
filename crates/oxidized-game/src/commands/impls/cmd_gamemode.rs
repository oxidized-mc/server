//! `/gamemode` command — change game mode.
//!
//! Sets the game mode for the executing player or specified targets.
//! Sends `ClientboundGameEventPacket`, `ClientboundPlayerAbilitiesPacket`,
//! and `ClientboundPlayerInfoUpdatePacket` via `ServerHandle`.

use crate::commands::argument_access::{get_entities, get_gamemode};
use crate::commands::nodes::LiteralBuilderExt;
use crate::commands::source::{CommandSourceKind, CommandSourceStack};
use crate::player::game_mode::GameMode;
use oxidized_chat::Component;
use oxidized_commands::CommandError;
use oxidized_commands::arguments::ArgumentType;
use oxidized_commands::context::CommandContext;
use oxidized_commands::dispatcher::CommandDispatcher;
use oxidized_commands::nodes::{argument, literal};

/// Registers the `/gamemode` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("gamemode")
            .description("Sets a player's game mode")
            .requires_op()
            .then(
                argument("gamemode", ArgumentType::Gamemode)
                    // /gamemode <mode> — apply to self
                    .executes(|ctx: &CommandContext<CommandSourceStack>| {
                        let gm = get_gamemode(ctx, "gamemode")?;
                        let mode = GameMode::from_id(gm.id());
                        let uuid = require_player_uuid(ctx)?;
                        set_game_mode(ctx, uuid, &ctx.source.display_name.clone(), mode, true)
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
                                let mode = GameMode::from_id(gm.id());
                                let targets = get_entities(ctx, "target")?;
                                if targets.is_empty() {
                                    return Err(CommandError::Parse(
                                        "No players found".to_string(),
                                    ));
                                }
                                let source_uuid = match &ctx.source.source {
                                    CommandSourceKind::Player { uuid, .. } => Some(*uuid),
                                    CommandSourceKind::Console => None,
                                };
                                let mut count = 0i32;
                                for target in &targets {
                                    let is_self = source_uuid.is_some_and(|u| u == target.uuid);
                                    if set_game_mode(ctx, target.uuid, &target.name, mode, is_self)
                                        .is_ok()
                                    {
                                        count += 1;
                                    }
                                }
                                Ok(count)
                            },
                        ),
                    ),
            ),
    );
}

/// Extracts the player UUID from the command source, or returns an error
/// if the source is not a player.
fn require_player_uuid(
    ctx: &CommandContext<CommandSourceStack>,
) -> Result<uuid::Uuid, CommandError> {
    match &ctx.source.source {
        CommandSourceKind::Player { uuid, .. } => Ok(*uuid),
        CommandSourceKind::Console => {
            ctx.source.send_failure(&Component::text(
                "This command can only be used by a player",
            ));
            Err(CommandError::Parse(
                "This command can only be used by a player".to_string(),
            ))
        },
    }
}

/// Sets a single player's game mode and sends appropriate feedback.
///
/// Returns `Ok(1)` if the mode changed, or `Err` if it didn't (already
/// in that mode or player not found).
fn set_game_mode(
    ctx: &CommandContext<CommandSourceStack>,
    target_uuid: uuid::Uuid,
    target_name: &str,
    mode: GameMode,
    is_self: bool,
) -> Result<i32, CommandError> {
    if !ctx.source.server.set_player_game_mode(target_uuid, mode) {
        // Already in that mode (or player not found) — no-op per vanilla.
        return Err(CommandError::Parse("Game mode unchanged".to_string()));
    }

    let mode_component = Component::translatable(mode_translation_key(mode), vec![]);

    if is_self {
        ctx.source.send_translatable_success(
            "commands.gamemode.success.self",
            vec![mode_component],
            true,
        );
    } else {
        // Notify the target player that their game mode was changed.
        ctx.source.server.send_system_message_to_player(
            target_uuid,
            &Component::translatable("gameMode.changed", vec![mode_component.clone()]),
        );
        ctx.source.send_translatable_success(
            "commands.gamemode.success.other",
            vec![Component::text(target_name), mode_component],
            true,
        );
    }

    Ok(1)
}

/// Returns the vanilla translation key for a game mode.
fn mode_translation_key(mode: GameMode) -> &'static str {
    match mode {
        GameMode::Survival => "gameMode.survival",
        GameMode::Creative => "gameMode.creative",
        GameMode::Adventure => "gameMode.adventure",
        GameMode::Spectator => "gameMode.spectator",
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_translation_keys() {
        assert_eq!(
            mode_translation_key(GameMode::Survival),
            "gameMode.survival"
        );
        assert_eq!(
            mode_translation_key(GameMode::Creative),
            "gameMode.creative"
        );
        assert_eq!(
            mode_translation_key(GameMode::Adventure),
            "gameMode.adventure"
        );
        assert_eq!(
            mode_translation_key(GameMode::Spectator),
            "gameMode.spectator"
        );
    }
}
