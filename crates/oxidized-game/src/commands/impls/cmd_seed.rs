//! `/seed` command — show the world seed.

use crate::commands::nodes::LiteralBuilderExt;
use crate::commands::source::CommandSourceStack;
use oxidized_chat::ChatFormatting;
use oxidized_chat::Component;
use oxidized_chat::{ClickEvent, HoverEvent, TextColor};
use oxidized_commands::context::CommandContext;
use oxidized_commands::dispatcher::CommandDispatcher;
use oxidized_commands::nodes::literal;

/// Registers the `/seed` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("seed")
            .description("Displays the world seed")
            .requires_op()
            .executes(|ctx: &CommandContext<CommandSourceStack>| {
                let seed = ctx.source.server.seed();
                // Build a clickable seed component like vanilla does
                let seed_text = Component::text(format!("[{seed}]"))
                    .color(TextColor::Named(ChatFormatting::Green))
                    .click(ClickEvent::CopyToClipboard(seed.to_string()))
                    .hover(HoverEvent::ShowText(Box::new(Component::translatable(
                        "chat.copy.click",
                        vec![],
                    ))));
                ctx.source.send_translatable_success(
                    "commands.seed.success",
                    vec![seed_text],
                    false,
                );
                Ok(seed as i32)
            }),
    );
}
