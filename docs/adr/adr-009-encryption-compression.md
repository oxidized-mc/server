# ADR-009: Encryption & Compression Pipeline

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-17 |
| Phases | P04 |
| Deciders | Oxidized Core Team |

## Context

After the initial handshake and login sequence, a Minecraft connection undergoes two critical transformations: encryption and compression. During the Login state, the server sends an `EncryptionRequest` with its RSA public key and a verify token. The client responds with the shared secret encrypted with the server's public key. Both sides then enable AES-128-CFB8 encryption on the raw byte stream — every byte sent and received is encrypted from that point forward. Later, the server sends a `SetCompression` packet specifying a compression threshold. Packets whose encoded size exceeds this threshold are zlib-compressed before being framed.

Vanilla Java implements these as Netty pipeline handlers inserted dynamically. The `CipherEncoder`/`CipherDecoder` wraps the stream, and the `CompressionEncoder`/`CompressionDecoder` operates on individual frames. The pipeline order is: raw bytes ↔ cipher ↔ frame decoder ↔ decompression ↔ packet decoder. This ordering is critical — encryption operates on the raw byte stream (including frame headers), while compression operates on individual packet payloads within frames.

Getting this wrong means clients cannot connect. The encryption must use AES-128-CFB8 with a specific initialization vector (the shared secret itself), and the compression must follow the exact frame format (a `VarInt` data length field, where 0 means uncompressed). These are protocol-level requirements with zero flexibility — the bytes on the wire must match exactly.

## Decision Drivers

- **Byte-level correctness**: the encrypted and compressed byte stream must be identical to what vanilla produces — any deviation breaks client compatibility
- **Stream vs. frame semantics**: encryption is a continuous stream cipher (state carries across frames), compression is per-frame (each frame is independently compressed/decompressed)
- **Performance**: encryption and compression are applied to every byte on the wire — they must be efficient, with minimal allocation and buffer copying
- **Memory reuse**: zlib deflate/inflate contexts are expensive to allocate — they should be reused across frames, not created per-frame
- **Correct pipeline ordering**: the transformation order (encrypt → frame → compress → packet) must be enforced by the architecture, not by developer discipline

## Considered Options

### Option 1: Transform at byte stream level (wrap AsyncRead/AsyncWrite)

Wrap the `TcpStream`'s `AsyncRead`/`AsyncWrite` implementations with encryption and compression layers, similar to how `tokio-rustls` wraps a stream for TLS. Each layer is a struct that implements `AsyncRead`/`AsyncWrite` and transforms bytes as they pass through. This is elegant for encryption (which is a stream transform) but awkward for compression (which is per-frame, requiring frame boundary knowledge that a raw byte stream doesn't have).

### Option 2: Transform at frame level (encrypt/compress each frame independently)

Apply both encryption and compression on a per-frame basis. Each frame is encrypted and compressed independently. This simplifies the API (each frame is self-contained) but is incorrect for AES-CFB8 — it's a stream cipher whose internal state must carry across frame boundaries. Resetting the cipher per frame would produce incorrect output that clients reject.

### Option 3: Hybrid — stream-level encryption, frame-level compression

Apply AES-CFB8 encryption at the byte stream level (stateful across frames, applied to all bytes including frame headers). Apply zlib compression at the frame level (each frame's payload is independently compressed/decompressed). This matches vanilla's pipeline exactly: bytes are decrypted as they arrive, then frame boundaries are identified, then individual frames are decompressed.

## Decision

**We adopt the hybrid approach (Option 3): stream-level encryption with frame-level compression.** This precisely matches vanilla's Netty pipeline ordering and correctly handles the distinct semantics of each transformation.

### Pipeline Order

```
Read path:  TCP bytes → AES-CFB8 decrypt (stream) → Frame decode → Zlib decompress (per-frame) → Packet decode
Write path: Packet encode → Zlib compress (per-frame) → Frame encode → AES-CFB8 encrypt (stream) → TCP bytes
```

### AES-128-CFB8 Encryption

AES-128-CFB8 is a stream cipher mode that encrypts one byte at a time using an 8-bit feedback shift register. The initialization vector (IV) and key are both the 16-byte shared secret negotiated during login.

```rust
use aes::Aes128;
// Manual CFB-8 implementation — the `cfb8` crate 0.9.0-rc.3 is incompatible
// with cipher 0.5 and `cfb-mode` only supports CFB-128, not CFB-8.
// We implement CFB-8 directly using AES-128 block cipher.

pub struct CipherState {
    cipher: Aes128,
    enc_iv: [u8; 16],  // separate shift registers for each direction
    dec_iv: [u8; 16],
}

impl CipherState {
    /// Create a new cipher state from the shared secret.
    /// Both key and IV are the shared secret (16 bytes).
    pub fn new(shared_secret: &[u8; 16]) -> Self {
        Self {
            encryptor: Cfb8Enc::<Aes128>::new(shared_secret.into(), shared_secret.into()),
            decryptor: Cfb8Dec::<Aes128>::new(shared_secret.into(), shared_secret.into()),
        }
    }

    /// Decrypt bytes in-place. Cipher state is updated (stateful across calls).
    pub fn decrypt(&mut self, data: &mut [u8]) {
        self.decryptor.decrypt(data);
    }

    /// Encrypt bytes in-place. Cipher state is updated (stateful across calls).
    pub fn encrypt(&mut self, data: &mut [u8]) {
        self.encryptor.encrypt(data);
    }
}
```

**Critical**: The cipher state is stateful — `decrypt`/`encrypt` advance an internal feedback register. Calling `decrypt` on frame N affects the state used for frame N+1. This is why encryption must operate on the raw byte stream, not on individual frames.

### Frame Format

Before compression is enabled, frames have this format:

```
┌──────────────────────┬──────────────────────┐
│ Packet Length (VarInt)│ Packet Data          │
│ (length of Data)     │ (ID + Fields)        │
└──────────────────────┴──────────────────────┘
```

After compression is enabled, frames have this format:

```
┌──────────────────────┬──────────────────────┬──────────────────────┐
│ Packet Length (VarInt)│ Data Length (VarInt)  │ Packet Data          │
│ (length of rest)     │ (0 = uncompressed)   │ (compressed or raw)  │
└──────────────────────┴──────────────────────┴──────────────────────┘
```

- **Packet Length**: VarInt — the byte length of Data Length + Packet Data
- **Data Length**: VarInt — the uncompressed size of the Packet Data. If 0, the data is not compressed (below threshold)
- **Packet Data**: if Data Length > 0, this is zlib-compressed; otherwise, it's raw packet bytes

### Compression Implementation

```rust
use flate2::Compress;
use flate2::Decompress;
use flate2::FlushCompress;
use flate2::FlushDecompress;
use flate2::Compression;

pub struct CompressionState {
    threshold: usize,
    compressor: Compress,
    decompressor: Decompress,
    compress_buf: Vec<u8>,
}

impl CompressionState {
    /// Create a new compression state with the given threshold.
    /// Frames with uncompressed size below the threshold are sent raw.
    pub fn new(threshold: usize) -> Self {
        Self {
            threshold,
            compressor: Compress::new(Compression::default(), /* zlib */ true),
            decompressor: Decompress::new(/* zlib */ true),
            compress_buf: Vec::with_capacity(8192),
        }
    }

    /// Compress a frame's packet data if it exceeds the threshold.
    /// Returns the Data Length field value and the (possibly compressed) data.
    pub fn compress_frame(&mut self, data: &[u8], out: &mut BytesMut) -> Result<(), ProtocolError> {
        if data.len() < self.threshold {
            // Below threshold — send uncompressed
            write_var_int(out, 0); // Data Length = 0 means uncompressed
            out.extend_from_slice(data);
        } else {
            // Above threshold — compress with zlib
            write_var_int(out, data.len() as i32); // Uncompressed Data Length

            self.compress_buf.clear();
            self.compressor.reset();
            // Compress into temporary buffer
            let status = self.compressor.compress(data, &mut self.compress_buf, FlushCompress::Finish)?;
            out.extend_from_slice(&self.compress_buf);
        }
        Ok(())
    }

    /// Decompress a frame's packet data.
    /// `data_length` is the Data Length field from the frame (0 = uncompressed).
    pub fn decompress_frame(
        &mut self,
        compressed: &[u8],
        data_length: usize,
    ) -> Result<Vec<u8>, ProtocolError> {
        if data_length == 0 {
            // Uncompressed — return as-is
            return Ok(compressed.to_vec());
        }

        let mut decompressed = vec![0u8; data_length];
        self.decompressor.reset(/* zlib */ true);
        let status = self.decompressor.decompress(compressed, &mut decompressed, FlushDecompress::Finish)?;

        if decompressed.len() != data_length {
            return Err(ProtocolError::DecompressionSizeMismatch {
                expected: data_length,
                actual: decompressed.len(),
            });
        }

        Ok(decompressed)
    }
}
```

### Memory Reuse

The `Compress` and `Decompress` objects from `flate2` maintain internal zlib state (dictionaries, hash tables). Creating them is expensive (~256 KB allocation each). By storing them in `CompressionState` and calling `reset()` between frames, we reuse this allocation across the lifetime of the connection. The `compress_buf` is also reused, avoiding per-frame allocation.

### Default Compression Threshold

The default threshold is **256 bytes** (matching vanilla). Frames with uncompressed payload below 256 bytes are sent raw (Data Length = 0). This avoids the overhead of compressing small packets (keep-alives, position updates) where compression would actually increase size due to zlib headers.

### Enabling Encryption and Compression

Both transformations are enabled during the Login state via control messages to the reader/writer tasks:

```rust
// In Connection<Login>::run_login_sequence()

// 1. Send EncryptionRequest, receive EncryptionResponse
let shared_secret = decrypt_shared_secret(&response, &server_key)?;
let cipher = CipherState::new(&shared_secret);

// Enable encryption on both read and write paths
self.reader_ctrl.send(ReaderControl::EnableEncryption(cipher.clone_decrypt())).await?;
self.writer_ctrl.send(WriterControl::EnableEncryption(cipher.clone_encrypt())).await?;

// 2. Send SetCompression
let threshold = config.compression_threshold; // default: 256
self.send_packet(SetCompressionPacket { threshold: VarInt(threshold as i32) }).await?;

// Enable compression on both paths
self.reader_ctrl.send(ReaderControl::EnableCompression(threshold)).await?;
self.writer_ctrl.send(WriterControl::EnableCompression(threshold)).await?;
```

### Timing Constraint

Encryption must be enabled **after** the `EncryptionResponse` is received but **before** any subsequent packets are sent. The `SetCompression` packet itself is sent **uncompressed** (compression isn't active yet) but **encrypted** (encryption was just enabled). The `LoginSuccess` packet is sent both encrypted and compressed. This ordering is enforced by the sequential nature of the login flow.

## Consequences

### Positive

- Byte-for-byte compatibility with vanilla's encryption and compression — clients connect without issues
- Stream-level encryption correctly handles AES-CFB8's stateful nature — cipher state flows across frame boundaries
- Frame-level compression allows independent decompression of each frame — a corrupted frame doesn't affect subsequent frames
- Memory reuse via persistent `Compress`/`Decompress` objects avoids ~512 KB allocation per connection per frame
- The threshold mechanism avoids wasting CPU on compressing small packets where compression has negative returns

### Negative

- AES-CFB8 is a slow cipher mode (encrypts one byte at a time, requiring one AES block operation per byte) — this is a protocol constraint we cannot change without breaking clients
- The `Compress`/`Decompress` objects add ~512 KB of resident memory per connection — at 1000 players, that's ~500 MB just for zlib state (mitigated: most of this is virtual memory, only ~32 KB is hot)
- Encryption must be enabled symmetrically on read and write tasks with precise timing — a race condition could cause one side to encrypt while the other doesn't, corrupting the stream

### Neutral

- The `flate2` crate wraps either `miniz_oxide` (pure Rust) or `zlib-ng` (C, faster) — we default to `miniz_oxide` for build simplicity but can switch to `zlib-ng` via feature flag for ~30% compression speedup
- AES-128-CFB8 is not considered cryptographically strong by modern standards (no authentication, susceptible to bit-flipping) — but the Minecraft protocol has always used it and we must match the wire format

## Compliance

- **Byte-level integration test**: connect to an Oxidized server with a vanilla client and verify successful login, or capture and replay a vanilla encryption handshake and verify byte-identical output
- **Round-trip test**: for each compression level (below threshold, at threshold, above threshold), compress → decompress and verify the output matches the input exactly
- **Cipher continuity test**: encrypt three frames sequentially, then decrypt them sequentially — verify the plaintext matches. Then encrypt three frames with a fresh cipher and decrypt with a fresh cipher — verify the results differ from decrypting with a stale cipher
- **Threshold boundary test**: verify that a packet of exactly `threshold - 1` bytes is sent uncompressed and a packet of exactly `threshold` bytes is sent compressed
- **Memory test**: create 100 compression states and verify total memory usage stays under 60 MB (100 × 512 KB + overhead)
- **Code review**: any modification to the encryption or compression pipeline must include a comment referencing the specific protocol spec section it implements

## Related ADRs

- [ADR-001: Async Runtime Selection](adr-001-async-runtime.md) — reader/writer tasks run on Tokio's async runtime
- [ADR-006: Network I/O Architecture](adr-006-network-io.md) — cipher and compressor state are owned by the reader/writer tasks
- [ADR-007: Packet Codec Framework](adr-007-packet-codec.md) — packet encode/decode operates on decompressed, decrypted data
- [ADR-008: Connection State Machine](adr-008-connection-state-machine.md) — encryption and compression are enabled during the Login state transition

## References

- [wiki.vg — Protocol Encryption](https://wiki.vg/Protocol_Encryption)
- [wiki.vg — Packet Format (with compression)](https://wiki.vg/Protocol#With_compression)
- [AES-CFB8 mode — NIST SP 800-38A](https://csrc.nist.gov/publications/detail/sp/800-38a/final)
- ~~RustCrypto cfb8 crate~~ — not used; manual CFB-8 implementation required (see implementation notes above)
- [flate2 crate documentation](https://docs.rs/flate2/latest/flate2/)
- [Netty CipherEncoder source](https://github.com/netty/netty/blob/4.1/handler/src/main/java/io/netty/handler/codec/)
