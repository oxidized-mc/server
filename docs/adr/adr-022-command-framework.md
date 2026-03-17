# ADR-022: Command Framework

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-17 |
| Phases | P18 |
| Deciders | Oxidized Core Team |

## Context

Minecraft's command system allows players and server operators to interact with the game
through text commands (e.g., `/tp @a ~ ~10 ~`, `/give Steve diamond 64`,
`/data merge entity @e[type=zombie,limit=1] {NoAI:1b}`). Since Minecraft 1.13, the
command system is built on Mojang's Brigadier library — a graph-based command dispatcher
where each command is represented as a tree of nodes. Literal nodes match exact strings
(`tp`, `give`, `data`), and argument nodes parse typed values (integer, float, entity
selector, block position, resource location). The command graph is serialized and sent to
the client as a `ClientboundCommandsPacket`, enabling client-side tab completion, syntax
highlighting, and error underlining without round-trips to the server.

Brigadier is a Java library with a generic type parameter for its "source" type (in
vanilla, `CommandSourceStack`). It provides argument parsing, command dispatching,
suggestions, and error reporting. The API uses a fluent builder pattern:
`LiteralArgumentBuilder.literal("tp").then(RequiredArgumentBuilder.argument("target",
EntityArgument.entities()))`. The command graph supports redirects (aliases), forks
(execute-style multi-target), and requirements (permission checks). The graph structure is
isomorphic to the wire format — every node in the graph becomes a node in the
`ClientboundCommandsPacket`, which is a DAG (directed acyclic graph) with node indices
instead of pointers.

Oxidized cannot use Brigadier directly (it's Java) and cannot use an existing Rust command
framework like `clap` because the wire format is non-negotiable — the client expects a
specific packet structure for tab completion. We must build a Brigadier-compatible command
system in Rust that produces the exact same wire format and supports the same argument
types, while being idiomatic Rust and integrating with our ECS architecture.

## Decision Drivers

- **Wire format compatibility**: The `ClientboundCommandsPacket` must be byte-identical to
  what vanilla produces for the same command set. The client parses this packet to build its
  local command graph for tab completion and syntax highlighting.
- **Argument type fidelity**: All vanilla argument types must be supported — entity
  selectors (`@a`, `@p[distance=..5]`), block predicates (`stone`, `minecraft:oak_log[axis=y]`),
  NBT paths, JSON text components, scoreboard objectives, resource locations, coordinates
  (absolute, relative `~`, local `^`), and more.
- **Permission integration**: Commands must check permission levels (0-4) and support
  fine-grained `requires()` predicates matching vanilla's behavior.
- **Async suggestions**: Tab completion for some arguments (e.g., player names, scoreboard
  objectives) requires querying game state. Suggestions must support async resolution
  matching the `ServerboundCommandSuggestionPacket` → `ClientboundCommandSuggestionsPacket`
  flow.
- **Ergonomic registration API**: Developers should be able to register commands concisely
  without excessive boilerplate, while maintaining type safety for argument access.
- **Execute command support**: The `/execute` command's subcommand chaining (`/execute as
  @a at @s run tp ~ ~1 ~`) requires redirect nodes and context modification, which is one
  of Brigadier's most complex features.

## Considered Options

### Option 1: Port Brigadier 1:1 to Rust

Translate Brigadier's Java source code to Rust line by line, preserving the same class
hierarchy, generic types, and API surface.

**Pros:**
- Exact behavioral match — same code, same results.
- Easy to verify against vanilla.
- Familiar API for anyone who's used Brigadier in Java modding.

**Cons:**
- Brigadier's design is heavily OOP (abstract classes, inheritance, type erasure via
  generics) — translating this to Rust produces unidiomatic code with lots of `Box<dyn>`,
  `Arc<dyn>`, and `Any` downcasting.
- Brigadier uses mutable shared state (the `CommandDispatcher` is mutated during
  registration and read during dispatch) — hard to make thread-safe in Rust without locks.
- Java-isms like `null` returns, checked exceptions, and `Optional` don't map cleanly.

**Verdict: Rejected.** Would produce ugly, unidiomatic Rust.

### Option 2: Brigadier-Compatible Command Graph with Rust Builder DSL

Build a Brigadier-compatible command graph using a fluent Rust builder API. The internal
representation is a graph of nodes (literals and arguments) with the same semantics as
Brigadier. The graph serializes to the same wire format. But the API is idiomatic Rust:
builder pattern, closures for execution, trait objects for argument types.

**Pros:**
- Wire format compatibility without slavish Java translation.
- Idiomatic Rust API with builder pattern and closures.
- Can enforce more safety at compile time (typed argument access).
- Graph is built once during server startup, then immutable during operation — no lock
  contention.

**Cons:**
- Must carefully verify wire format compatibility — any divergence breaks tab completion.
- Building a graph serializer that exactly matches vanilla's node indexing and flag encoding
  requires careful implementation against the protocol spec.
- Some Brigadier features (redirects, forks) are complex to implement correctly.

**Verdict: Selected.** Best balance of compatibility and Rust idiom.

### Option 3: Derive Macro Approach (`#[derive(Command)]`)

Use Rust derive macros to generate command trees from annotated structs or enums.

```rust
#[derive(Command)]
#[command(name = "gamemode")]
struct GameModeCommand {
    #[argument(type = "gamemode")]
    mode: GameMode,
    #[argument(type = "entity", optional)]
    target: Option<EntitySelector>,
}
```

**Pros:**
- Very concise command definitions.
- Compile-time type checking for arguments.
- Familiar to developers who've used `clap` derive macros.

**Cons:**
- Proc macros are complex to implement and debug.
- Brigadier's graph structure (with redirects, forks, and multi-path commands) doesn't map
  cleanly to flat struct definitions.
- The `/execute` command with its recursive subcommand chaining is extremely difficult to
  represent as a derive macro.
- The macro must generate the correct wire format graph, adding a layer of indirection.

**Verdict: Rejected.** Too rigid for Brigadier's graph complexity, especially `/execute`.

### Option 4: Existing Rust Command Framework (clap-style)

Use `clap`, `pico-args`, or another Rust CLI parsing library, adapted for Minecraft's
needs.

**Pros:**
- Mature, well-tested parsing logic.
- Rich ecosystem of argument types and completions.

**Cons:**
- None of these produce Brigadier's wire format. We'd need a translation layer.
- CLI parsers assume POSIX-style arguments (`--flag value`); Minecraft uses positional
  arguments with custom syntax.
- Entity selectors, NBT paths, and coordinate syntax are Minecraft-specific — no existing
  parser handles them.
- Tab completion models are fundamentally different (terminal completions vs. graph-based
  suggestions).

**Verdict: Rejected.** Wrong abstraction — CLI parsers solve a different problem.

## Decision

**We build a Brigadier-compatible command graph with a Rust builder DSL.** The command
system consists of three layers: (1) a graph data structure representing the command tree,
(2) a builder API for constructing the graph, and (3) a dispatcher that parses input
strings, matches them against the graph, and executes the corresponding command function.

### Command Graph Structure

```rust
/// A node in the command graph.
struct CommandNode {
    /// Node type: root, literal, or argument.
    node_type: NodeType,
    /// Child node indices in the graph.
    children: Vec<NodeIndex>,
    /// Redirect target (for aliases and /execute chaining).
    redirect: Option<NodeIndex>,
    /// Permission check — must return true for the node to be visible/executable.
    requirement: Option<Box<dyn Fn(&CommandSource) -> bool + Send + Sync>>,
    /// Execution function — called when this node is the terminal node of a parse.
    executes: Option<Box<dyn Fn(&CommandContext) -> CommandResult + Send + Sync>>,
}

enum NodeType {
    Root,
    Literal { name: String },
    Argument { name: String, arg_type: Box<dyn ArgumentType>, suggestions: Option<SuggestionProvider> },
}

/// The complete command graph, immutable after construction.
struct CommandDispatcher {
    nodes: Vec<CommandNode>,
    root: NodeIndex,
}
```

### Builder API

```rust
let dispatcher = CommandDispatcher::builder()
    .register(
        literal("tp")
            .requires(|src| src.has_permission(2))
            .then(
                argument("target", EntityArgument::entities())
                    .then(
                        argument("destination", EntityArgument::entity())
                            .executes(|ctx| {
                                let targets = ctx.get::<Vec<Entity>>("target")?;
                                let dest = ctx.get::<Entity>("destination")?;
                                let dest_pos = ctx.world().get::<Position>(dest)?;
                                for target in targets {
                                    ctx.world_mut().get_mut::<Position>(target)?.0 = dest_pos.0;
                                }
                                Ok(targets.len() as i32)
                            })
                    )
                    .then(
                        argument("location", Vec3Argument::new())
                            .executes(|ctx| {
                                let targets = ctx.get::<Vec<Entity>>("target")?;
                                let loc = ctx.get::<DVec3>("location")?;
                                for target in targets {
                                    ctx.world_mut().get_mut::<Position>(target)?.0 = loc;
                                }
                                Ok(targets.len() as i32)
                            })
                    )
            )
    )
    .build();
```

### Wire Format Serialization

The `ClientboundCommandsPacket` encodes the command graph as:

```
VarInt: node_count
For each node:
  u8: flags (node_type in bits 0-1, has_executes in bit 2, has_redirect in bit 3, has_suggestions in bit 4)
  VarInt: children_count
  VarInt[]: children (node indices)
  Optional<VarInt>: redirect_target (node index)
  // If literal:
  String: name
  // If argument:
  String: name
  VarInt: parser_id (registry index for argument type)
  Optional<...>: parser-specific properties (min/max for integers, etc.)
  Optional<Identifier>: custom_suggestions (e.g., "minecraft:ask_server")
VarInt: root_index
```

Our serializer traverses the `CommandDispatcher` graph and produces this exact byte
sequence. Node indices are assigned by a BFS traversal of the graph, matching vanilla's
ordering. This is critical — the client uses these indices to navigate the graph.

### Argument Types

Each argument type implements the `ArgumentType` trait:

```rust
trait ArgumentType: Send + Sync {
    /// Parse the argument from the input string reader.
    fn parse(&self, reader: &mut StringReader) -> Result<Box<dyn Any>, CommandSyntaxException>;

    /// Serialize the argument type's properties for the wire format.
    fn serialize_properties(&self, buf: &mut BytesMut);

    /// The parser identifier (e.g., "brigadier:integer", "minecraft:entity").
    fn parser_id(&self) -> ResourceLocation;

    /// Optional: provide suggestions for this argument.
    fn suggest(&self, context: &CommandContext, builder: &mut SuggestionsBuilder)
        -> Result<(), CommandSyntaxException> {
        Ok(()) // default: no suggestions
    }
}
```

Vanilla argument types that must be implemented:

| Parser ID | Rust Type | Notes |
|-----------|-----------|-------|
| `brigadier:bool` | `BoolArgument` | `true`/`false` |
| `brigadier:integer` | `IntegerArgument` | With optional min/max |
| `brigadier:long` | `LongArgument` | With optional min/max |
| `brigadier:float` | `FloatArgument` | With optional min/max |
| `brigadier:double` | `DoubleArgument` | With optional min/max |
| `brigadier:string` | `StringArgument` | Word, quotable, or greedy |
| `minecraft:entity` | `EntityArgument` | Single entity or multiple, players-only flag |
| `minecraft:game_profile` | `GameProfileArgument` | Player name or selector |
| `minecraft:block_pos` | `BlockPosArgument` | Integer coordinates with `~` support |
| `minecraft:vec3` | `Vec3Argument` | Double coordinates with `~` and `^` support |
| `minecraft:vec2` | `Vec2Argument` | Horizontal coordinates |
| `minecraft:block_state` | `BlockStateArgument` | Block ID + optional properties + NBT |
| `minecraft:item_stack` | `ItemArgument` | Item ID + optional components |
| `minecraft:component` | `ComponentArgument` | JSON text component |
| `minecraft:nbt_compound_tag` | `NbtArgument` | SNBT compound tag |
| `minecraft:nbt_path` | `NbtPathArgument` | NBT path expression |
| `minecraft:resource_location` | `ResourceLocationArgument` | Namespaced ID |
| `minecraft:gamemode` | `GameModeArgument` | survival, creative, adventure, spectator |
| `minecraft:time` | `TimeArgument` | Tick count with d/s/t suffix |
| `minecraft:resource` | `ResourceArgument` | Registry resource reference |

### Entity Selector Parsing

Entity selectors (`@a`, `@p`, `@r`, `@e`, `@s`) are one of the most complex argument
types. The selector syntax is:

```
@<type>[<key>=<value>,<key>=<value>,...]

Types: a (all players), p (nearest player), r (random player), e (all entities), s (self)
Filters: type, distance, x/y/z, dx/dy/dz, scores, tag, team, name, limit, sort, level,
         gamemode, nbt, advancements, predicate
```

The selector parser produces a `EntitySelector` struct that is resolved against the ECS
world at execution time. Resolution queries entities matching all filter criteria and
applies limit/sort. The `@p` selector resolves to the nearest player to the command source;
`@e[type=zombie,distance=..10,limit=5,sort=nearest]` finds the 5 closest zombies within
10 blocks.

### Command Suggestions (Tab Completion)

When a player presses Tab, the client sends `ServerboundCommandSuggestionPacket` with the
current input text and cursor position. The server:

1. Parses the input up to the cursor position against the command graph.
2. Determines which node the cursor is in (literal or argument).
3. Calls the node's `suggest()` method (or the argument type's default suggestions).
4. Returns `ClientboundCommandSuggestionsPacket` with the suggestion list.

For arguments with `suggestions: Some(SuggestionProvider::AskServer)`, the server provides
dynamic suggestions (e.g., online player names, loaded dimension names, scoreboard
objectives). For arguments with client-side suggestion types (like `minecraft:summonable_entities`),
the client handles suggestions locally without a round trip.

### Permission Levels

Commands check permission levels 0-4, matching vanilla:

| Level | Description | Example Commands |
|-------|-------------|------------------|
| 0 | All players | `/help`, `/me`, `/msg` |
| 1 | Moderators | (Not commonly used) |
| 2 | Game masters (ops) | `/gamemode`, `/tp`, `/give`, `/kill` |
| 3 | Admins | `/ban`, `/op`, `/whitelist` |
| 4 | Server owner | `/stop`, `/debug` |

The `requires()` predicate on each node controls visibility and executability. Nodes that
fail the requirement check are excluded from the `ClientboundCommandsPacket` sent to that
specific player — each player receives a personalized command graph based on their
permission level.

## Consequences

### Positive

- **Perfect tab completion**: Players get the same tab-completion experience as vanilla,
  because the wire format is identical.
- **Type-safe argument access**: `ctx.get::<i32>("count")` is checked at the type level,
  preventing runtime type errors that are common in vanilla's `getArgument()`.
- **Immutable dispatch graph**: The graph is built once at startup and never mutated,
  making it trivially thread-safe and eliminating a class of concurrency bugs.
- **Ergonomic builder API**: Command registration is concise and readable, comparable to
  vanilla Brigadier's fluent API.
- **Modular argument types**: New argument types can be added by implementing `ArgumentType`,
  without modifying the core dispatcher.

### Negative

- **Entity selector complexity**: The entity selector parser and resolver is hundreds of
  lines of code, covering many filter types, sort modes, and edge cases. This is
  inherent complexity that cannot be avoided.
- **`/execute` complexity**: The `/execute` command's subcommand chaining (run, as, at,
  positioned, if, unless, store, etc.) requires redirect nodes that modify the command
  context. This is the most complex part of the command system.
- **Per-player graph serialization**: Each player may see a different command graph (based
  on permissions), requiring re-serialization per player. This can be cached by permission
  level to avoid redundant work.

### Neutral

- **No proc macros**: We chose the builder pattern over derive macros. This means slightly
  more verbose command registration but simpler tooling and better error messages.
- **SNBT parsing**: NBT arguments require an SNBT (Stringified NBT) parser, which is a
  separate subsystem used by commands and data packs.

## Compliance

- **Wire format test**: Serialize our command graph for the default vanilla command set
  and compare the output byte-for-byte with vanilla's `ClientboundCommandsPacket`.
- **Entity selector test suite**: Test all selector types and filter combinations against
  vanilla's behavior (e.g., `@e[type=zombie,distance=..10,limit=3,sort=nearest]` with a
  known entity layout).
- **Permission filtering test**: Connect with different op levels and verify the received
  command graph matches vanilla's permission-filtered graph.
- **Tab completion integration test**: Send `ServerboundCommandSuggestionPacket` for
  various partial inputs and verify suggestions match vanilla.
- **Argument parsing fuzz test**: Fuzz each argument type's parser with random input and
  verify it either parses correctly or returns the same error as vanilla.

## Related ADRs

- **ADR-018**: Entity System Architecture — entity selectors resolve against ECS queries
- **ADR-019**: Tick Loop Design — commands are processed during NETWORK_RECEIVE phase
- **ADR-020**: Player Session Lifecycle — command packets flow through the network bridge

## References

- [Mojang Brigadier (Java)](https://github.com/Mojang/brigadier) — the original library
- Vanilla source: `net.minecraft.commands.Commands` — command registration
- Vanilla source: `net.minecraft.commands.CommandSourceStack` — command execution context
- Vanilla source: `net.minecraft.commands.arguments.EntityArgument` — entity selector
- Vanilla source: `net.minecraft.network.protocol.game.ClientboundCommandsPacket`
- [wiki.vg — Command Data](https://wiki.vg/Command_Data) — wire format documentation
- [Minecraft Wiki — Commands](https://minecraft.wiki/w/Commands) — end-user command docs
