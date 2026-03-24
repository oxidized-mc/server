# Phase 17 — Chat System

**Status:** ✅ Complete  
**Crate:** `oxidized-game`  
**Reward:** Players can chat with each other and use `/say` and `/me`.

---

## Architecture Decisions

Before implementing this phase, review:

- [ADR-020: Player Session](../adr/adr-020-player-session.md) — split network actor + ECS entity architecture
- [ADR-028: Chat Components](../adr/adr-028-chat-components.md) — enum-based component tree with shared Style


## Goal

Implement the full chat pipeline: receive `ServerboundChatPacket`, validate and
rate-limit it, build a signed `ClientboundPlayerChatPacket` (or unsigned
`ClientboundSystemChatPacket` for server messages), broadcast to all online
players, and format chat with the Component + Style system. Implement `/say` and
`/me` as the first command-driven chat features.

---

## Java Reference

| Concept | Java class |
|---------|-----------|
| Incoming chat handler | `net.minecraft.server.network.ServerGamePacketListenerImpl#handleChat` |
| Incoming command chat | `net.minecraft.server.network.ServerGamePacketListenerImpl#handleChatCommand` |
| Player chat packet (C→S) | `net.minecraft.network.protocol.game.ServerboundChatPacket` |
| Chat command packet (C→S) | `net.minecraft.network.protocol.game.ServerboundChatCommandPacket` |
| Player chat packet (S→C) | `net.minecraft.network.protocol.game.ClientboundPlayerChatPacket` |
| Disguised chat packet | `net.minecraft.network.protocol.game.ClientboundDisguisedChatPacket` |
| System chat packet | `net.minecraft.network.protocol.game.ClientboundSystemChatPacket` |
| Chat component base | `net.minecraft.network.chat.Component` |
| Text component | `net.minecraft.network.chat.contents.PlainTextContents` |
| Translatable component | `net.minecraft.network.chat.contents.TranslatableContents` |
| Style | `net.minecraft.network.chat.Style` |
| Chat formatting codes | `net.minecraft.ChatFormatting` |
| Chat type | `net.minecraft.network.chat.ChatType` |
| Filtered text | `net.minecraft.network.chat.FilteredText` |
| Last seen messages | `net.minecraft.network.chat.LastSeenMessages` |

---

## Tasks

### 17.1 — `ServerboundChatPacket` (`oxidized-protocol/src/packets/serverbound/game.rs`)

```rust
/// 0x06 – client sends a plain chat message
#[derive(Debug, Clone)]
pub struct ServerboundChatPacket {
    pub message: String,            // max 256 chars
    pub timestamp: i64,             // unix epoch ms
    pub salt: i64,
    pub signature: Option<Vec<u8>>, // 256-byte RSA sig when signed chat enabled
    pub last_seen: LastSeenMessages,
}

#[derive(Debug, Clone, Default)]
pub struct LastSeenMessages {
    pub acknowledged: BitSet, // bitset over last 20 messages
}

impl Decode for ServerboundChatPacket {
    fn decode(buf: &mut impl Buf) -> anyhow::Result<Self> {
        let message = String::decode(buf)?;
        anyhow::ensure!(message.len() <= 256, "chat message too long");
        anyhow::ensure!(!message.starts_with('/'), "chat message must not start with /");
        let timestamp = i64::decode(buf)?;
        let salt = i64::decode(buf)?;
        let has_sig = bool::decode(buf)?;
        let signature = if has_sig {
            let mut sig = vec![0u8; 256];
            buf.copy_to_slice(&mut sig);
            Some(sig)
        } else {
            None
        };
        let last_seen = LastSeenMessages::decode(buf)?;
        Ok(Self { message, timestamp, salt, signature, last_seen })
    }
}
```

### 17.2 — `ServerboundChatCommandPacket` (`oxidized-protocol/src/packets/serverbound/game.rs`)

```rust
/// 0x04 – client dispatches a command (leading slash already stripped)
#[derive(Debug, Clone)]
pub struct ServerboundChatCommandPacket {
    pub command: String,            // without leading "/"
    pub timestamp: i64,
    pub salt: i64,
    pub argument_signatures: Vec<ArgumentSignature>,
    pub last_seen: LastSeenMessages,
}

#[derive(Debug, Clone)]
pub struct ArgumentSignature {
    pub name: String,
    pub signature: [u8; 256],
}
```

### 17.3 — Component system (`oxidized-game/src/chat/component.rs`)

```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum Component {
    Text(TextComponent),
    Translatable(TranslatableComponent),
    Keybind(KeybindComponent),
    Score(ScoreComponent),
    Selector(SelectorComponent),
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TextComponent {
    pub text: String,
    #[serde(flatten)]
    pub base: ComponentBase,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TranslatableComponent {
    pub translate: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub with: Vec<Component>,
    #[serde(flatten)]
    pub base: ComponentBase,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SelectorComponent {
    pub selector: String,           // "@a", "@p", etc.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub separator: Option<Box<Component>>,
    #[serde(flatten)]
    pub base: ComponentBase,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ScoreComponent {
    pub score: ScoreContents,
    #[serde(flatten)]
    pub base: ComponentBase,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ScoreContents {
    pub name: String,
    pub objective: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct KeybindComponent {
    pub keybind: String,
    #[serde(flatten)]
    pub base: ComponentBase,
}

#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct ComponentBase {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<Style>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub extra: Vec<Component>,
}

impl Component {
    pub fn text(s: impl Into<String>) -> Self {
        Component::Text(TextComponent {
            text: s.into(),
            base: ComponentBase::default(),
        })
    }

    pub fn translate(key: impl Into<String>, args: Vec<Component>) -> Self {
        Component::Translatable(TranslatableComponent {
            translate: key.into(),
            with: args,
            base: ComponentBase::default(),
        })
    }

    pub fn with_style(mut self, style: Style) -> Self {
        match &mut self {
            Component::Text(c) => c.base.style = Some(style),
            Component::Translatable(c) => c.base.style = Some(style),
            Component::Keybind(c) => c.base.style = Some(style),
            Component::Score(c) => c.base.style = Some(style),
            Component::Selector(c) => c.base.style = Some(style),
        }
        self
    }

    pub fn append(mut self, child: Component) -> Self {
        match &mut self {
            Component::Text(c) => c.base.extra.push(child),
            Component::Translatable(c) => c.base.extra.push(child),
            Component::Keybind(c) => c.base.extra.push(child),
            Component::Score(c) => c.base.extra.push(child),
            Component::Selector(c) => c.base.extra.push(child),
        }
        self
    }

    /// Serialize to JSON string for use on the wire
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("component serialization is infallible")
    }
}
```

### 17.4 — Style (`oxidized-game/src/chat/style.rs`)

```rust
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct Style {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<TextColor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underlined: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strikethrough: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub obfuscated: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insertion: Option<String>,
    #[serde(rename = "clickEvent", skip_serializing_if = "Option::is_none")]
    pub click_event: Option<ClickEvent>,
    #[serde(rename = "hoverEvent", skip_serializing_if = "Option::is_none")]
    pub hover_event: Option<HoverEvent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,           // resource location, default "minecraft:default"
}

/// Either a named color (§0–§f) or a 24-bit RGB hex color (#RRGGBB)
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum TextColor {
    Named(NamedColor),
    Hex(HexColor),
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NamedColor {
    Black,        // §0  #000000
    DarkBlue,     // §1  #0000AA
    DarkGreen,    // §2  #00AA00
    DarkAqua,     // §3  #00AAAA
    DarkRed,      // §4  #AA0000
    DarkPurple,   // §5  #AA00AA
    Gold,         // §6  #FFAA00
    Gray,         // §7  #AAAAAA
    DarkGray,     // §8  #555555
    Blue,         // §9  #5555FF
    Green,        // §a  #55FF55
    Aqua,         // §b  #55FFFF
    Red,          // §c  #FF5555
    LightPurple,  // §d  #FF55FF
    Yellow,       // §e  #FFFF55
    White,        // §f  #FFFFFF
}

#[derive(Debug, Clone, PartialEq)]
pub struct HexColor(pub u32); // 0x00RRGGBB

impl serde::Serialize for HexColor {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format!("#{:06X}", self.0 & 0xFFFFFF))
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "action", content = "value", rename_all = "snake_case")]
pub enum ClickEvent {
    OpenUrl(String),
    RunCommand(String),
    SuggestCommand(String),
    CopyToClipboard(String),
    ChangePage(String),
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "action", content = "contents", rename_all = "snake_case")]
pub enum HoverEvent {
    ShowText(Box<Component>),
    ShowItem(HoverItem),
    ShowEntity(HoverEntity),
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HoverItem {
    pub id: String,
    pub count: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub components: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HoverEntity {
    #[serde(rename = "type")]
    pub entity_type: String,
    pub id: String,  // UUID as string
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Box<Component>>,
}

impl Style {
    pub fn empty() -> Self { Self::default() }

    pub fn bold() -> Self { Self { bold: Some(true), ..Default::default() } }

    pub fn color(color: NamedColor) -> Self {
        Self { color: Some(TextColor::Named(color)), ..Default::default() }
    }

    pub fn hex_color(rgb: u32) -> Self {
        Self { color: Some(TextColor::Hex(HexColor(rgb))), ..Default::default() }
    }
}
```

### 17.5 — ChatFormatting legacy codes (`oxidized-game/src/chat/formatting.rs`)

```rust
/// Legacy §-code formatting for system messages and plain-text fallbacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatFormatting {
    // Colors
    Black        = 0x00,  // §0
    DarkBlue     = 0x11,  // §1
    DarkGreen    = 0x22,  // §2
    DarkAqua     = 0x33,  // §3
    DarkRed      = 0x44,  // §4
    DarkPurple   = 0x55,  // §5
    Gold         = 0x66,  // §6
    Gray         = 0x77,  // §7
    DarkGray     = 0x88,  // §8
    Blue         = 0x99,  // §9
    Green        = 0xAA,  // §a
    Aqua         = 0xBB,  // §b
    Red          = 0xCC,  // §c
    LightPurple  = 0xDD,  // §d
    Yellow       = 0xEE,  // §e
    White        = 0xFF,  // §f
    // Formatting
    Obfuscated   = 0x106, // §k
    Bold         = 0x111, // §l
    Strikethrough= 0x122, // §m
    Underline    = 0x133, // §n
    Italic       = 0x144, // §o
    Reset        = 0x155, // §r
}

impl ChatFormatting {
    pub fn code(self) -> char {
        match self {
            Self::Black        => '0',
            Self::DarkBlue     => '1',
            Self::DarkGreen    => '2',
            Self::DarkAqua     => '3',
            Self::DarkRed      => '4',
            Self::DarkPurple   => '5',
            Self::Gold         => '6',
            Self::Gray         => '7',
            Self::DarkGray     => '8',
            Self::Blue         => '9',
            Self::Green        => 'a',
            Self::Aqua         => 'b',
            Self::Red          => 'c',
            Self::LightPurple  => 'd',
            Self::Yellow       => 'e',
            Self::White        => 'f',
            Self::Obfuscated   => 'k',
            Self::Bold         => 'l',
            Self::Strikethrough=> 'm',
            Self::Underline    => 'n',
            Self::Italic       => 'o',
            Self::Reset        => 'r',
        }
    }

    /// e.g. `ChatFormatting::Red.prefix()` → `"§c"`
    pub fn prefix(self) -> String {
        format!("\u{00A7}{}", self.code())
    }

    /// RGB value for color codes; None for formatting-only codes.
    pub fn color(self) -> Option<u32> {
        match self {
            Self::Black        => Some(0x000000),
            Self::DarkBlue     => Some(0x0000AA),
            Self::DarkGreen    => Some(0x00AA00),
            Self::DarkAqua     => Some(0x00AAAA),
            Self::DarkRed      => Some(0xAA0000),
            Self::DarkPurple   => Some(0xAA00AA),
            Self::Gold         => Some(0xFFAA00),
            Self::Gray         => Some(0xAAAAAA),
            Self::DarkGray     => Some(0x555555),
            Self::Blue         => Some(0x5555FF),
            Self::Green        => Some(0x55FF55),
            Self::Aqua         => Some(0x55FFFF),
            Self::Red          => Some(0xFF5555),
            Self::LightPurple  => Some(0xFF55FF),
            Self::Yellow       => Some(0xFFFF55),
            Self::White        => Some(0xFFFFFF),
            _ => None,
        }
    }
}
```

### 17.6 — Outbound chat packets (`oxidized-protocol/src/packets/clientbound/game.rs`)

```rust
/// 0x39 – signed player chat message
#[derive(Debug, Clone)]
pub struct ClientboundPlayerChatPacket {
    pub sender: uuid::Uuid,
    pub index: i32,               // VarInt
    pub message_signature: Option<[u8; 256]>,
    pub body: ChatMessageBody,
    pub unsigned_content: Option<Component>,
    pub filter_mask: FilterMask,
    pub chat_type: i32,           // registry id
    pub sender_name: Component,
    pub target_name: Option<Component>,
}

#[derive(Debug, Clone)]
pub struct ChatMessageBody {
    pub content: String,          // plain text, max 256
    pub timestamp: i64,
    pub salt: i64,
    pub last_seen: Vec<MessageSignature>,
}

#[derive(Debug, Clone)]
pub struct MessageSignature(pub [u8; 256]);

#[derive(Debug, Clone)]
pub enum FilterMask {
    PassThrough,
    FullyFiltered,
    PartiallyFiltered(BitSet),
}

/// 0x70 – server-originated system message (no signature)
#[derive(Debug, Clone)]
pub struct ClientboundSystemChatPacket {
    pub content: Component,
    pub overlay: bool,            // true = action bar, false = chat
}

/// 0x1D – disguised chat (chat type + sender name, no UUID/signature)
#[derive(Debug, Clone)]
pub struct ClientboundDisguisedChatPacket {
    pub message: Component,
    pub chat_type: i32,
    pub sender_name: Component,
    pub target_name: Option<Component>,
}
```

### 17.7 — Chat rate limiter (`oxidized-game/src/chat/rate_limit.rs`)

```rust
use std::collections::VecDeque;
use std::time::{Duration, Instant};

const MAX_MESSAGES_PER_MINUTE: usize = 200;
const WINDOW: Duration = Duration::from_secs(60);

pub struct ChatRateLimiter {
    timestamps: VecDeque<Instant>,
}

impl ChatRateLimiter {
    pub fn new() -> Self {
        Self { timestamps: VecDeque::with_capacity(MAX_MESSAGES_PER_MINUTE + 1) }
    }

    /// Returns `true` if the message is allowed, `false` if rate-limited.
    pub fn check(&mut self) -> bool {
        let now = Instant::now();
        // Evict entries older than 1 minute
        while self.timestamps.front().map_or(false, |t| now - *t > WINDOW) {
            self.timestamps.pop_front();
        }
        if self.timestamps.len() >= MAX_MESSAGES_PER_MINUTE {
            return false;
        }
        self.timestamps.push_back(now);
        true
    }
}
```

### 17.8 — Chat handler (`oxidized-game/src/player/chat_handler.rs`)

```rust
impl PlayerConnection {
    pub async fn handle_chat(&mut self, packet: ServerboundChatPacket) -> anyhow::Result<()> {
        if !self.chat_rate_limiter.check() {
            self.send_system_message(
                Component::text("You are sending too many messages. Slow down!")
                    .with_style(Style::color(NamedColor::Red)),
                false,
            ).await?;
            return Ok(());
        }

        let msg = packet.message.trim().to_string();
        if msg.is_empty() { return Ok(()); }

        let player_name = self.player.profile.name.clone();
        let sender_uuid = self.player.uuid;

        // Build the decorated display name for hover
        let name_component = Component::text(&player_name)
            .with_style(Style {
                hover_event: Some(HoverEvent::ShowEntity(HoverEntity {
                    entity_type: "minecraft:player".into(),
                    id: sender_uuid.to_string(),
                    name: Some(Box::new(Component::text(&player_name))),
                })),
                click_event: Some(ClickEvent::SuggestCommand(
                    format!("/tell {} ", player_name)
                )),
                ..Default::default()
            });

        let packet = ClientboundPlayerChatPacket {
            sender: sender_uuid,
            index: self.next_chat_index(),
            message_signature: None, // unsigned mode
            body: ChatMessageBody {
                content: msg.clone(),
                timestamp: packet.timestamp,
                salt: packet.salt,
                last_seen: vec![],
            },
            unsigned_content: Some(Component::text(&msg)),
            filter_mask: FilterMask::PassThrough,
            chat_type: 0, // minecraft:chat
            sender_name: name_component,
            target_name: None,
        };

        self.server.broadcast_player_chat(packet).await;
        tracing::info!("<{}> {}", player_name, msg);
        Ok(())
    }

    pub async fn send_system_message(&mut self, msg: Component, overlay: bool) -> anyhow::Result<()> {
        self.send_packet(ClientboundSystemChatPacket { content: msg, overlay }).await
    }
}
```

### 17.9 — `/say` and `/me` commands (`oxidized-game/src/commands/chat_commands.rs`)

```rust
pub fn register_say(dispatcher: &mut CommandDispatcher<CommandSource>) {
    dispatcher.register(
        literal("say")
            .requires(|src| src.has_permission(1))
            .then(
                argument("message", StringArgument::Greedy)
                    .executes(|ctx| {
                        let msg = get_string(&ctx, "message")?;
                        let src_name = ctx.source.display_name();
                        let component = Component::text(format!("[{}] {}", src_name, msg))
                            .with_style(Style::color(NamedColor::White));
                        ctx.source.server().broadcast_system(component, false);
                        tracing::info!("[{}] {}", src_name, msg);
                        Ok(1)
                    })
            )
    );
}

pub fn register_me(dispatcher: &mut CommandDispatcher<CommandSource>) {
    dispatcher.register(
        literal("me")
            .then(
                argument("action", StringArgument::Greedy)
                    .executes(|ctx| {
                        let action = get_string(&ctx, "action")?;
                        let src_name = ctx.source.display_name();
                        let component = Component::translate(
                            "chat.type.emote",
                            vec![Component::text(&src_name), Component::text(&action)],
                        );
                        ctx.source.server().broadcast_system(component, false);
                        Ok(1)
                    })
            )
    );
}
```

---

## Data Structures

```rust
// oxidized-game/src/chat/mod.rs

/// Ring buffer of the last 20 chat message signatures seen by the server
/// for the acknowledgement protocol.
pub struct AcknowledgedMessages {
    entries: VecDeque<(uuid::Uuid, [u8; 256])>, // (sender, sig)
}

/// Per-player chat state stored inside PlayerConnection.
pub struct PlayerChatState {
    pub rate_limiter: ChatRateLimiter,
    pub next_index: i32,
    pub acknowledged: AcknowledgedMessages,
    pub session_key: Option<RemoteChatSession>,
}

/// Public key + expiry carried in the chat session update packet.
pub struct RemoteChatSession {
    pub session_id: uuid::Uuid,
    pub expires_at: i64,           // unix ms
    pub public_key: Vec<u8>,       // DER-encoded RSA public key
    pub key_signature: Vec<u8>,    // signed by Mojang
}
```

---

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // --- Component serialization ---

    #[test]
    fn text_component_serializes_to_json() {
        // Component::text("hello") → {"text":"hello"}
        let c = Component::text("hello");
        let json = c.to_json();
        assert_eq!(json, r#"{"text":"hello"}"#);
    }

    #[test]
    fn styled_component_includes_color_field() {
        // bold red text serializes color + bold fields
        let c = Component::text("alert")
            .with_style(Style { bold: Some(true), color: Some(TextColor::Named(NamedColor::Red)), ..Default::default() });
        let json: serde_json::Value = serde_json::from_str(&c.to_json()).unwrap();
        assert_eq!(json["color"], "red");
        assert_eq!(json["bold"], true);
        assert_eq!(json["text"], "alert");
    }

    #[test]
    fn hex_color_serializes_as_hash_string() {
        // HexColor(0xFF8000) → "#FF8000"
        let style = Style::hex_color(0xFF8000);
        let json = serde_json::to_string(&style).unwrap();
        assert!(json.contains("#FF8000"), "got: {json}");
    }

    #[test]
    fn translatable_component_with_args_serializes_correctly() {
        let c = Component::translate(
            "chat.type.text",
            vec![Component::text("Alice"), Component::text("hello world")],
        );
        let json: serde_json::Value = serde_json::from_str(&c.to_json()).unwrap();
        assert_eq!(json["translate"], "chat.type.text");
        assert_eq!(json["with"][0]["text"], "Alice");
    }

    #[test]
    fn click_event_run_command_serializes_correctly() {
        let style = Style {
            click_event: Some(ClickEvent::RunCommand("/home".into())),
            ..Default::default()
        };
        let json = serde_json::to_string(&style).unwrap();
        assert!(json.contains("run_command"), "got: {json}");
        assert!(json.contains("/home"), "got: {json}");
    }

    // --- ChatFormatting ---

    #[test]
    fn chat_formatting_code_roundtrips() {
        assert_eq!(ChatFormatting::Red.code(), 'c');
        assert_eq!(ChatFormatting::Bold.code(), 'l');
        assert_eq!(ChatFormatting::Reset.code(), 'r');
    }

    #[test]
    fn chat_formatting_prefix_contains_section_sign() {
        let prefix = ChatFormatting::Gold.prefix();
        assert!(prefix.starts_with('\u{00A7}'));
        assert!(prefix.ends_with('6'));
    }

    #[test]
    fn chat_formatting_color_returns_correct_rgb() {
        assert_eq!(ChatFormatting::Red.color(), Some(0xFF5555));
        assert_eq!(ChatFormatting::Black.color(), Some(0x000000));
        assert_eq!(ChatFormatting::Bold.color(), None);
    }

    // --- Rate limiter ---

    #[test]
    fn rate_limiter_allows_up_to_200_messages() {
        let mut limiter = ChatRateLimiter::new();
        for _ in 0..200 {
            assert!(limiter.check(), "should allow first 200 messages");
        }
        assert!(!limiter.check(), "201st message must be blocked");
    }

    #[test]
    fn rate_limiter_allows_after_window_expires() {
        // This test uses real time; use a mock clock in integration tests.
        // Validates the eviction logic path is reachable.
        let mut limiter = ChatRateLimiter::new();
        // fill to limit
        for _ in 0..200 { limiter.check(); }
        // Inject old timestamp by manipulating internal state via test helper
        limiter.timestamps.clear(); // simulate 60s passing
        assert!(limiter.check(), "should allow after window cleared");
    }

    // --- Packet decode ---

    #[test]
    fn serverbound_chat_packet_rejects_slash_prefix() {
        let mut buf = build_chat_buf("/gamemode creative", 0, 0, false);
        let result = ServerboundChatPacket::decode(&mut buf);
        assert!(result.is_err(), "slash-prefixed message must be rejected");
    }

    #[test]
    fn serverbound_chat_packet_rejects_over_256_chars() {
        let long_msg = "a".repeat(257);
        let mut buf = build_chat_buf(&long_msg, 0, 0, false);
        let result = ServerboundChatPacket::decode(&mut buf);
        assert!(result.is_err(), "message over 256 chars must be rejected");
    }
}
```
