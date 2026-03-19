//! `/gamerule` command — query or set game rules.

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
            .requires(|s: &CommandSourceStack| s.has_permission(2))
            // /gamerule <rule>
            .then(
                argument("rule", ArgumentType::String(StringKind::SingleWord))
                    .executes(|ctx: &CommandContext<CommandSourceStack>| {
                        let rule = get_string(ctx, "rule")?;
                        ctx.source.send_success(
                            &Component::text(format!("Game rule {rule} is currently set")),
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
                                ctx.source.send_success(
                                    &Component::text(format!(
                                        "Game rule {rule} has been set to {value}"
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
