//! `/gamerule` command — query or set game rules.
//!
//! Uses `ServerHandle::get_game_rule` / `set_game_rule` to access the
//! game rules storage.

use crate::commands::nodes::LiteralBuilderExt;
use crate::commands::source::CommandSourceStack;
use oxidized_chat::Component;
use oxidized_commands::argument_access::get_string;
use oxidized_commands::arguments::{ArgumentType, StringKind};
use oxidized_commands::context::CommandContext;
use oxidized_commands::dispatcher::CommandDispatcher;
use oxidized_commands::nodes::{argument, literal};

/// Registers the `/gamerule` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("gamerule")
            .description("Sets or queries a game rule value")
            .requires_op()
            // /gamerule <rule>
            .then(
                argument("rule", ArgumentType::String(StringKind::SingleWord))
                    .executes(|ctx: &CommandContext<CommandSourceStack>| {
                        let rule = get_string(ctx, "rule")?;
                        match ctx.source.server.get_game_rule(rule) {
                            Some(value) => {
                                ctx.source.send_translatable_success(
                                    "commands.gamerule.query",
                                    vec![Component::text(rule), Component::text(value)],
                                    false,
                                );
                                Ok(1)
                            },
                            None => {
                                ctx.source.send_failure(&Component::text(format!(
                                    "Unknown game rule: {rule}"
                                )));
                                Ok(0)
                            },
                        }
                    })
                    // /gamerule <rule> <value>
                    .then(
                        argument("value", ArgumentType::String(StringKind::SingleWord)).executes(
                            |ctx: &CommandContext<CommandSourceStack>| {
                                let rule = get_string(ctx, "rule")?;
                                let value = get_string(ctx, "value")?;
                                match ctx.source.server.set_game_rule(rule, value) {
                                    Ok(()) => {
                                        ctx.source.send_translatable_success(
                                            "commands.gamerule.set",
                                            vec![Component::text(rule), Component::text(value)],
                                            true,
                                        );
                                        Ok(1)
                                    },
                                    Err(msg) => {
                                        ctx.source.send_failure(&Component::text(msg));
                                        Ok(0)
                                    },
                                }
                            },
                        ),
                    ),
            ),
    );
}
