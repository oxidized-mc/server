//! Reusable chat pagination for long command output.

use oxidized_protocol::chat::ChatFormatting;
use oxidized_protocol::chat::Component;
use oxidized_protocol::chat::{ClickEvent, HoverEvent, TextColor};

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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn make_msg(n: usize, per_page: usize) -> PaginatedMessage {
        let mut msg = PaginatedMessage::new("Test", "/test");
        msg = msg.per_page(per_page);
        for i in 0..n {
            msg.add_line(Component::text(format!("Line {i}")));
        }
        msg
    }

    #[test]
    fn test_pagination_single_page() {
        let msg = make_msg(3, 7);
        assert_eq!(msg.page_count(), 1);
        let lines = msg.render_page(1);
        // Header + 3 lines, no footer (single page)
        assert_eq!(lines.len(), 4);
    }

    #[test]
    fn test_pagination_multi_page_count() {
        let msg = make_msg(15, 7);
        assert_eq!(msg.page_count(), 3);
    }

    #[test]
    fn test_pagination_page_1_has_next_only() {
        let msg = make_msg(15, 7);
        let lines = msg.render_page(1);
        // Header + 7 lines + footer
        assert_eq!(lines.len(), 9);
        // Footer should contain "Next" but not "Previous"
        let footer = &lines[8];
        let footer_text = format!("{footer:?}");
        assert!(footer_text.contains("Next"), "footer should have Next");
    }

    #[test]
    fn test_pagination_last_page_has_prev_only() {
        let msg = make_msg(15, 7);
        let lines = msg.render_page(3);
        // Header + 1 line (15 - 14) + footer
        assert_eq!(lines.len(), 3);
        let footer = &lines[2];
        let footer_text = format!("{footer:?}");
        assert!(
            footer_text.contains("Previous"),
            "footer should have Previous"
        );
    }

    #[test]
    fn test_pagination_middle_page_has_both_nav() {
        let msg = make_msg(15, 7);
        let lines = msg.render_page(2);
        // Header + 7 lines + footer
        assert_eq!(lines.len(), 9);
        let footer = &lines[8];
        let footer_text = format!("{footer:?}");
        assert!(
            footer_text.contains("Previous"),
            "footer should have Previous"
        );
        assert!(footer_text.contains("Next"), "footer should have Next");
    }

    #[test]
    fn test_pagination_empty_has_one_page() {
        let msg = make_msg(0, 7);
        assert_eq!(msg.page_count(), 1);
        let lines = msg.render_page(1);
        // Header only
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn test_pagination_clamps_out_of_range_page() {
        let msg = make_msg(5, 7);
        // Page 99 should clamp to page 1 (only 1 page)
        let lines = msg.render_page(99);
        assert_eq!(lines.len(), 6); // header + 5 lines
    }

    #[test]
    fn test_pagination_per_page_min_one() {
        let msg = make_msg(3, 0); // 0 should clamp to 1
        assert_eq!(msg.page_count(), 3);
    }
}
