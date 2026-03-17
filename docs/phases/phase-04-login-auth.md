# Phase 4 — Login + Encryption + Compression

**Crate:** `oxidized-protocol`  
**Reward:** 🎉 A real vanilla Minecraft 26.1 client can connect and authenticate
(both online and offline mode).

---

## Goal

Implement the full LOGIN protocol state including RSA key exchange, Mojang session
server authentication, AES-128-CFB8 encryption, and zlib compression.

---

## Java Reference

| Concept | Java class |
|---------|-----------|
| Login handler | `net.minecraft.server.network.ServerLoginPacketListenerImpl` |
| Hello packet | `net.minecraft.network.protocol.login.ClientboundHelloPacket` |
| Key packet | `net.minecraft.network.protocol.login.ServerboundKeyPacket` |
| Login finished | `net.minecraft.network.protocol.login.ClientboundLoginFinishedPacket` |
| Compression | `net.minecraft.network.LoginCompressionPacket` |
| Cipher encoder/decoder | `net.minecraft.network.CipherEncoder`, `CipherDecoder` |
| Compression codec | `net.minecraft.network.CompressionEncoder`, `CompressionDecoder` |
| Auth hash | Java `MessageDigest("SHA-1")` with Minecraft's non-standard encoding |

---

## Login Packet Types

### Clientbound

```rust
/// 0x01 — sent to start auth
pub struct ClientboundHelloPacket {
    pub server_id: String,          // always "" in modern versions
    pub public_key: Vec<u8>,        // DER-encoded RSA public key
    pub challenge: Vec<u8>,         // 4 random bytes
    pub should_authenticate: bool,  // online_mode
}

/// 0x02 — login complete (terminal packet → switch to CONFIGURATION)
pub struct ClientboundLoginFinishedPacket {
    pub uuid: Uuid,
    pub username: String,
    pub properties: Vec<ProfileProperty>,
}

/// 0x03 — enable compression
pub struct ClientboundLoginCompressionPacket {
    pub threshold: i32,  // -1 = disable, 0+ = threshold in bytes
}

/// 0x00 — disconnect during login
pub struct ClientboundLoginDisconnectPacket {
    pub reason: Component,
}
```

### Serverbound

```rust
/// 0x00 — client's name and UUID
pub struct ServerboundHelloPacket {
    pub name: String,        // max 16 chars
    pub profile_id: Uuid,    // may be nil UUID for offline
}

/// 0x01 — encrypted shared secret
pub struct ServerboundKeyPacket {
    pub key_bytes: Vec<u8>,             // RSA-encrypted AES secret
    pub encrypted_challenge: Vec<u8>,   // RSA-encrypted challenge bytes
}

/// 0x02 — acknowledge login finished (terminal → CONFIGURATION)
pub struct ServerboundLoginAcknowledgedPacket;
```

---

## Tasks

### 4.1 — RSA Key Generation (`oxidized-protocol/src/auth/rsa.rs`)

```rust
use rsa::{RsaPrivateKey, RsaPublicKey, pkcs8::EncodePublicKey};

pub struct ServerKeyPair {
    private: RsaPrivateKey,
    public_der: Vec<u8>,   // DER-encoded public key
}

impl ServerKeyPair {
    /// Generate a new 1024-bit RSA key pair on startup.
    pub fn generate() -> Result<Self, rsa::Error>;
    
    /// Decrypt data with the private key (PKCS#1 v1.5).
    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, rsa::Error>;
    
    /// Public key in DER format (sent in ClientboundHelloPacket).
    pub fn public_key_der(&self) -> &[u8];
}
```

### 4.2 — Mojang Authentication Hash (`auth/hash.rs`)

Minecraft uses a non-standard SHA-1 hex digest (can be negative):

```rust
pub fn compute_server_id_hash(
    server_id: &str,     // ""
    shared_secret: &[u8],
    public_key_der: &[u8],
) -> String {
    // SHA-1(server_id_bytes + shared_secret + public_key_der)
    let mut hasher = Sha1::new();
    hasher.update(server_id.as_bytes());
    hasher.update(shared_secret);
    hasher.update(public_key_der);
    let digest = hasher.finalize();
    
    // Minecraft's "twos complement hex" — may have leading '-'
    minecraft_hex_digest(&digest)
}

fn minecraft_hex_digest(bytes: &[u8]) -> String {
    // If high bit set, negate and prepend '-'
    // Otherwise, format as lowercase hex, no leading zeros
}
```

### 4.3 — Mojang Session Server Auth (`auth/mojang.rs`)

```rust
pub async fn authenticate(
    username: &str,
    server_hash: &str,
    client_ip: Option<&str>,   // for preventProxyConnections
) -> Result<GameProfile, AuthError> {
    let url = format!(
        "https://sessionserver.mojang.com/session/minecraft/hasJoined\
         ?username={}&serverId={}{}",
        username, server_hash,
        client_ip.map(|ip| format!("&ip={}", ip)).unwrap_or_default()
    );
    let resp = reqwest::get(&url).await?;
    if resp.status() == 204 { return Err(AuthError::NotAuthenticated); }
    let profile: GameProfile = resp.json().await?;
    Ok(profile)
}

pub struct GameProfile {
    pub id: Uuid,
    pub name: String,
    pub properties: Vec<ProfileProperty>,  // skin, cape
}

pub struct ProfileProperty {
    pub name: String,
    pub value: String,      // base64-encoded JSON
    pub signature: Option<String>,
}
```

### 4.4 — AES-128-CFB8 Cipher (`codec/cipher.rs`)

```rust
use aes::Aes128;
use cfb_mode::{Cfb, Encryptor, Decryptor};

pub struct PacketCipher {
    key: [u8; 16],
    iv: [u8; 16],   // same as key (Minecraft quirk)
}

impl PacketCipher {
    pub fn new(shared_secret: &[u8]) -> Self {
        let mut key = [0u8; 16];
        key.copy_from_slice(&shared_secret[..16]);
        PacketCipher { key, iv: key }
    }
    
    pub fn encrypt_in_place(&self, data: &mut [u8]);
    pub fn decrypt_in_place(&self, data: &mut [u8]);
}
```

Encrypt applies to the raw byte stream (applied after framing). Both encoder
and decoder use the same key+IV.

### 4.5 — Zlib Compression (`codec/compress.rs`)

```rust
pub fn compress(data: &[u8], threshold: usize) -> Vec<u8> {
    if data.len() < threshold {
        // [0x00 VarInt (data_length=0)] [raw data]
        let mut out = vec![0u8];
        out.extend_from_slice(data);
        out
    } else {
        // [uncompressed_length VarInt] [deflate(data)]
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data)?;
        let compressed = encoder.finish()?;
        let mut out = encode_varint_bytes(data.len() as i32);
        out.extend(compressed);
        out
    }
}

pub fn decompress(data: &[u8], max_size: usize) -> Result<Vec<u8>, CompressError> {
    let (data_length, offset) = decode_varint(data)?;
    if data_length == 0 {
        return Ok(data[offset..].to_vec());
    }
    if data_length as usize > max_size {
        return Err(CompressError::TooLarge);
    }
    // zlib decompress data[offset..]
}
```

### 4.6 — Login State Handler (`connection.rs` extension)

```rust
async fn handle_login(
    conn: &mut Connection,
    pkt: RawPacket,
    config: &ServerConfig,
    key_pair: &ServerKeyPair,
) -> Result<LoginResult, LoginError> {
    match pkt.id {
        0x00 => {  // ServerboundHelloPacket
            let hello = ServerboundHelloPacket::decode(pkt)?;
            // Store name for auth
            if config.online_mode {
                // Send challenge
                let challenge = rand::random::<[u8; 4]>();
                conn.send(ClientboundHelloPacket {
                    server_id: "".into(),
                    public_key: key_pair.public_key_der().to_vec(),
                    challenge: challenge.to_vec(),
                    should_authenticate: true,
                }).await?;
                conn.login_state = LoginState::AwaitingKey { name: hello.name, challenge };
            } else {
                // Offline mode: derive UUID
                let uuid = offline_uuid(&hello.name);
                conn.finish_login(uuid, hello.name, vec![]).await?;
            }
        }
        0x01 => {  // ServerboundKeyPacket
            // Decrypt shared secret + challenge
            // Verify challenge matches
            // Authenticate with Mojang
            // Enable encryption
            // Send compression
            // Send LoginFinished
        }
        0x02 => {  // ServerboundLoginAcknowledged
            conn.state = ConnectionState::Configuration;
            return Ok(LoginResult::Proceed);
        }
        _ => {}
    }
}
```

### 4.7 — Offline UUID

```rust
/// Derive a v3 UUID from "OfflinePlayer:<name>", matching vanilla behaviour.
pub fn offline_uuid(name: &str) -> Uuid {
    Uuid::new_v3(&Uuid::NAMESPACE_DNS, format!("OfflinePlayer:{}", name).as_bytes())
}
```

---

## Encryption Sequencing

```
1. ServerboundKeyPacket received
2. Server decrypts shared_secret (RSA private key)
3. Server decrypts challenge → verify matches stored challenge
4. [Optional] POST to Mojang auth
5. [Optional] Send ClientboundLoginCompressionPacket
6. *** Enable compression in pipeline ***
7. Send ClientboundLoginFinishedPacket  ← this packet IS compressed
8. *** Enable AES encryption in pipeline ***
9. All subsequent bytes are encrypted
```

Note: encryption is enabled **after** sending LoginFinished in vanilla.
The client enables encryption after receiving LoginFinished.

---

## Tests

```rust
#[test]
fn test_minecraft_hash_positive() {
    // Known vector: "Notch" → "4ed1f46bbe04bc756bcb17c0c7ce3e4632f06a48"
}
#[test]
fn test_minecraft_hash_negative() {
    // Known vector: "jeb_" → "-7c9d5b0044c130109a5d7b5fb5c317c02b4e28c1"
}
#[test]
fn test_offline_uuid() {
    // "Player" → deterministic UUID
}
#[test]
fn test_aes_roundtrip() {
    let key = [0u8; 16];
    let cipher = PacketCipher::new(&key);
    let original = b"hello world packet data";
    let mut data = original.to_vec();
    cipher.encrypt_in_place(&mut data);
    cipher.decrypt_in_place(&mut data);
    assert_eq!(data, original);
}
```
