# Pir9

A modern, high-performance Smart PVR for TV and anime, written in Rust.

## Overview

Pir9 is a complete media management solution designed for TV series and anime. It automatically monitors RSS feeds, downloads episodes, and organizes your library.

## Architecture

```
pir9/
├── src/
│   ├── main.rs              # Application entry point
│   ├── cli.rs               # CLI tool entry point
│   ├── api/                 # REST API layer
│   │   ├── v3/              # API v3 (legacy compatibility)
│   │   ├── v5/              # API v5 (current)
│   │   └── models.rs        # Shared API models
│   ├── core/                # Core business logic
│   │   ├── configuration.rs # App configuration
│   │   ├── datastore/       # Database layer
│   │   │   ├── models.rs    # Database models
│   │   │   └── repositories.rs
│   │   ├── download/        # Download client integration
│   │   ├── indexers/        # Indexer/RSS feed integration
│   │   ├── mediafiles/      # Media file management
│   │   ├── messaging.rs     # Event bus
│   │   ├── notifications/   # Notification providers
│   │   ├── parser/          # Release title parser
│   │   ├── profiles/        # Quality/Language/Delay profiles
│   │   ├── queue/           # Download queue
│   │   ├── scheduler.rs     # Background job scheduler
│   │   └── tv/              # TV domain (Series, Episodes)
│   └── web/                 # Web layer (WebSocket, static files)
├── migrations/              # Database migrations
├── frontend/             # Frontend (TypeScript/Web Components)
└── config/                  # Configuration files
```

## Features

### Core Features

- **Series Management**
  - Add, update, delete TV series
  - Automatic metadata refresh from TVDB/TMDB
  - Season and episode tracking
  - Monitoring options

- **Download Management**
  - Integration with Usenet clients (SABnzbd, NZBGet)
  - Integration with BitTorrent clients (qBittorrent, Transmission, etc.)
  - RSS feed monitoring
  - Automatic download grabbing
  - Queue management

- **Quality Management**
  - Quality profiles with upgrade paths
  - Language profiles
  - Custom formats
  - Delay profiles

- **File Management**
  - Automatic episode renaming
  - Season folder organization
  - Hardlink/copy support
  - Media info extraction

- **Notifications**
  - Webhook support
  - Discord, Slack, Telegram
  - Email notifications

- **API**
  - RESTful API (v3/v5)
  - WebSocket for real-time updates
  - OpenAPI/Swagger documentation

## Technology Stack

| Component | Technology |
|-----------|------------|
| Runtime | Tokio (async runtime) |
| Web Framework | Axum |
| Database | SQLite (default) / PostgreSQL |
| ORM | SQLx |
| Serialization | Serde |
| HTTP Client | Reqwest |
| RSS Parsing | rss + atom_syndication |
| Scheduling | tokio-cron-scheduler |
| Logging | Tracing |
| Frontend | Web Components + TanStack Query |

## Building

### Prerequisites

- Rust 1.93+ (install via [rustup](https://rustup.rs/))
- SQLite (optional, bundled by default)
- Node.js 18+ (for frontend)

### Build

```bash
# Clone the repository
git clone https://github.com/pir9/pir9.git
cd pir9

# Build in release mode
cargo build --release

# Run tests
cargo test
```

### Docker

```bash
# Build Docker image
docker build -t pir9 .

# Run
docker run -p 8989:8989 -v /path/to/config:/config -v /path/to/data:/data pir9
```

### Docker Compose

```bash
# Production (multi-container with Redis)
docker compose --profile production up -d

# Simple (single container)
docker compose -f docker-compose.simple.yml up -d
```

## Configuration

Configuration is loaded from (in order of priority):
1. Environment variables (`PIR9_*`)
2. `config/config.toml`
3. `/config/pir9.toml`
4. Default values

### Example `config.toml`

```toml
[server]
port = 8989
bind_address = "0.0.0.0"
enable_ssl = false

[database]
database_type = "sqlite"
connection_string = "pir9.db"
max_connections = 10

[security]
enable_authentication = true
session_timeout_hours = 24

[download]
check_interval_secs = 60
enable_completed_download_handling = true

[media]
default_root_folder = "/data/tv"
rename_episodes = true
```

### Environment Variables

- `PIR9_PORT` - Server port
- `PIR9_BIND` - Bind address
- `PIR9_DB_TYPE` - Database type (sqlite/postgres)
- `PIR9_DB_CONNECTION` - Database connection string
- `PIR9_SECRET_KEY` - Secret key for encryption
- `PIR9_REDIS_URL` - Redis URL for distributed deployments
- `PIR9_REDIS_EVENTS` - Enable Redis event bus (true/false)

## API Endpoints

### Series

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/v5/series` | List all series |
| GET | `/api/v5/series/:id` | Get series by ID |
| POST | `/api/v5/series` | Add new series |
| PUT | `/api/v5/series/:id` | Update series |
| DELETE | `/api/v5/series/:id` | Delete series |
| POST | `/api/v5/series/:id/refresh` | Refresh from metadata |
| POST | `/api/v5/series/:id/rescan` | Rescan files |

### Episodes

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/v5/episode` | List episodes |
| GET | `/api/v5/episode/:id` | Get episode by ID |
| PUT | `/api/v5/episode/:id` | Update episode |

### Indexers

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/v5/indexer` | List indexers |
| POST | `/api/v5/indexer` | Add indexer |
| PUT | `/api/v5/indexer/:id` | Update indexer |
| DELETE | `/api/v5/indexer/:id` | Delete indexer |
| POST | `/api/v5/indexer/:id/test` | Test indexer |

### System

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/v5/system/status` | System status |
| GET | `/api/v5/system/health` | Health check |
| GET | `/api/v5/system/diskspace` | Disk space info |
| POST | `/api/v5/system/restart` | Restart application |
| POST | `/api/v5/system/shutdown` | Shutdown application |

## Development

### Makefile Commands

```bash
make help           # Show all available commands
make start          # Start all services (production)
make stop           # Stop all services
make restart        # Restart with rebuild
make logs           # Tail logs
make build          # Build Docker images
make dev-api        # Build Rust API locally
make dev-frontend   # Build frontend locally
make deploy         # Quick deploy without full rebuild
make test           # Run tests
make lint           # Run linters
```

## Contributing

Contributions are welcome! Please open an issue or pull request.

## License

This project is licensed under the GPL-3.0 License - see [LICENSE](LICENSE) for details.
