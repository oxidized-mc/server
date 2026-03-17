//! Oxidized — a high-performance Minecraft Java Edition server.
//!
//! This is the binary entry point. It configures the global allocator,
//! initialises tracing, and launches the Tokio async runtime.

use mimalloc::MiMalloc;

/// Use mimalloc as the global allocator for improved throughput and
/// reduced fragmentation under the server's allocation patterns.
/// See [ADR-029](../../docs/adr/adr-029-memory-management.md).
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() -> anyhow::Result<()> {
    // Initialise structured logging (RUST_LOG env filter).
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("Oxidized server starting...");

    // Build the Tokio runtime for async networking and I/O.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("oxidized-worker")
        .build()?;

    runtime.block_on(async {
        // TODO: Phase 1 — TCP listener, configuration loading, tick loop
        tracing::info!("Runtime ready — awaiting implementation");
        Ok(())
    })
}
