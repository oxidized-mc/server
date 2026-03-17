# ADR-028: Chat & Text Component System

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-17 |
| Phases | P17 |
| Deciders | Oxidized Core Team |

## Context

Minecraft's text component system is the universal abstraction for styled, interactive,
translatable text. It powers chat messages, item names and lore, sign text, book pages, boss
bar titles, action bar messages, tab list headers/footers, title/subtitle overlays, death
messages, disconnect reasons, and scoreboard display names. A component is a tree of styled
text nodes that the client renders with inheritance-based style resolution. The wire protocol
serializes components as JSON (or in some packets, NBT-encoded JSON) — this format is
non-negotiable because the vanilla client parses it directly.

Vanilla's Java implementation uses a `Component` interface with `MutableComponent` as the
primary implementation, backed by a `ComponentContents` sealed interface hierarchy:
`LiteralContents` (plain text), `TranslatableContents` (localized text with argument
substitution), `SelectorContents` (entity selector like `@a`), `ScoreContents` (scoreboard
value), `KeybindContents` (client keybinding like `key.jump`), and `NbtContents` (NBT path
value from entity/block/storage). Each component carries an optional `Style` (color, bold,
italic, underline, strikethrough, obfuscated, font, insertion text, click event, hover event)
and a list of child components. Style properties use a tri-state: set-true, set-false, or
inherit-from-parent.

The component system is one of the most frequently used subsystems on the server. Every chat
message, every item tooltip, every sign placement, every scoreboard update, and every packet
containing display text must construct and serialize components. The builder API must be
ergonomic (developers write component code constantly) and serialization must be fast (it
happens on every chat message for every online player).

## Decision Drivers

- **Wire protocol fidelity**: JSON serialization must be byte-for-byte compatible with what
  the vanilla client expects. The client's JSON parser is the authority.
- **Ergonomic API**: Building components should be pleasant — chain-style builders, not
  multi-line struct construction for simple "Hello, world!" messages.
- **Performance**: Component serialization to JSON is a hot path (every chat message is
  serialized per-player for personalized content like selectors/scores).
- **Style inheritance**: Parent styles must propagate to children correctly — a red bold
  parent with a child that only sets italic should render red+bold+italic.
- **Legacy support**: The `§` (section sign) formatting system is still used in many contexts
  (MOTD, item names from legacy data). We must convert between §-codes and components.
- **Translatable support**: Server-side translation for `TranslatableContents` with fallback
  to the translation key when the key is unknown.

## Considered Options

### Option 1: Enum-Based Component Tree

A single `Component` enum with one variant per content type, plus children and style. Simple,
matches the data model directly.

**Pros**: Exhaustive matching, stack-allocated for simple components, straightforward serde
implementation. Pattern matches at every usage site ensure all variants are handled.

**Cons**: The enum itself is moderately large (largest variant drives size). Children require
`Vec<Component>`, so the tree is partially heap-allocated regardless. Adding the builder
pattern to an enum requires wrapper methods.

### Option 2: Trait-Based Polymorphism

A `Component` trait with `TextComponent`, `TranslatableComponent`, etc. implementing it.
Components stored as `Box<dyn Component>`.

**Pros**: Extensible, familiar OOP pattern, each component type is self-contained.

**Cons**: Virtual dispatch on every operation. `Box<dyn Component>` is heap-allocated and
scattered. Serialization requires `typetag` or manual registry. Builder API is awkward
(can't chain across types). Cloning requires `dyn Clone` hacks.

### Option 3: Flat String with Embedded §-Codes

Represent all text as strings with `§` format codes. Internally, only deal with strings.

**Pros**: Minimal memory, trivial construction, fast concatenation.

**Cons**: Cannot represent click events, hover events, translations, selectors, scores, NBT
paths, hex colors, or fonts. Only supports the 16 legacy colors and 5 formatting codes.
Fundamentally insufficient for modern Minecraft. Not viable.

### Option 4: String Interning with Style Refs

Intern all unique text strings and styles into pools. Components reference interned IDs.

**Pros**: Deduplication of repeated strings (e.g., color names, common phrases). Compact
representation for repeated components.

**Cons**: Interning adds complexity (pool management, thread safety). Most components are
unique (chat messages), so deduplication has minimal benefit. Lookup overhead may negate
memory savings. Premature optimization.

## Decision

**Enum-based component tree with shared Style.** A `Component` is a struct containing content
(an enum), an optional style, and a vector of children. This is the simplest design that
correctly models the data and provides a clean builder API.

### Core Types

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct Component {
    content: ComponentContent,
    style: Style,
    children: Vec<Component>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ComponentContent {
    Text(String),
    Translatable {
        key: String,
        fallback: Option<String>,
        args: Vec<Component>,
    },
    Selector {
        pattern: String,           // e.g., "@a[distance=..5]"
        separator: Option<Box<Component>>,
    },
    Score {
        name: String,              // entity selector or player name
        objective: String,
    },
    Keybind(String),               // e.g., "key.jump", "key.inventory"
    Nbt {
        path: String,
        interpret: bool,
        separator: Option<Box<Component>>,
        source: NbtSource,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum NbtSource {
    Entity(String),                // entity selector
    Block(String),                 // block position "~0 ~1 ~0"
    Storage(ResourceLocation),     // storage namespace:key
}
```

### Style System

Style uses `Option<T>` for every field. `None` means "inherit from parent". The client
resolves inheritance by walking up the component tree. We match this behavior on the server
side for any server-resolved content (e.g., translatable components, selector resolution).

```rust
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Style {
    pub color: Option<TextColor>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underlined: Option<bool>,
    pub strikethrough: Option<bool>,
    pub obfuscated: Option<bool>,
    pub font: Option<ResourceLocation>,
    pub insertion: Option<String>,
    pub click_event: Option<ClickEvent>,
    pub hover_event: Option<HoverEvent>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TextColor {
    Named(ChatFormatting),         // §0-§f: 16 named colors
    Hex(u32),                      // #RRGGBB as 24-bit integer
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatFormatting {
    Black,         // §0, #000000
    DarkBlue,      // §1, #0000AA
    DarkGreen,     // §2, #00AA00
    DarkAqua,      // §3, #00AAAA
    DarkRed,       // §4, #AA0000
    DarkPurple,    // §5, #AA00AA
    Gold,          // §6, #FFAA00
    Gray,          // §7, #AAAAAA
    DarkGray,      // §8, #555555
    Blue,          // §9, #5555FF
    Green,         // §a, #55FF55
    Aqua,          // §b, #55FFFF
    Red,           // §c, #FF5555
    LightPurple,   // §d, #FF55FF
    Yellow,        // §e, #FFFF55
    White,         // §f, #FFFFFF
}
```

### Click and Hover Events

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum ClickEvent {
    OpenUrl(String),
    RunCommand(String),
    SuggestCommand(String),
    CopyToClipboard(String),
    ChangePage(i32),
}

#[derive(Debug, Clone, PartialEq)]
pub enum HoverEvent {
    ShowText(Box<Component>),
    ShowItem {
        id: ResourceLocation,
        count: i32,
        components: Option<String>,  // SNBT-encoded DataComponentPatch
    },
    ShowEntity {
        entity_type: ResourceLocation,
        id: Uuid,
        name: Option<Box<Component>>,
    },
}
```

### Builder API

Construction is chainable and ergonomic:

```rust
// Simple text
let msg = Component::text("Hello, world!");

// Styled text
let msg = Component::text("Warning!")
    .bold()
    .color(TextColor::Named(ChatFormatting::Red));

// Nested components
let msg = Component::text("Player ")
    .append(Component::text("Steve").color(TextColor::Named(ChatFormatting::Gold)).bold())
    .append(Component::text(" joined the game"))
    .color(TextColor::Named(ChatFormatting::Yellow));

// Interactive
let msg = Component::text("[Click me]")
    .color(TextColor::Named(ChatFormatting::Aqua))
    .underlined()
    .click(ClickEvent::RunCommand("/help".into()))
    .hover(HoverEvent::ShowText(Box::new(Component::text("Click for help"))));

// Translatable with arguments
let msg = Component::translatable(
    "death.attack.player",
    vec![victim_name.clone(), killer_name.clone()],
);

// Hex color (1.16+)
let msg = Component::text("Custom color!")
    .color(TextColor::Hex(0xFF6B35));

// Keybind (client resolves to actual key)
let msg = Component::text("Press ")
    .append(Component::keybind("key.jump"))
    .append(Component::text(" to jump"));
```

### JSON Serialization

Serialization must produce JSON that the vanilla client accepts. The format is well-defined:

```json
{
    "text": "Hello ",
    "bold": true,
    "color": "red",
    "extra": [
        {
            "text": "world",
            "color": "#FF6B35",
            "clickEvent": {
                "action": "run_command",
                "value": "/help"
            },
            "hoverEvent": {
                "action": "show_text",
                "contents": { "text": "Click me!" }
            }
        }
    ]
}
```

Key serialization rules:
- Only include fields that are explicitly set (not `None` / inherited).
- `"text"` for `Text`, `"translate"` for `Translatable`, `"selector"` for `Selector`, etc.
- Children go in `"extra"` array.
- Colors: named colors as lowercase strings (`"red"`, `"dark_blue"`), hex as `"#RRGGBB"`.
- Boolean style fields only included when `Some` — `"bold": true` or `"bold": false`.
- Click/hover events use camelCase keys (`clickEvent`, `hoverEvent`).

We implement `serde::Serialize` and `serde::Deserialize` manually (not derive) to match this
exact format, since the JSON structure varies by content type (it's an externally tagged
union where the tag key changes: `"text"`, `"translate"`, `"selector"`, etc.).

### Legacy §-Code Conversion

For compatibility with legacy text (MOTDs, old plugin data), we provide bidirectional
conversion:

```rust
// §-code string → Component
Component::from_legacy("§cRed §lBold§r Normal")
// Produces: Component tree with Red "Red " + Red+Bold "Bold" + Reset "Normal"

// Component → §-code string (lossy — drops click/hover/translate)
component.to_legacy_string()
```

§-code mapping: `§0`–`§9` and `§a`–`§f` map to the 16 `ChatFormatting` colors (Black through
White). `§k` Obfuscated, `§l` Bold, `§m` Strikethrough, `§n` Underline, `§o` Italic, `§r`
Reset all formatting.

### Translation Support

`TranslatableContents` components reference translation keys (e.g., `"death.attack.player"`,
`"chat.type.text"`, `"commands.help.failed"`). The server holds a translation table loaded
from `en_us.json` (or operator-configured locale). When the server needs to resolve a
translatable component (for logging, RCON output, or non-vanilla protocol uses), it
substitutes arguments into the format string. For client-bound packets, translatable
components are sent as-is — the client resolves them using its own locale.

Argument substitution follows Java's `String.format` conventions used by vanilla:
`%s` for string, `%1$s` for positional, with component arguments interpolated into the tree.

## Consequences

### Positive

- **Simple mental model**: A component is content + style + children. The enum clearly lists
  all content types. Developers can pattern-match exhaustively.
- **Fast serialization**: Enum-based serialization avoids virtual dispatch. Simple text
  components (the common case) serialize with a single `serde_json::to_string`.
- **Ergonomic builder**: The fluent API makes component construction readable and concise.
  Most common cases (text with color) are one-liners.
- **Correct wire format**: Manual serde implementation ensures JSON matches vanilla exactly,
  avoiding surprises from derive macros.
- **Style inheritance**: Using `Option<T>` naturally models the "inherit or override"
  semantics without special sentinel values.

### Negative

- **Manual serde**: Custom Serialize/Deserialize implementations are more code to maintain
  than `#[derive(Serialize, Deserialize)]`. Each new field or variant requires updating both
  implementations.
- **String allocation**: Text content is owned `String`, not `&str`. For frequently
  constructed components (e.g., scoreboard updates every tick), this creates allocation
  pressure. A future optimization could use `Cow<'static, str>` for known-static strings.
- **No server-side rendering cache**: If the same component is serialized for 100 players,
  it's serialized 100 times. A caching layer (serialize once, send bytes to all) could be
  added later for broadcast messages.

### Neutral

- The component system is foundational — nearly every other subsystem depends on it. Changes
  to the Component type ripple across the codebase, which is expected for such a core type.
- The JSON format is client-defined and cannot be optimized without client changes. We accept
  JSON serialization overhead as a wire protocol constraint.

## Compliance

- [ ] Vanilla client renders all component types correctly (text, translatable, selector,
  score, keybind, nbt) when serialized by our implementation.
- [ ] Style inheritance works correctly: child with `color: None` inherits parent color.
- [ ] Click events (open_url, run_command, suggest_command, copy_to_clipboard) trigger
  correct client behavior.
- [ ] Hover events (show_text, show_item, show_entity) display correct tooltips.
- [ ] Hex colors (#RRGGBB) render correctly on 1.16+ clients.
- [ ] Legacy §-code conversion round-trips: `from_legacy(s).to_legacy_string() ≈ s` for
  simple cases.
- [ ] Translation substitution produces correct output for `death.attack.player` with two
  component arguments.
- [ ] Benchmark: simple text component JSON serialization < 200ns.

## Related ADRs

- **ADR-003**: Packet Codec Architecture (components are serialized in many packets)
- **ADR-012**: NBT & Data Codec (NbtContents reads from NBT paths)
- **ADR-017**: Player Session & Authentication (chat signing uses components)
- **ADR-027**: Recipe System (recipe toast uses text components)

## References

- [Minecraft Wiki — Raw JSON Text Format](https://minecraft.wiki/w/Raw_JSON_text_format)
- [Minecraft Wiki — Formatting Codes](https://minecraft.wiki/w/Formatting_codes)
- [wiki.vg — Chat](https://wiki.vg/Chat)
- [`serde_json` custom serialization](https://serde.rs/impl-serialize.html)
- [Unicode § (section sign) U+00A7](https://www.compart.com/en/unicode/U+00A7)
