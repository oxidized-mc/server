//! Reusable chat pagination for long command output.

use oxidized_protocol::chat::Component;
use oxidized_protocol::chat::style::{ClickEvent, HoverEvent, TextColor};
use oxidized_protocol::chat::ChatFormatting;

/// Configuration for a paginated message.
pub struct PaginatedMessage {
    /// Lines to display across pages.
    lines: Vec<Component>,
    /// Number of lines per page.
    per_page: usize,
    /// The command prefix for page navigation (e.g., "/help").
    command_prefix: String,
    /// Title shown in the header.
    title: String,
}

impl PaginatedMessage {
    /// Creates a new paginated message.
    pub fn new(title: impl Into<String>, command_prefix: impl Into<String>) -> Self {
        Self {
            lines: Vec::new(),
            per_page: 7,
            command_prefix: command_prefix.into(),
            title: title.into(),
        }
    }

    /// Sets the number of lines per page.
    pub fn per_page(mut self, n: usize) -> Self {
        self.per_page = n.max(1);
        self
    }

    /// Adds a line to the message.
    pub fn add_line(&mut self, line: Component) {
        self.lines.push(line);
    }

    /// Returns the total number of pages.
    pub fn page_count(&self) -> usize {
        if self.lines.is_empty() {
            1
        } else {
            self.lines.len().div_ceil(self.per_page)
        }
    }

    /// Renders a specific page (1-indexed) as a list of [`Component`]s to send.
    pub fn render_page(&self, page: usize) -> Vec<Component> {
        let total_pages = self.page_count();
        let page = page.clamp(1, total_pages);
        let mut output = Vec::new();

        // Header
        output.push(
            Component::text(format!(
                "--- {} (page {} of {}) ---",
                self.title, page, total_pages
            ))
            .color(TextColor::Named(ChatFormatting::Yellow)),
        );

        // Page content
        let start = (page - 1) * self.per_page;
        let end = (start + self.per_page).min(self.lines.len());
        for line in &self.lines[start..end] {
            output.push(line.clone());
        }

        // Footer with navigation
        if total_pages > 1 {
            let mut footer = Component::empty();

            if page > 1 {
                let prev_cmd = format!("{} {}", self.command_prefix, page - 1);
                footer = footer.append(
                    Component::text("«« Previous")
                        .color(TextColor::Named(ChatFormatting::Gold))
                        .click(ClickEvent::RunCommand(prev_cmd))
                        .hover(HoverEvent::ShowText(Box::new(Component::text(format!(
                            "Go to page {}",
                            page - 1
                        ))))),
                );
            }

            if page > 1 && page < total_pages {
                footer = footer.append(Component::text("  "));
            }

            if page < total_pages {
                let next_cmd = format!("{} {}", self.command_prefix, page + 1);
                footer = footer.append(
                    Component::text("Next »»")
                        .color(TextColor::Named(ChatFormatting::Gold))
                        .click(ClickEvent::RunCommand(next_cmd))
                        .hover(HoverEvent::ShowText(Box::new(Component::text(format!(
                            "Go to page {}",
                            page + 1
                        ))))),
                );
            }

            output.push(footer);
        }

        output
    }
}
