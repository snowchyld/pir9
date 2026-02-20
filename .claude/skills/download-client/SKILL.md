---
name: download-client
description: Add a new download client integration (e.g., Deluge, Aria2)
user-invocable: true
arguments:
  - name: client
    description: Name of the download client to integrate
    required: true
allowed-tools:
  - Read
  - Write
  - Edit
  - Grep
  - Glob
  - Bash
---

# New Download Client: $ARGUMENTS

You are adding a new download client integration for **$ARGUMENTS** to pir9.

## Reference Files

Read these first to understand existing patterns:
- `src/core/download/clients.rs` — trait definition and existing implementations
- `src/core/download/` — directory listing for all client modules
- `src/api/v5/download_client.rs` — API endpoints for client management

## Steps

### 1. Implement the DownloadClient trait

Create a new module in `src/core/download/` for the client:

```rust
#[async_trait::async_trait]
pub trait DownloadClient: Send + Sync {
    fn name(&self) -> &str;
    fn protocol(&self) -> DownloadProtocol;  // Usenet or Torrent
    async fn test(&self) -> Result<()>;
    async fn add_from_url(&self, url: &str, category: &str) -> Result<String>;
    async fn add_from_file(&self, data: &[u8], filename: &str, category: &str) -> Result<String>;
    async fn get_downloads(&self) -> Result<Vec<DownloadStatus>>;
    async fn remove(&self, id: &str) -> Result<()>;
}
```

### 2. Add the variant to DownloadClientType enum

In `clients.rs`, add the new variant:
```rust
pub enum DownloadClientType {
    // ... existing variants ...
    $ARGUMENTS,
}
```

### 3. Register in the client factory

Wire up the new client in the factory/builder function that maps `DownloadClientType` to concrete implementations.

### 4. API settings

Ensure the download client settings endpoint can configure the new client's connection details (host, port, API key, etc.).

### 5. Test

- Unit tests for the client module with mocked HTTP responses
- Integration test that verifies the factory creates the correct client type

## Conventions
- Use `reqwest` for HTTP client calls (already a dependency)
- Use `anyhow` for error handling
- Map client-specific errors to `DownloadState` variants
- Follow existing client implementations (qBittorrent, SABnzbd, NZBGet, Transmission) as examples
