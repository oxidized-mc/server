//! Command dispatcher: parse input against the graph, execute, and suggest.

use crate::commands::CommandError;
use crate::commands::context::{
    CommandContext, ParseResults, ParsedArgument, StringRange, StringReader, Suggestion,
    parse_argument,
};
use crate::commands::nodes::{CommandNode, LiteralBuilder, RootCommandNode};
use crate::commands::serializer::{CommandTreeData, serialize_tree};
use std::collections::HashMap;

/// The top-level command dispatcher holding the full command graph.
pub struct CommandDispatcher<S> {
    /// The root node of the command graph.
    pub root: RootCommandNode<S>,
}

impl<S: Clone + Send + Sync + 'static> CommandDispatcher<S> {
    /// Creates a new empty dispatcher.
    pub fn new() -> Self {
        Self {
            root: RootCommandNode::new(),
        }
    }

    /// Registers a top-level command.
    pub fn register(&mut self, builder: LiteralBuilder<S>) {
        let node = builder.build();
        self.root.add_child(node);
    }

    /// Parses input against the command graph, returning a ready-to-execute
    /// context with parsed arguments.
    pub fn parse(&self, input: &str, source: S) -> Result<ParseResults<S>, CommandError> {
        let mut reader = StringReader::new(input, 0);

        // Read the first word as the command name.
        let cmd_name = reader.read_word();
        if cmd_name.is_empty() {
            return Err(CommandError::Parse("Expected command name".to_string()));
        }

        let node = self.root.children.get(cmd_name).ok_or_else(|| {
            CommandError::Parse(format!(
                "Unknown or incomplete command, see below for error\n{cmd_name}<--[HERE]"
            ))
        })?;

        // Check requirement
        if !node.can_use(&source) {
            return Err(CommandError::Parse(format!(
                "Unknown or incomplete command, see below for error\n{cmd_name}<--[HERE]"
            )));
        }

        // Now walk deeper into the tree, parsing arguments.
        let mut arguments = HashMap::new();
        let mut current = node;
        let mut command = node.command().cloned();

        loop {
            reader.skip_whitespace();
            if !reader.can_read() {
                break;
            }

            // Try to match a child node.
            let remaining = reader.remaining().to_string();
            let mut matched = false;

            // Try literal children first, then argument children.
            for child in current.children().values() {
                match child {
                    CommandNode::Literal(lit) => {
                        if remaining.starts_with(&lit.literal)
                            && (remaining.len() == lit.literal.len()
                                || remaining.as_bytes().get(lit.literal.len()) == Some(&b' '))
                        {
                            if !child.can_use(&source) {
                                continue;
                            }
                            reader = StringReader::new(input, reader.cursor() + lit.literal.len());
                            current = child;
                            if let Some(cmd) = child.command() {
                                command = Some(cmd.clone());
                            }
                            matched = true;
                            break;
                        }
                    },
                    CommandNode::Argument(arg) => {
                        if !child.can_use(&source) {
                            continue;
                        }
                        let start = reader.cursor();
                        match parse_argument(&mut reader, &arg.argument_type) {
                            Ok(result) => {
                                let end = reader.cursor();
                                arguments.insert(
                                    arg.name.clone(),
                                    ParsedArgument {
                                        range: StringRange::new(start, end),
                                        result,
                                    },
                                );
                                current = child;
                                if let Some(cmd) = child.command() {
                                    command = Some(cmd.clone());
                                }
                                matched = true;
                                break;
                            },
                            Err(e) => {
                                // Reset cursor and try next child.
                                reader = StringReader::new(input, start);
                                // If this is the only child, propagate the error.
                                if current.children().len() == 1 {
                                    return Err(e);
                                }
                            },
                        }
                    },
                    CommandNode::Root(_) => {},
                }
            }

            if !matched {
                // If there's remaining input but no child matched, error.
                let pos = reader.cursor();
                return Err(CommandError::Parse(format!(
                    "Incorrect argument for command at position {pos}"
                )));
            }
        }

        Ok(ParseResults {
            context: CommandContext {
                source,
                input: input.to_string(),
                arguments,
                command,
            },
            cursor: reader.cursor(),
        })
    }

    /// Executes a previously-parsed command.
    pub fn execute(&self, parse: &ParseResults<S>) -> Result<i32, CommandError> {
        let cmd = parse
            .context
            .command
            .as_ref()
            .ok_or_else(|| CommandError::Parse("Incomplete command".to_string()))?;
        (cmd)(&parse.context)
    }

    /// Collects tab-completion suggestions for the given input.
    pub fn get_completions(&self, input: &str, source: &S) -> Vec<Suggestion> {
        let parts: Vec<&str> = input.splitn(2, ' ').collect();
        let partial_cmd = parts[0];

        // If we're still on the first word, suggest command names.
        if parts.len() == 1 && !input.ends_with(' ') {
            return self
                .root
                .children
                .keys()
                .filter(|name| name.starts_with(partial_cmd))
                .filter(|name| {
                    self.root
                        .children
                        .get(name.as_str())
                        .is_some_and(|n| n.can_use(source))
                })
                .map(|name| Suggestion {
                    range: StringRange::new(0, partial_cmd.len()),
                    text: name.clone(),
                    tooltip: None,
                })
                .collect();
        }

        // We're past the first word — walk the tree to find the current node,
        // then suggest its children.
        if let Some(node) = self.root.children.get(partial_cmd) {
            if !node.can_use(source) {
                return Vec::new();
            }
            let remaining = if parts.len() > 1 { parts[1] } else { "" };
            return collect_child_suggestions(node, remaining, source);
        }

        Vec::new()
    }

    /// Serializes the command tree, filtered by the given source's
    /// permissions.
    pub fn serialize_tree(&self, source: &S) -> CommandTreeData {
        serialize_tree(&self.root, source)
    }
}

impl<S: Clone + Send + Sync + 'static> Default for CommandDispatcher<S> {
    fn default() -> Self {
        Self::new()
    }
}

/// Recursively collects suggestions from child nodes.
fn collect_child_suggestions<S>(
    node: &CommandNode<S>,
    remaining: &str,
    source: &S,
) -> Vec<Suggestion> {
    let parts: Vec<&str> = remaining.splitn(2, ' ').collect();
    let current_word = parts[0];

    // If there's more input after a space, try to walk deeper.
    if parts.len() > 1 {
        // Try to match the current word to a child.
        for child in node.children().values() {
            match child {
                CommandNode::Literal(lit) if lit.literal == current_word => {
                    if !child.can_use(source) {
                        continue;
                    }
                    return collect_child_suggestions(child, parts[1], source);
                },
                CommandNode::Argument(_) => {
                    if !child.can_use(source) {
                        continue;
                    }
                    return collect_child_suggestions(child, parts[1], source);
                },
                _ => {},
            }
        }
        return Vec::new();
    }

    // We're at the last word — suggest matching children.
    let mut suggestions = Vec::new();
    for child in node.children().values() {
        if !child.can_use(source) {
            continue;
        }
        match child {
            CommandNode::Literal(lit) => {
                if lit.literal.starts_with(current_word) {
                    suggestions.push(Suggestion {
                        range: StringRange::new(0, current_word.len()),
                        text: lit.literal.clone(),
                        tooltip: None,
                    });
                }
            },
            CommandNode::Argument(arg) => {
                // For argument nodes, suggest the argument name as a hint.
                suggestions.push(Suggestion {
                    range: StringRange::new(0, current_word.len()),
                    text: format!("<{}>", arg.name),
                    tooltip: None,
                });
            },
            _ => {},
        }
    }
    suggestions
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::commands::arguments::ArgumentType;
    use crate::commands::context::get_integer;
    use crate::commands::nodes::{argument, literal};
    use crate::commands::source::{CommandSourceKind, CommandSourceStack, ServerHandle};
    use oxidized_protocol::chat::Component;
    use std::sync::Arc;

    /// Minimal mock server handle for tests.
    struct MockServer;

    impl ServerHandle for MockServer {
        fn broadcast_to_ops(&self, _msg: &Component, _min_level: u32) {}
        fn request_shutdown(&self) {}
        fn seed(&self) -> i64 {
            42
        }
        fn online_player_names(&self) -> Vec<String> {
            vec!["Alice".to_string()]
        }
        fn online_player_count(&self) -> usize {
            1
        }
        fn max_players(&self) -> usize {
            20
        }
    }

    fn make_source(permission_level: u32) -> CommandSourceStack {
        CommandSourceStack {
            source: CommandSourceKind::Console,
            position: (0.0, 64.0, 0.0),
            rotation: (0.0, 0.0),
            permission_level,
            display_name: "Console".to_string(),
            server: Arc::new(MockServer),
            feedback_sender: Arc::new(|_| {}),
            silent: false,
        }
    }

    // ── Dispatcher: parse & execute ─────────────────────────────────────

    #[test]
    fn dispatcher_executes_literal_command() {
        let mut d = CommandDispatcher::new();
        d.register(literal("ping").executes(|_| Ok(42)));
        let src = make_source(4);
        let parse = d.parse("ping", src).unwrap();
        assert_eq!(d.execute(&parse).unwrap(), 42);
    }

    #[test]
    fn dispatcher_returns_error_for_unknown_command() {
        let d = CommandDispatcher::<CommandSourceStack>::new();
        let src = make_source(4);
        assert!(d.parse("unknowncommand", src).is_err());
    }

    #[test]
    fn dispatcher_parses_integer_argument() {
        let mut d = CommandDispatcher::new();
        d.register(
            literal("test").then(
                argument(
                    "n",
                    ArgumentType::Integer {
                        min: Some(0),
                        max: None,
                    },
                )
                .executes(|ctx| {
                    let n = get_integer(ctx, "n")?;
                    Ok(n)
                }),
            ),
        );
        let src = make_source(4);
        let parse = d.parse("test 7", src).unwrap();
        assert_eq!(d.execute(&parse).unwrap(), 7);
    }

    #[test]
    fn dispatcher_integer_argument_rejects_out_of_range() {
        let mut d = CommandDispatcher::new();
        d.register(
            literal("setval").then(
                argument(
                    "n",
                    ArgumentType::Integer {
                        min: Some(1),
                        max: Some(10),
                    },
                )
                .executes(|_| Ok(1)),
            ),
        );
        let src = make_source(4);
        assert!(d.parse("setval 99", src).is_err());
    }

    #[test]
    fn permission_requirement_blocks_low_permission_source() {
        let mut d = CommandDispatcher::new();
        d.register(
            literal("stop")
                .requires(|s: &CommandSourceStack| s.has_permission(4))
                .executes(|_| Ok(1)),
        );
        let src = make_source(0); // no permission
        assert!(d.parse("stop", src).is_err());
    }

    #[test]
    fn permission_requirement_allows_high_permission_source() {
        let mut d = CommandDispatcher::new();
        d.register(
            literal("stop")
                .requires(|s: &CommandSourceStack| s.has_permission(4))
                .executes(|_| Ok(1)),
        );
        let src = make_source(4);
        let parse = d.parse("stop", src).unwrap();
        assert_eq!(d.execute(&parse).unwrap(), 1);
    }

    #[test]
    fn dispatcher_handles_nested_literals() {
        let mut d = CommandDispatcher::new();
        d.register(
            literal("time").then(
                literal("set").then(
                    argument(
                        "value",
                        ArgumentType::Integer {
                            min: None,
                            max: None,
                        },
                    )
                    .executes(|ctx| get_integer(ctx, "value")),
                ),
            ),
        );
        let src = make_source(4);
        let parse = d.parse("time set 1000", src).unwrap();
        assert_eq!(d.execute(&parse).unwrap(), 1000);
    }

    // ── Serialization ───────────────────────────────────────────────────

    #[test]
    fn serialize_tree_root_node_has_zero_flags() {
        let d = CommandDispatcher::<CommandSourceStack>::new();
        let src = make_source(4);
        let tree = d.serialize_tree(&src);
        assert_eq!(tree.nodes[0].flags & 0b11, 0b00);
    }

    #[test]
    fn serialize_tree_literal_node_has_correct_flags() {
        let mut d = CommandDispatcher::new();
        d.register(literal("help").executes(|_| Ok(1)));
        let src = make_source(4);
        let tree = d.serialize_tree(&src);
        let help_node = tree
            .nodes
            .iter()
            .find(|n| n.name.as_deref() == Some("help"))
            .unwrap();
        assert_eq!(help_node.flags & 0b11, 0b01, "should be literal type");
        assert!(help_node.flags & 0b100 != 0, "should be executable");
    }

    #[test]
    fn serialize_tree_argument_node_has_correct_flags() {
        let mut d = CommandDispatcher::new();
        d.register(
            literal("test").then(
                argument(
                    "n",
                    ArgumentType::Integer {
                        min: None,
                        max: None,
                    },
                )
                .executes(|_| Ok(1)),
            ),
        );
        let src = make_source(4);
        let tree = d.serialize_tree(&src);
        let arg_node = tree
            .nodes
            .iter()
            .find(|n| n.name.as_deref() == Some("n"))
            .unwrap();
        assert_eq!(arg_node.flags & 0b11, 0b10, "should be argument type");
        assert!(arg_node.parser.is_some(), "should have parser info");
    }

    #[test]
    fn serialize_tree_filters_by_permission() {
        let mut d = CommandDispatcher::new();
        d.register(literal("help").executes(|_| Ok(1)));
        d.register(
            literal("stop")
                .requires(|s: &CommandSourceStack| s.has_permission(4))
                .executes(|_| Ok(1)),
        );
        let src = make_source(0); // low permission
        let tree = d.serialize_tree(&src);
        // "help" should be present, "stop" should not
        assert!(tree.nodes.iter().any(|n| n.name.as_deref() == Some("help")));
        assert!(!tree.nodes.iter().any(|n| n.name.as_deref() == Some("stop")));
    }

    // ── Completions ─────────────────────────────────────────────────────

    #[test]
    fn completions_returns_registered_command_names_at_root() {
        let mut d = CommandDispatcher::new();
        d.register(literal("help").executes(|_| Ok(1)));
        d.register(literal("stop").executes(|_| Ok(1)));
        let src = make_source(4);
        let completions = d.get_completions("", &src);
        let texts: Vec<_> = completions.iter().map(|s| s.text.as_str()).collect();
        assert!(texts.contains(&"help"));
        assert!(texts.contains(&"stop"));
    }

    #[test]
    fn completions_filters_by_prefix() {
        let mut d = CommandDispatcher::new();
        d.register(literal("give").executes(|_| Ok(1)));
        d.register(literal("gamemode").executes(|_| Ok(1)));
        d.register(literal("kill").executes(|_| Ok(1)));
        let src = make_source(4);
        let completions = d.get_completions("g", &src);
        let texts: Vec<_> = completions.iter().map(|s| s.text.as_str()).collect();
        assert!(texts.contains(&"give"), "should include give");
        assert!(texts.contains(&"gamemode"), "should include gamemode");
        assert!(!texts.contains(&"kill"), "should not include kill");
    }

    #[test]
    fn completions_respects_permissions() {
        let mut d = CommandDispatcher::new();
        d.register(literal("help").executes(|_| Ok(1)));
        d.register(
            literal("stop")
                .requires(|s: &CommandSourceStack| s.has_permission(4))
                .executes(|_| Ok(1)),
        );
        let src = make_source(0);
        let completions = d.get_completions("", &src);
        let texts: Vec<_> = completions.iter().map(|s| s.text.as_str()).collect();
        assert!(texts.contains(&"help"), "should include help");
        assert!(!texts.contains(&"stop"), "should not include stop");
    }
}
