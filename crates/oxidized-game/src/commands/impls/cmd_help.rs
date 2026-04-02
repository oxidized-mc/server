//! `/help` command — dynamically list all registered commands with
//! descriptions, pagination, and interactive chat elements.
//!
//! Each command entry is clickable (suggests the command) and hoverable
//! (shows the description). Pages are navigated via clickable prev/next.

use crate::commands::pagination::PaginatedMessage;
use crate::commands::source::CommandSourceStack;
use oxidized_chat::ChatFormatting;
use oxidized_chat::Component;
use oxidized_chat::{ClickEvent, HoverEvent, TextColor};
use oxidized_commands::argument_access::get_integer;
use oxidized_commands::arguments::ArgumentType;
use oxidized_commands::context::CommandContext;
use oxidized_commands::dispatcher::CommandDispatcher;
use oxidized_commands::nodes::{argument, literal};

/// Registers the `/help` command.
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(
        literal("help")
            .description("Shows the help menu")
            // /help
            .executes(|ctx: &CommandContext<CommandSourceStack>| show_help(ctx, 1))
            // /help <page>
            .then(
                argument(
                    "page",
                    ArgumentType::Integer {
                        min: Some(1),
                        max: None,
                    },
                )
                .executes(|ctx: &CommandContext<CommandSourceStack>| {
                    let page = get_integer(ctx, "page").unwrap_or(1);
                    show_help(ctx, page.max(1) as usize)
                }),
            ),
    );
}

fn show_help(
    ctx: &CommandContext<CommandSourceStack>,
    page: usize,
) -> Result<i32, oxidized_commands::CommandError> {
    let descs = ctx.source.server.command_descriptions();
    if descs.is_empty() {
        ctx.source
            .send_translatable_failure("commands.help.failed", vec![]);
        return Ok(0);
    }

    let mut paginated = PaginatedMessage::new("Help", "/help");
    paginated = paginated.per_page(7);

    for (name, desc) in &descs {
        let desc_text = desc.as_deref().unwrap_or("No description available");

        // Clickable command name that suggests the command, with hover description
        let entry = Component::text(format!("/{name}"))
            .color(TextColor::Named(ChatFormatting::Gold))
            .click(ClickEvent::SuggestCommand(format!("/{name} ")))
            .hover(HoverEvent::ShowText(Box::new(
                Component::text(desc_text).color(TextColor::Named(ChatFormatting::Yellow)),
            )))
            .append(
                Component::text(format!(" - {desc_text}"))
                    .color(TextColor::Named(ChatFormatting::White)),
            );

        paginated.add_line(entry);
    }

    let total_pages = paginated.page_count();
    if page > total_pages {
        ctx.source
            .send_translatable_failure("commands.help.failed", vec![]);
        return Ok(0);
    }

    let lines = paginated.render_page(page);
    for line in lines {
        ctx.source.send_message(&line);
    }

    Ok(descs.len() as i32)
}
