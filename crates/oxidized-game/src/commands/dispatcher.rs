//! Command dispatcher: parse input against the graph, execute, and suggest.

use crate::commands::CommandError;
use crate::commands::argument_parser::parse_argument;
use crate::commands::arguments::ArgumentType;
use crate::commands::context::{
    CommandContext, ParseResults, ParsedArgument, StringRange, Suggestion,
};
use crate::commands::nodes::{CommandNode, LiteralBuilder, RootCommandNode};
use crate::commands::serializer::{CommandTreeData, serialize_tree};
use crate::commands::string_reader::StringReader;
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
    ///
    /// # Errors
    ///
    /// Returns [`CommandError::Parse`] if the command name is unknown, the
    /// source lacks permission, or any argument fails to parse.
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

            match try_match_child(current, &source, input, &mut reader, &mut arguments) {
                ChildMatch::Matched { node: next, cmd } => {
                    current = next;
                    if let Some(c) = cmd {
                        command = Some(c);
                    }
                },
                ChildMatch::NoMatch => {
                    let pos = reader.cursor();
                    return Err(CommandError::Parse(format!(
                        "Incorrect argument for command at position {pos}"
                    )));
                },
                ChildMatch::Error(e) => return Err(e),
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
    ///
    /// # Errors
    ///
    /// Returns [`CommandError::Parse`] if the parse result has no resolved
    /// command. May also propagate errors from the command handler itself.
    pub fn execute(&self, parse: &ParseResults<S>) -> Result<i32, CommandError> {
        let cmd = parse
            .context
            .command
            .as_ref()
            .ok_or_else(|| CommandError::Parse("Incomplete command".to_string()))?;
        (cmd)(&parse.context)
    }

    /// Collects tab-completion suggestions for the given input.
    ///
    /// The returned [`Suggestion`]s have their `range` set relative to
    /// the full `input` string, so callers can map them directly to the
    /// protocol's `start`/`length` fields.
    pub fn get_completions(
        &self,
        input: &str,
        source: &S,
        player_names: &[String],
    ) -> Vec<Suggestion> {
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
            // Offset = length of command name + 1 (for the space separator)
            let offset = partial_cmd.len() + 1;
            return collect_child_suggestions(node, remaining, offset, source, player_names);
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

/// Result of attempting to match input against child nodes.
enum ChildMatch<'a, S> {
    /// A child node matched.
    Matched {
        /// The matched child node.
        node: &'a CommandNode<S>,
        /// The command handler, if the matched node is executable.
        cmd: Option<
            std::sync::Arc<dyn Fn(&CommandContext<S>) -> Result<i32, CommandError> + Send + Sync>,
        >,
    },
    /// No child matched the input.
    NoMatch,
    /// A parse error occurred (single-child propagation).
    Error(CommandError),
}

/// Tries to match the remaining input against children of `current`.
///
/// On success, advances `reader` past the matched token and inserts
/// any parsed argument into `arguments`.
fn try_match_child<'a, S: Clone + Send + Sync + 'static>(
    current: &'a CommandNode<S>,
    source: &S,
    input: &'a str,
    reader: &mut StringReader<'a>,
    arguments: &mut HashMap<String, ParsedArgument>,
) -> ChildMatch<'a, S> {
    let remaining = reader.remaining().to_string();

    for child in current.children().values() {
        match child {
            CommandNode::Literal(lit) => {
                let is_match = remaining.starts_with(&lit.literal)
                    && (remaining.len() == lit.literal.len()
                        || remaining.as_bytes().get(lit.literal.len()) == Some(&b' '));
                if !is_match || !child.can_use(source) {
                    continue;
                }
                *reader = StringReader::new(input, reader.cursor() + lit.literal.len());
                return ChildMatch::Matched {
                    node: child,
                    cmd: child.command().cloned(),
                };
            },
            CommandNode::Argument(arg) => {
                if !child.can_use(source) {
                    continue;
                }
                let start = reader.cursor();
                let result = match parse_argument(reader, &arg.argument_type) {
                    Ok(r) => r,
                    Err(e) if current.children().len() == 1 => {
                        *reader = StringReader::new(input, start);
                        return ChildMatch::Error(e);
                    },
                    Err(_) => {
                        *reader = StringReader::new(input, start);
                        continue;
                    },
                };
                let range = StringRange::new(start, reader.cursor());
                arguments.insert(arg.name.clone(), ParsedArgument { range, result });
                return ChildMatch::Matched {
                    node: child,
                    cmd: child.command().cloned(),
                };
            },
            CommandNode::Root(_) => {},
        }
    }

    ChildMatch::NoMatch
}

/// Recursively collects suggestions from child nodes.
///
/// `offset` is the character position in the original input where
/// `remaining` starts. This lets us build correct [`StringRange`]s.
fn collect_child_suggestions<S>(
    node: &CommandNode<S>,
    remaining: &str,
    offset: usize,
    source: &S,
    player_names: &[String],
) -> Vec<Suggestion> {
    let parts: Vec<&str> = remaining.splitn(2, ' ').collect();
    let current_word = parts[0];

    // If there's more input after a space, try to walk deeper.
    if parts.len() > 1 {
        let next_offset = offset + current_word.len() + 1;
        // Try to match the current word to a child.
        for child in node.children().values() {
            match child {
                CommandNode::Literal(lit) if lit.literal == current_word => {
                    if !child.can_use(source) {
                        continue;
                    }
                    return collect_child_suggestions(
                        child,
                        parts[1],
                        next_offset,
                        source,
                        player_names,
                    );
                },
                CommandNode::Argument(_) => {
                    if !child.can_use(source) {
                        continue;
                    }
                    return collect_child_suggestions(
                        child,
                        parts[1],
                        next_offset,
                        source,
                        player_names,
                    );
                },
                _ => {},
            }
        }
        return Vec::new();
    }

    // We're at the last word — suggest matching children.
    let range = StringRange::new(offset, offset + current_word.len());
    let mut suggestions = Vec::new();
    for child in node.children().values() {
        if !child.can_use(source) {
            continue;
        }
        match child {
            CommandNode::Literal(lit) => {
                if lit.literal.starts_with(current_word) {
                    suggestions.push(Suggestion {
                        range,
                        text: lit.literal.clone(),
                        tooltip: None,
                    });
                }
            },
            CommandNode::Argument(arg) => {
                suggest_for_argument(
                    &arg.argument_type,
                    &arg.name,
                    current_word,
                    range,
                    player_names,
                    &mut suggestions,
                );
            },
            _ => {},
        }
    }
    suggestions
}

/// Builds suggestions for an argument node based on its type.
fn suggest_for_argument(
    arg_type: &ArgumentType,
    arg_name: &str,
    current_word: &str,
    range: StringRange,
    player_names: &[String],
    suggestions: &mut Vec<Suggestion>,
) {
    use crate::commands::selector::{FILTER_KEYS, SORT_VALUES};
    use crate::player::game_mode::GameMode;

    let is_entity = matches!(
        arg_type,
        ArgumentType::Entity { .. } | ArgumentType::GameProfile
    );
    if !is_entity {
        suggestions.push(Suggestion {
            range,
            text: format!("<{arg_name}>"),
            tooltip: None,
        });
        return;
    }

    // Check if we're inside a selector bracket expression like @a[...
    if let Some(bracket_start) = current_word.find('[') {
        let inside = &current_word[bracket_start + 1..];

        // Find the last filter segment (after the last comma at depth 0).
        let last_segment = split_last_segment(inside);

        if let Some((key, value_part)) = last_segment.split_once('=') {
            // We're after `key=`, suggest values for this key.
            let prefix_before =
                &current_word[..current_word.len() - value_part.len()];
            let value_range = StringRange {
                start: range.start + prefix_before.len(),
                end: range.end,
            };
            let lower = value_part.to_lowercase();

            match key.trim() {
                "sort" => {
                    for val in SORT_VALUES {
                        if val.starts_with(&lower) {
                            suggestions.push(Suggestion {
                                range: value_range,
                                text: val.to_string(),
                                tooltip: None,
                            });
                        }
                    }
                },
                "gamemode" => {
                    for val in &GameMode::ALL_NAMES {
                        if val.starts_with(&lower) {
                            suggestions.push(Suggestion {
                                range: value_range,
                                text: val.to_string(),
                                tooltip: None,
                            });
                        }
                    }
                },
                _ => {},
            }
        } else {
            // We're typing a key (no `=` yet), suggest filter keys.
            let prefix_before =
                &current_word[..current_word.len() - last_segment.len()];
            let key_range = StringRange {
                start: range.start + prefix_before.len(),
                end: range.end,
            };
            let lower = last_segment.to_lowercase();

            for key in FILTER_KEYS {
                let with_eq = format!("{key}=");
                if with_eq.starts_with(&lower) {
                    suggestions.push(Suggestion {
                        range: key_range,
                        text: with_eq,
                        tooltip: None,
                    });
                }
            }
        }
        return;
    }

    if current_word.starts_with('@') || current_word.is_empty() {
        for sel in &["@a", "@e", "@p", "@r", "@s", "@n"] {
            if sel.starts_with(current_word) {
                suggestions.push(Suggestion {
                    range,
                    text: (*sel).to_string(),
                    tooltip: None,
                });
            }
        }
    }
    let lower = current_word.to_lowercase();
    for name in player_names {
        if name.to_lowercase().starts_with(&lower) {
            suggestions.push(Suggestion {
                range,
                text: name.clone(),
                tooltip: None,
            });
        }
    }
}

/// Returns the last filter segment inside bracket syntax, respecting `{…}` nesting.
fn split_last_segment(inside: &str) -> &str {
    let mut depth = 0u32;
    let mut last_comma = None;
    for (i, ch) in inside.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => last_comma = Some(i),
            _ => {},
        }
    }
    match last_comma {
        Some(pos) => &inside[pos + 1..],
        None => inside,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::commands::argument_access::get_integer;
    use crate::commands::arguments::{ArgumentType, StringKind};
    use crate::commands::nodes::{CommandNode, argument, literal};
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
        fn difficulty(&self) -> i32 {
            2
        }
        fn game_time(&self) -> i64 {
            0
        }
        fn day_time(&self) -> i64 {
            0
        }
        fn is_raining(&self) -> bool {
            false
        }
        fn is_thundering(&self) -> bool {
            false
        }
        fn kick_player(&self, _name: &str, _reason: &str) -> bool {
            false
        }
        fn find_player_uuid(&self, _name: &str) -> Option<uuid::Uuid> {
            None
        }
        fn command_descriptions(&self) -> Vec<(String, Option<String>)> {
            vec![]
        }
    }

    fn make_source(permission_level: u32) -> CommandSourceStack {
        CommandSourceStack {
            source: CommandSourceKind::Player {
                name: "TestPlayer".to_string(),
                uuid: uuid::Uuid::nil(),
            },
            position: (0.0, 64.0, 0.0),
            rotation: (0.0, 0.0),
            permission_level,
            display_name: "TestPlayer".to_string(),
            server: Arc::new(MockServer),
            feedback_sender: Arc::new(|_| {}),
            is_silent: false,
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
    fn console_source_always_has_permission() {
        let mut d = CommandDispatcher::new();
        d.register(
            literal("stop")
                .requires(|s: &CommandSourceStack| s.has_permission(4))
                .executes(|_| Ok(1)),
        );
        let src = CommandSourceStack {
            source: CommandSourceKind::Console,
            position: (0.0, 0.0, 0.0),
            rotation: (0.0, 0.0),
            permission_level: 0, // Console should bypass regardless
            display_name: "Console".to_string(),
            server: Arc::new(MockServer),
            feedback_sender: Arc::new(|_| {}),
            is_silent: false,
        };
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
        let completions = d.get_completions("", &src, &[]);
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
        let completions = d.get_completions("g", &src, &[]);
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
        let completions = d.get_completions("", &src, &[]);
        let texts: Vec<_> = completions.iter().map(|s| s.text.as_str()).collect();
        assert!(texts.contains(&"help"), "should include help");
        assert!(!texts.contains(&"stop"), "should not include stop");
    }

    // ── Description field ───────────────────────────────────────────────

    #[test]
    fn literal_node_stores_description() {
        let mut d: CommandDispatcher<CommandSourceStack> = CommandDispatcher::new();
        d.register(
            literal("help")
                .description("Shows the help menu")
                .executes(|_| Ok(1)),
        );
        let node = CommandNode::Root(d.root);
        let desc = node.children().get("help").unwrap().description();
        assert_eq!(desc, Some("Shows the help menu"));
    }

    #[test]
    fn argument_node_stores_description() {
        let mut d: CommandDispatcher<CommandSourceStack> = CommandDispatcher::new();
        d.register(
            literal("test").then(
                argument("name", ArgumentType::String(StringKind::SingleWord))
                    .description("Player name")
                    .executes(|_| Ok(1)),
            ),
        );
        let node = CommandNode::Root(d.root);
        let test_node = node.children().get("test").unwrap();
        let name_node = test_node.children().get("name").unwrap();
        let desc = name_node.description();
        assert_eq!(desc, Some("Player name"));
    }

    #[test]
    fn node_without_description_returns_none() {
        let mut d: CommandDispatcher<CommandSourceStack> = CommandDispatcher::new();
        d.register(literal("ping").executes(|_| Ok(1)));
        let node = CommandNode::Root(d.root);
        let desc = node.children().get("ping").unwrap().description();
        assert_eq!(desc, None);
    }

    // ── Username autocomplete ───────────────────────────────────────────

    #[test]
    fn completions_suggest_player_names_for_entity_arg() {
        let mut d = CommandDispatcher::new();
        d.register(
            literal("kick").then(
                argument(
                    "target",
                    ArgumentType::Entity {
                        single: true,
                        player_only: true,
                    },
                )
                .executes(|_| Ok(1)),
            ),
        );
        let src = make_source(4);
        let names = vec!["Alice".to_string(), "Bob".to_string()];
        let completions = d.get_completions("kick ", &src, &names);
        let texts: Vec<_> = completions.iter().map(|s| s.text.as_str()).collect();
        assert!(texts.contains(&"Alice"), "should suggest Alice");
        assert!(texts.contains(&"Bob"), "should suggest Bob");
    }

    #[test]
    fn completions_filter_player_names_by_prefix() {
        let mut d = CommandDispatcher::new();
        d.register(
            literal("kick").then(
                argument(
                    "target",
                    ArgumentType::Entity {
                        single: true,
                        player_only: true,
                    },
                )
                .executes(|_| Ok(1)),
            ),
        );
        let src = make_source(4);
        let names = vec!["Alice".to_string(), "Bob".to_string()];
        let completions = d.get_completions("kick A", &src, &names);
        let texts: Vec<_> = completions.iter().map(|s| s.text.as_str()).collect();
        assert!(texts.contains(&"Alice"), "should suggest Alice");
        assert!(!texts.contains(&"Bob"), "should not suggest Bob");
    }

    // ── Suggestion range correctness ────────────────────────────────────

    #[test]
    fn suggestion_range_for_command_name() {
        let mut d = CommandDispatcher::new();
        d.register(literal("help").executes(|_| Ok(1)));
        let src = make_source(4);
        let completions = d.get_completions("he", &src, &[]);
        assert_eq!(completions.len(), 1);
        assert_eq!(completions[0].text, "help");
        // Range covers the partial input "he" at position 0
        assert_eq!(completions[0].range.start, 0);
        assert_eq!(completions[0].range.end, 2);
    }

    #[test]
    fn suggestion_range_for_first_argument() {
        let mut d = CommandDispatcher::new();
        d.register(
            literal("kick").then(
                argument(
                    "target",
                    ArgumentType::Entity {
                        single: true,
                        player_only: true,
                    },
                )
                .executes(|_| Ok(1)),
            ),
        );
        let src = make_source(4);
        let names = vec!["Alice".to_string()];
        // "kick Al" — the "Al" starts at offset 5 (after "kick ")
        let completions = d.get_completions("kick Al", &src, &names);
        assert_eq!(completions.len(), 1);
        assert_eq!(completions[0].text, "Alice");
        assert_eq!(completions[0].range.start, 5);
        assert_eq!(completions[0].range.end, 7);
    }

    #[test]
    fn suggestion_range_for_second_argument() {
        let mut d = CommandDispatcher::new();
        d.register(
            literal("give").then(
                argument(
                    "target",
                    ArgumentType::Entity {
                        single: true,
                        player_only: true,
                    },
                )
                .then(argument("item", ArgumentType::ItemStack).executes(|_| Ok(1))),
            ),
        );
        let src = make_source(4);
        // "give Alice sto" — cursor is at the "sto" argument (offset 11)
        let completions = d.get_completions("give Alice sto", &src, &[]);
        // Should get <item> placeholder
        assert!(!completions.is_empty());
        assert_eq!(completions[0].range.start, 11);
        assert_eq!(completions[0].range.end, 14);
    }

    #[test]
    fn suggestion_range_for_empty_argument() {
        let mut d = CommandDispatcher::new();
        d.register(
            literal("kick").then(
                argument(
                    "target",
                    ArgumentType::Entity {
                        single: true,
                        player_only: true,
                    },
                )
                .executes(|_| Ok(1)),
            ),
        );
        let src = make_source(4);
        let names = vec!["Alice".to_string()];
        // "kick " — trailing space, empty argument at offset 5
        // Should suggest entity selectors (@a, @e, @p, @r, @s, @n) plus
        // the online player name "Alice".
        let completions = d.get_completions("kick ", &src, &names);
        assert_eq!(completions.len(), 7);
        let texts: Vec<&str> = completions.iter().map(|c| c.text.as_str()).collect();
        assert!(texts.contains(&"@a"));
        assert!(texts.contains(&"@e"));
        assert!(texts.contains(&"@p"));
        assert!(texts.contains(&"@r"));
        assert!(texts.contains(&"@s"));
        assert!(texts.contains(&"@n"));
        assert!(texts.contains(&"Alice"));
        assert_eq!(completions[0].range.start, 5);
        assert_eq!(completions[0].range.end, 5);
    }

    #[test]
    fn suggestion_range_for_subcommand_literal() {
        let mut d = CommandDispatcher::new();
        d.register(
            literal("time")
                .then(literal("set").executes(|_| Ok(1)))
                .then(literal("query").executes(|_| Ok(2))),
        );
        let src = make_source(4);
        // "time s" — "s" starts at offset 5
        let completions = d.get_completions("time s", &src, &[]);
        assert_eq!(completions.len(), 1);
        assert_eq!(completions[0].text, "set");
        assert_eq!(completions[0].range.start, 5);
        assert_eq!(completions[0].range.end, 6);
    }

    // ── Serializer: Entity args get ask_server suggestions ──────────────

    #[test]
    fn serialize_entity_arg_has_ask_server_suggestion() {
        let mut d = CommandDispatcher::new();
        d.register(
            literal("kick").then(
                argument(
                    "target",
                    ArgumentType::Entity {
                        single: true,
                        player_only: true,
                    },
                )
                .executes(|_| Ok(1)),
            ),
        );
        let src = make_source(4);
        let tree = d.serialize_tree(&src);
        // Find the "target" argument node (index 2: root=0, kick=1, target=2)
        let target_node = &tree.nodes[2];
        assert_eq!(
            target_node.suggestions_type.as_deref(),
            Some("minecraft:ask_server"),
            "Entity arg should have ask_server suggestions"
        );
        // Should have FLAG_SUGGESTIONS (bit 4)
        assert_ne!(
            target_node.flags & 0x10,
            0,
            "Entity arg flags should have suggestions bit set"
        );
    }

    // ── Selector bracket filter key completion ─────────────────────────

    #[test]
    fn completions_suggest_filter_keys_after_bracket() {
        let mut d = CommandDispatcher::new();
        d.register(
            literal("kill").then(
                argument(
                    "target",
                    ArgumentType::Entity {
                        single: false,
                        player_only: false,
                    },
                )
                .executes(|_| Ok(1)),
            ),
        );
        let src = make_source(4);
        // "kill @a[" — inside brackets, empty key
        let completions = d.get_completions("kill @a[", &src, &[]);
        let texts: Vec<&str> = completions.iter().map(|c| c.text.as_str()).collect();
        assert!(texts.contains(&"name="), "should suggest name=");
        assert!(texts.contains(&"limit="), "should suggest limit=");
        assert!(texts.contains(&"sort="), "should suggest sort=");
        assert!(texts.contains(&"gamemode="), "should suggest gamemode=");
        assert!(texts.contains(&"distance="), "should suggest distance=");
        assert!(texts.contains(&"type="), "should suggest type=");
    }

    #[test]
    fn completions_filter_keys_by_prefix() {
        let mut d = CommandDispatcher::new();
        d.register(
            literal("kill").then(
                argument(
                    "target",
                    ArgumentType::Entity {
                        single: false,
                        player_only: false,
                    },
                )
                .executes(|_| Ok(1)),
            ),
        );
        let src = make_source(4);
        // "kill @a[na" — prefix "na" should match "name="
        let completions = d.get_completions("kill @a[na", &src, &[]);
        let texts: Vec<&str> = completions.iter().map(|c| c.text.as_str()).collect();
        assert!(texts.contains(&"name="), "should suggest name=");
        assert!(!texts.contains(&"limit="), "should not suggest limit=");
    }

    #[test]
    fn completions_suggest_sort_values() {
        let mut d = CommandDispatcher::new();
        d.register(
            literal("kill").then(
                argument(
                    "target",
                    ArgumentType::Entity {
                        single: false,
                        player_only: false,
                    },
                )
                .executes(|_| Ok(1)),
            ),
        );
        let src = make_source(4);
        // "kill @a[sort=" — suggest sort values
        let completions = d.get_completions("kill @a[sort=", &src, &[]);
        let texts: Vec<&str> = completions.iter().map(|c| c.text.as_str()).collect();
        assert!(texts.contains(&"nearest"), "should suggest nearest");
        assert!(texts.contains(&"furthest"), "should suggest furthest");
        assert!(texts.contains(&"random"), "should suggest random");
        assert!(texts.contains(&"arbitrary"), "should suggest arbitrary");
    }

    #[test]
    fn completions_suggest_gamemode_values() {
        let mut d = CommandDispatcher::new();
        d.register(
            literal("kill").then(
                argument(
                    "target",
                    ArgumentType::Entity {
                        single: false,
                        player_only: false,
                    },
                )
                .executes(|_| Ok(1)),
            ),
        );
        let src = make_source(4);
        // "kill @a[gamemode=" — suggest gamemode values
        let completions = d.get_completions("kill @a[gamemode=", &src, &[]);
        let texts: Vec<&str> = completions.iter().map(|c| c.text.as_str()).collect();
        assert!(texts.contains(&"survival"), "should suggest survival");
        assert!(texts.contains(&"creative"), "should suggest creative");
        assert!(texts.contains(&"adventure"), "should suggest adventure");
        assert!(texts.contains(&"spectator"), "should suggest spectator");
    }

    #[test]
    fn completions_suggest_filter_keys_after_comma() {
        let mut d = CommandDispatcher::new();
        d.register(
            literal("kill").then(
                argument(
                    "target",
                    ArgumentType::Entity {
                        single: false,
                        player_only: false,
                    },
                )
                .executes(|_| Ok(1)),
            ),
        );
        let src = make_source(4);
        // "kill @a[name=Steve," — after comma, suggest next keys
        let completions = d.get_completions("kill @a[name=Steve,", &src, &[]);
        let texts: Vec<&str> = completions.iter().map(|c| c.text.as_str()).collect();
        assert!(texts.contains(&"limit="), "should suggest limit=");
        assert!(texts.contains(&"sort="), "should suggest sort=");
    }

    #[test]
    fn completions_gamemode_values_filtered_by_prefix() {
        let mut d = CommandDispatcher::new();
        d.register(
            literal("kill").then(
                argument(
                    "target",
                    ArgumentType::Entity {
                        single: false,
                        player_only: false,
                    },
                )
                .executes(|_| Ok(1)),
            ),
        );
        let src = make_source(4);
        // "kill @a[gamemode=cr" — should match only creative
        let completions = d.get_completions("kill @a[gamemode=cr", &src, &[]);
        let texts: Vec<&str> = completions.iter().map(|c| c.text.as_str()).collect();
        assert!(texts.contains(&"creative"), "should suggest creative");
        assert_eq!(texts.len(), 1, "only creative should match 'cr' prefix");
    }
}
