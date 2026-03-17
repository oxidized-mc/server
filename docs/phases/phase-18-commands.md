# Phase 18 — Commands (Brigadier)

**Crate:** `oxidized-game`  
**Reward:** `/help`, `/tp`, `/gamemode`, `/stop` all work; tab-complete works.

---

## Goal

Reimplement the Brigadier command framework in Rust, register the full set of
vanilla server commands, and hook up tab-completion. The command tree is
serialized and sent to the client on join so the client can locally tab-complete
and highlight syntax errors without round-tripping the server for every keypress.

---

## Java Reference

| Concept | Java class |
|---------|-----------|
| Command registration hub | `net.minecraft.commands.Commands` |
| Brigadier dispatcher | `com.mojang.brigadier.CommandDispatcher` |
| Literal node | `com.mojang.brigadier.tree.LiteralCommandNode` |
| Argument node | `com.mojang.brigadier.tree.ArgumentCommandNode` |
| Root node | `com.mojang.brigadier.tree.RootCommandNode` |
| Command context | `com.mojang.brigadier.context.CommandContext` |
| Command source | `net.minecraft.commands.CommandSourceStack` |
| Command source factory | `net.minecraft.server.level.ServerPlayer#createCommandSourceStack` |
| Suggestions packet (C→S) | `net.minecraft.network.protocol.game.ServerboundCommandSuggestionsPacket` |
| Suggestions packet (S→C) | `net.minecraft.network.protocol.game.ClientboundCommandSuggestionsPacket` |
| Command tree packet | `net.minecraft.network.protocol.game.ClientboundCommandsPacket` |
| String argument | `com.mojang.brigadier.arguments.StringArgumentType` |
| Integer argument | `com.mojang.brigadier.arguments.IntegerArgumentType` |
| Entity selector parser | `net.minecraft.commands.arguments.EntityArgument` |
| Block pos argument | `net.minecraft.commands.arguments.coordinates.BlockPosArgument` |
| Gamemode argument | `net.minecraft.commands.arguments.GameModeArgument` |

---

## Tasks

### 18.1 — Core Brigadier types (`oxidized-game/src/commands/dispatcher.rs`)

```rust
use std::collections::HashMap;
use std::sync::Arc;

pub type CommandFn<S> = Arc<dyn Fn(&CommandContext<S>) -> anyhow::Result<i32> + Send + Sync>;
pub type RequirementFn<S> = Arc<dyn Fn(&S) -> bool + Send + Sync>;
pub type SuggestFn<S> = Arc<dyn Fn(&CommandContext<S>, &str) -> Vec<Suggestion> + Send + Sync>;

pub struct CommandDispatcher<S> {
    pub root: RootCommandNode<S>,
}

impl<S: Clone + Send + Sync + 'static> CommandDispatcher<S> {
    pub fn new() -> Self {
        Self { root: RootCommandNode::new() }
    }

    pub fn register(&mut self, node: LiteralBuilder<S>) -> &mut CommandNode<S> {
        let built = node.build();
        self.root.add_child(built)
    }

    /// Parse input and return the matching command and parsed context.
    pub fn parse(&self, input: &str, source: S) -> Result<ParseResults<S>, CommandSyntaxError> {
        let mut ctx = CommandContextBuilder::new(source, self.root.clone(), 0);
        parse_nodes(&self.root.children, input, &mut ctx)
    }

    /// Execute a previously-parsed command. Returns the result integer.
    pub fn execute(&self, parse: ParseResults<S>) -> anyhow::Result<i32> {
        let Some(cmd) = parse.context.command.as_ref() else {
            return Err(anyhow::anyhow!("Incomplete command"));
        };
        (cmd)(&parse.context.build())
    }

    /// Collect completions for the cursor position in `input`.
    pub fn get_completions(&self, input: &str, source: &S) -> Vec<Suggestion> {
        // Walk tree up to cursor, gather applicable suggestions
        collect_suggestions(&self.root, input, source)
    }

    /// Serialize the command tree for ClientboundCommandsPacket.
    pub fn serialize_tree(&self) -> CommandTreeData {
        serialize_node_recursive(&CommandNode::Root(self.root.clone()))
    }
}
```

### 18.2 — Command node types (`oxidized-game/src/commands/nodes.rs`)

```rust
pub enum CommandNode<S> {
    Root(RootCommandNode<S>),
    Literal(LiteralCommandNode<S>),
    Argument(ArgumentCommandNode<S>),
}

pub struct RootCommandNode<S> {
    pub children: HashMap<String, CommandNode<S>>,
}

pub struct LiteralCommandNode<S> {
    pub literal: String,
    pub children: HashMap<String, CommandNode<S>>,
    pub redirect: Option<Arc<CommandNode<S>>>,
    pub command: Option<CommandFn<S>>,
    pub requirement: Option<RequirementFn<S>>,
}

pub struct ArgumentCommandNode<S> {
    pub name: String,
    pub argument_type: ArgumentType,
    pub children: HashMap<String, CommandNode<S>>,
    pub redirect: Option<Arc<CommandNode<S>>>,
    pub command: Option<CommandFn<S>>,
    pub requirement: Option<RequirementFn<S>>,
    pub custom_suggestions: Option<SuggestFn<S>>,
}

impl<S> RootCommandNode<S> {
    pub fn new() -> Self {
        Self { children: HashMap::new() }
    }

    pub fn add_child(&mut self, node: CommandNode<S>) -> &mut CommandNode<S> {
        let name = node.name().to_string();
        self.children.entry(name).or_insert(node)
    }
}
```

### 18.3 — Argument types (`oxidized-game/src/commands/arguments.rs`)

```rust
#[derive(Debug, Clone)]
pub enum ArgumentType {
    Bool,
    Integer { min: Option<i32>, max: Option<i32> },
    Float   { min: Option<f32>, max: Option<f32> },
    Double  { min: Option<f64>, max: Option<f64> },
    String(StringKind),
    Entity  { single: bool, player_only: bool },
    GameProfile,
    BlockPos,
    ColumnPos,
    Vec3,
    Vec2,
    BlockState,
    ItemStack,
    Color,
    Component,
    Message,
    Nbt,
    NbtPath,
    Objective,
    ObjectiveCriteria,
    Operation,
    Particle,
    Angle,
    Rotation,
    ScoreboardSlot,
    ScoreHolder { allow_multiple: bool },
    Swizzle,
    Team,
    ItemSlot,
    ItemSlots,
    ResourceLocation,
    Dimension,
    Gamemode,
    Time { min: i32 },
    ResourceKey { registry: String },
    Resource    { registry: String },
    ResourceOrTag { registry: String },
    Uuid,
    // Used for /tick
    TemplateMirror,
    TemplateRotation,
}

#[derive(Debug, Clone, Copy)]
pub enum StringKind {
    SingleWord,    // no spaces, stops at first whitespace
    QuotablePhrase,// optionally quoted
    GreedyPhrase,  // everything to end of input
}

impl ArgumentType {
    /// Brigadier node type id sent in ClientboundCommandsPacket.
    pub fn brigadier_type_id(&self) -> &'static str {
        match self {
            Self::Bool          => "brigadier:bool",
            Self::Integer {..}  => "brigadier:integer",
            Self::Float {..}    => "brigadier:float",
            Self::Double {..}   => "brigadier:double",
            Self::String(_)     => "brigadier:string",
            Self::Entity {..}   => "minecraft:entity",
            Self::GameProfile   => "minecraft:game_profile",
            Self::BlockPos      => "minecraft:block_pos",
            Self::Vec3          => "minecraft:vec3",
            Self::BlockState    => "minecraft:block_state",
            Self::ItemStack     => "minecraft:item_stack",
            Self::Component     => "minecraft:component",
            Self::Gamemode      => "minecraft:game_mode",
            Self::ResourceLocation => "minecraft:resource_location",
            Self::Uuid          => "minecraft:uuid",
            Self::Dimension     => "minecraft:dimension",
            Self::Time {..}     => "minecraft:time",
            _                   => "minecraft:resource_location",
        }
    }
}
```

### 18.4 — CommandSource trait (`oxidized-game/src/commands/source.rs`)

```rust
use crate::chat::Component;
use crate::world::ServerLevel;

pub trait CommandSource: Clone + Send + Sync + 'static {
    fn send_message(&self, component: Component);
    fn send_failure(&self, component: Component);
    fn send_success(&self, component: Component, broadcast_to_ops: bool);

    fn get_position(&self) -> Option<(f64, f64, f64)>;
    fn get_level(&self) -> Option<Arc<ServerLevel>>;
    fn get_entity(&self) -> Option<EntityRef>;
    fn get_server(&self) -> Arc<MinecraftServer>;

    fn has_permission(&self, level: u32) -> bool;
    fn display_name(&self) -> String;
    fn rotation(&self) -> Option<(f32, f32)>;  // (yaw, pitch)
}

/// Concrete source for player-originated commands.
#[derive(Clone)]
pub struct CommandSourceStack {
    pub source: CommandSourceKind,
    pub position: (f64, f64, f64),
    pub rotation: (f32, f32),
    pub level: Arc<ServerLevel>,
    pub permission_level: u32,
    pub textname: String,
    pub display_name: Component,
    pub server: Arc<MinecraftServer>,
    pub silent: bool,
}

#[derive(Clone)]
pub enum CommandSourceKind {
    Player(Arc<Mutex<ServerPlayer>>),
    Entity(EntityRef),
    Console,
    CommandBlock { pos: BlockPos },
    Rcon,
}
```

### 18.5 — ClientboundCommandsPacket (`oxidized-protocol/src/packets/clientbound/game.rs`)

```rust
/// 0x11 – sends the full command tree to the client on join/update.
#[derive(Debug, Clone)]
pub struct ClientboundCommandsPacket {
    pub nodes: Vec<CommandNode>,   // flattened graph; index 0 is root
    pub root_index: i32,           // VarInt index into nodes[]
}

#[derive(Debug, Clone)]
pub struct CommandNodeData {
    pub flags: u8,
    // flags bits:
    //   0-1: node type (0=root, 1=literal, 2=argument)
    //   2:   executable (has command fn)
    //   3:   has redirect
    //   4:   has custom suggestions
    pub children: Vec<i32>,        // VarInt indices
    pub redirect_node: Option<i32>,
    pub name: Option<String>,      // literal/argument name
    pub parser: Option<ArgumentTypeData>,
    pub suggestions_type: Option<String>, // resource location
}

impl Encode for ClientboundCommandsPacket {
    fn encode(&self, buf: &mut impl BufMut) -> anyhow::Result<()> {
        VarInt(self.nodes.len() as i32).encode(buf)?;
        for node in &self.nodes {
            node.encode(buf)?;
        }
        VarInt(self.root_index).encode(buf)?;
        Ok(())
    }
}
```

### 18.6 — Tab-complete packets

```rust
/// 0x20 (C→S) – client requests completions
#[derive(Debug, Clone)]
pub struct ServerboundCommandSuggestionsPacket {
    pub id: i32,     // VarInt, echoed back
    pub command: String,
}

/// 0x10 (S→C) – server returns completions
#[derive(Debug, Clone)]
pub struct ClientboundCommandSuggestionsPacket {
    pub id: i32,
    pub start: i32,
    pub length: i32,
    pub suggestions: Vec<Suggestion>,
}

#[derive(Debug, Clone)]
pub struct Suggestion {
    pub text: String,
    pub tooltip: Option<Component>,
}
```

### 18.7 — Core commands (`oxidized-game/src/commands/`)

Each command is in its own file under `oxidized-game/src/commands/`:

```rust
// stop.rs
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(literal("stop")
        .requires(|s| s.has_permission(4))
        .executes(|ctx| {
            ctx.source.send_success(Component::text("Stopping the server"), true);
            ctx.source.get_server().request_shutdown();
            Ok(1)
        }));
}

// tp.rs
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(literal("tp")
        .requires(|s| s.has_permission(2))
        // /tp <x> <y> <z>
        .then(argument("x", ArgumentType::Vec3)
            .executes(|ctx| {
                let pos = get_vec3(&ctx, "x")?;
                let entity = ctx.source.get_entity().ok_or_else(|| command_err("No entity"))?;
                entity.teleport(pos);
                ctx.source.send_success(Component::text(format!("Teleported to {:.2} {:.2} {:.2}", pos.0, pos.1, pos.2)), true);
                Ok(1)
            }))
        // /tp <destination> (entity selector)
        .then(argument("destination", ArgumentType::Entity { single: true, player_only: false })
            .executes(|ctx| {
                let dest = get_entity(&ctx, "destination")?;
                let src  = ctx.source.get_entity().ok_or_else(|| command_err("No entity"))?;
                let pos  = dest.position();
                src.teleport(pos);
                Ok(1)
            }))
        // /tp <target> <destination>
        .then(argument("target", ArgumentType::Entity { single: false, player_only: false })
            .then(argument("dest2", ArgumentType::Entity { single: true, player_only: false })
                .executes(|ctx| {
                    let targets = get_entities(&ctx, "target")?;
                    let dest    = get_entity(&ctx, "dest2")?;
                    let pos     = dest.position();
                    for t in targets { t.teleport(pos); }
                    Ok(targets.len() as i32)
                })))
    );
    // /teleport is an alias
    d.register(literal("teleport")
        .requires(|s| s.has_permission(2))
        .redirect(d.root.get_child("tp").unwrap()));
}

// gamemode.rs
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(literal("gamemode")
        .requires(|s| s.has_permission(2))
        .then(argument("gamemode", ArgumentType::Gamemode)
            .executes(|ctx| {
                let gm = get_gamemode(&ctx, "gamemode")?;
                let player = require_player(&ctx)?;
                player.set_game_mode(gm);
                ctx.source.send_success(
                    Component::translate("commands.gamemode.success.self",
                        vec![gm.display_name()]), true);
                Ok(1)
            })
            .then(argument("target", ArgumentType::Entity { single: false, player_only: true })
                .executes(|ctx| {
                    let gm      = get_gamemode(&ctx, "gamemode")?;
                    let players = get_players(&ctx, "target")?;
                    for p in &players { p.set_game_mode(gm); }
                    Ok(players.len() as i32)
                }))
        )
    );
}

// give.rs
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(literal("give")
        .requires(|s| s.has_permission(2))
        .then(argument("targets", ArgumentType::Entity { single: false, player_only: true })
            .then(argument("item", ArgumentType::ItemStack)
                .executes(|ctx| give_items(&ctx, 1))
                .then(argument("count", ArgumentType::Integer { min: Some(1), max: Some(2147483647) })
                    .executes(|ctx| {
                        let count = get_integer(&ctx, "count")?;
                        give_items(&ctx, count)
                    })))));
}

// time.rs
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(literal("time")
        .requires(|s| s.has_permission(2))
        .then(literal("set")
            .then(literal("day")    .executes(|ctx| set_time(&ctx, 1000)))
            .then(literal("noon")   .executes(|ctx| set_time(&ctx, 6000)))
            .then(literal("night")  .executes(|ctx| set_time(&ctx, 13000)))
            .then(literal("midnight").executes(|ctx| set_time(&ctx, 18000)))
            .then(argument("time", ArgumentType::Time { min: 0 })
                .executes(|ctx| { let t = get_time(&ctx, "time")?; set_time(&ctx, t) })))
        .then(literal("add")
            .then(argument("time", ArgumentType::Time { min: 0 })
                .executes(|ctx| {
                    let add = get_time(&ctx, "time")?;
                    ctx.source.get_level().unwrap().add_day_time(add);
                    Ok(1)
                })))
        .then(literal("query")
            .then(literal("daytime")  .executes(|ctx| query_time(&ctx, TimeQuery::DayTime)))
            .then(literal("gametime") .executes(|ctx| query_time(&ctx, TimeQuery::GameTime)))
            .then(literal("day")      .executes(|ctx| query_time(&ctx, TimeQuery::Day))))
    );
}

// weather.rs
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(literal("weather")
        .requires(|s| s.has_permission(2))
        .then(literal("clear")  .executes(|ctx| set_weather(&ctx, Weather::Clear, None))
            .then(argument("duration", ArgumentType::Time { min: 0 })
                .executes(|ctx| set_weather(&ctx, Weather::Clear, Some(get_time(&ctx, "duration")?)))))
        .then(literal("rain")   .executes(|ctx| set_weather(&ctx, Weather::Rain, None))
            .then(argument("duration", ArgumentType::Time { min: 0 })
                .executes(|ctx| set_weather(&ctx, Weather::Rain, Some(get_time(&ctx, "duration")?)))))
        .then(literal("thunder").executes(|ctx| set_weather(&ctx, Weather::Thunder, None))
            .then(argument("duration", ArgumentType::Time { min: 0 })
                .executes(|ctx| set_weather(&ctx, Weather::Thunder, Some(get_time(&ctx, "duration")?)))))
    );
}
```

### 18.8 — Remaining command registrations (stub signatures)

```rust
// list.rs
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>);   // /list [uuids]

// op_deop.rs
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>);   // /op <player>, /deop <player>

// whitelist.rs
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>);   // /whitelist add|remove|list|on|off|reload

// ban.rs
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>);   // /ban /ban-ip /pardon

// kick.rs
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>);   // /kick <player> [reason]

// difficulty.rs
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>);   // /difficulty [peaceful|easy|normal|hard]

// gamerule.rs
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>);   // /gamerule [rule] [value]

// help.rs
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>);   // /help [command]

// seed.rs
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>);   // /seed (op 2)

// kill.rs
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>);   // /kill [target]

// summon.rs
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>);   // /summon <entity> [pos]

// setblock.rs
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>);   // /setblock <pos> <block> [mode]

// fill.rs
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>);   // /fill <from> <to> <block> [mode]

// clone.rs
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>);   // /clone <from> <to> <dest>

// effect.rs
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>);   // /effect give|clear ...
```

### 18.9 — Commands hub (`oxidized-game/src/commands/mod.rs`)

```rust
pub struct Commands {
    dispatcher: CommandDispatcher<CommandSourceStack>,
}

impl Commands {
    pub fn new(environment: Environment) -> Self {
        let mut d = CommandDispatcher::new();
        stop::register(&mut d);
        tp::register(&mut d);
        gamemode::register(&mut d);
        give::register(&mut d);
        kill::register(&mut d);
        time::register(&mut d);
        weather::register(&mut d);
        chat_commands::register_say(&mut d);
        chat_commands::register_me(&mut d);
        list::register(&mut d);
        op_deop::register(&mut d);
        whitelist::register(&mut d);
        ban::register(&mut d);
        kick::register(&mut d);
        difficulty::register(&mut d);
        gamerule::register(&mut d);
        help::register(&mut d);
        seed::register(&mut d);
        summon::register(&mut d);
        setblock::register(&mut d);
        fill::register(&mut d);
        clone::register(&mut d);
        effect::register(&mut d);
        tick::register(&mut d);
        Self { dispatcher: d }
    }

    pub fn dispatch(&self, input: &str, source: CommandSourceStack) -> anyhow::Result<i32> {
        let input = input.trim_start_matches('/');
        let parse = self.dispatcher.parse(input, source)
            .map_err(|e| anyhow::anyhow!("Unknown command: {}", e))?;
        self.dispatcher.execute(parse)
    }

    pub fn completions(&self, input: &str, source: &CommandSourceStack) -> Vec<Suggestion> {
        self.dispatcher.get_completions(input, source)
    }

    pub fn commands_packet(&self) -> ClientboundCommandsPacket {
        self.dispatcher.serialize_tree().into_packet()
    }
}
```

---

## Data Structures

```rust
// oxidized-game/src/commands/context.rs

pub struct CommandContext<S> {
    pub source: S,
    pub input: String,
    pub arguments: HashMap<String, ParsedArgument>,
    pub command: Option<CommandFn<S>>,
    pub root_node: Arc<CommandNode<S>>,
    pub nodes: Vec<ParsedCommandNode<S>>,
    pub range: StringRange,
    pub child: Option<Box<CommandContext<S>>>,
    pub modifier: Option<RedirectModifier<S>>,
    pub forks: bool,
}

pub struct ParsedArgument {
    pub range: StringRange,
    pub result: ArgumentResult,
}

pub enum ArgumentResult {
    Bool(bool),
    Integer(i32),
    Float(f32),
    Double(f64),
    String(String),
    BlockPos(BlockPos),
    Vec3(f64, f64, f64),
    Gamemode(GameType),
    Entities(Vec<EntityRef>),
    ResourceLocation(String),
    Uuid(uuid::Uuid),
    Component(Component),
    ItemStack(ItemStack),
    Time(i32),
}

#[derive(Debug, Clone, Copy)]
pub struct StringRange {
    pub start: usize,
    pub end: usize,
}

pub struct ParseResults<S> {
    pub context: CommandContextBuilder<S>,
    pub reader: StringReader,
    pub exceptions: HashMap<CommandNode<S>, CommandSyntaxError>,
}

pub struct Suggestion {
    pub range: StringRange,
    pub text: String,
    pub tooltip: Option<Component>,
}
```

---

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn make_console_source(server: Arc<MinecraftServer>) -> CommandSourceStack {
        CommandSourceStack {
            source: CommandSourceKind::Console,
            position: (0.0, 64.0, 0.0),
            rotation: (0.0, 0.0),
            level: server.overworld(),
            permission_level: 4,
            textname: "Console".into(),
            display_name: Component::text("Console"),
            server,
            silent: false,
        }
    }

    // --- Dispatcher ---

    #[test]
    fn dispatcher_executes_literal_command() {
        // Registers literal "ping" that returns 42; dispatch("ping") returns Ok(42)
        let mut d = CommandDispatcher::new();
        d.register(literal("ping").executes(|_| Ok(42)));
        let src = make_console_source(Arc::new(MockServer::new()));
        let parse = d.parse("ping", src).unwrap();
        assert_eq!(d.execute(parse).unwrap(), 42);
    }

    #[test]
    fn dispatcher_returns_error_for_unknown_command() {
        let d = CommandDispatcher::<CommandSourceStack>::new();
        let src = make_console_source(Arc::new(MockServer::new()));
        assert!(d.parse("unknowncommand", src).is_err());
    }

    #[test]
    fn dispatcher_parses_integer_argument() {
        // /time set <value> parses an integer and stores it in context
        let mut d = CommandDispatcher::new();
        d.register(literal("test")
            .then(argument("n", ArgumentType::Integer { min: Some(0), max: None })
                .executes(|ctx| {
                    let n = get_integer(ctx, "n")?;
                    Ok(n)
                })));
        let src = make_console_source(Arc::new(MockServer::new()));
        let parse = d.parse("test 7", src).unwrap();
        assert_eq!(d.execute(parse).unwrap(), 7);
    }

    #[test]
    fn dispatcher_integer_argument_rejects_out_of_range() {
        let mut d = CommandDispatcher::new();
        d.register(literal("setval")
            .then(argument("n", ArgumentType::Integer { min: Some(1), max: Some(10) })
                .executes(|_| Ok(1))));
        let src = make_console_source(Arc::new(MockServer::new()));
        // value 99 exceeds max=10
        assert!(d.parse("setval 99", src).is_err());
    }

    #[test]
    fn permission_requirement_blocks_low_permission_source() {
        let mut d = CommandDispatcher::new();
        d.register(literal("stop")
            .requires(|s: &CommandSourceStack| s.has_permission(4))
            .executes(|_| Ok(1)));
        let mut src = make_console_source(Arc::new(MockServer::new()));
        src.permission_level = 0; // no permission
        // should fail the requirement check
        assert!(d.parse("stop", src).is_err());
    }

    // --- Command tree serialization ---

    #[test]
    fn serialize_tree_root_node_has_zero_flags() {
        let d = CommandDispatcher::<CommandSourceStack>::new();
        let tree = d.serialize_tree();
        // Root node flags: type bits = 0b00
        assert_eq!(tree.nodes[0].flags & 0b11, 0b00);
    }

    #[test]
    fn serialize_tree_literal_node_has_correct_flags() {
        let mut d = CommandDispatcher::new();
        d.register(literal("help").executes(|_| Ok(1)));
        let tree = d.serialize_tree();
        let help_node = tree.nodes.iter().find(|n| n.name.as_deref() == Some("help")).unwrap();
        // Literal node: type bits = 0b01, executable = bit 2
        assert_eq!(help_node.flags & 0b11, 0b01);
        assert!(help_node.flags & 0b100 != 0, "should be executable");
    }

    // --- Argument type ids ---

    #[test]
    fn argument_type_ids_match_minecraft_protocol() {
        assert_eq!(ArgumentType::Bool.brigadier_type_id(), "brigadier:bool");
        assert_eq!(ArgumentType::BlockPos.brigadier_type_id(), "minecraft:block_pos");
        assert_eq!(ArgumentType::Gamemode.brigadier_type_id(), "minecraft:game_mode");
        assert_eq!(ArgumentType::Uuid.brigadier_type_id(), "minecraft:uuid");
    }

    // --- Tab completion ---

    #[test]
    fn completions_returns_registered_command_names_at_root() {
        let mut d = CommandDispatcher::new();
        d.register(literal("help").executes(|_| Ok(1)));
        d.register(literal("stop").executes(|_| Ok(1)));
        let src = make_console_source(Arc::new(MockServer::new()));
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
        let src = make_console_source(Arc::new(MockServer::new()));
        let completions = d.get_completions("g", &src);
        let texts: Vec<_> = completions.iter().map(|s| s.text.as_str()).collect();
        assert!(texts.contains(&"give"), "should include give");
        assert!(texts.contains(&"gamemode"), "should include gamemode");
        assert!(!texts.contains(&"kill"), "should not include kill");
    }
}
```
