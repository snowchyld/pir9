# pir9 Architecture

This document describes the architecture of pir9.

## Overview

pir9 is a complete rewrite of the original C# Sonarr application, maintaining API compatibility while leveraging Rust's performance and safety guarantees.

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                         Web Layer                               │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐  │
│  │   Static    │  │  WebSocket  │  │      REST API           │  │
│  │   Files     │  │  Handler    │  │  (v3/v5 compatibility)  │  │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                         API Layer                               │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐  │
│  │   Series    │  │   Episode   │  │      Indexers           │  │
│  │   Routes    │  │   Routes    │  │      Routes             │  │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐  │
│  │   Queue     │  │   System    │  │      Config             │  │
│  │   Routes    │  │   Routes    │  │      Routes             │  │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Core Business Logic                        │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐  │
│  │    TV       │  │   Parser    │  │      Profiles           │  │
│  │  (Series,   │  │  (Release   │  │  (Quality, Language,    │  │
│  │  Episodes)  │  │   Parsing)  │  │   Delay)                │  │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐  │
│  │   Indexers  │  │   Download  │  │   Media Files           │  │
│  │ (RSS/Search)│  │   Clients   │  │   (Import/Rename)       │  │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐  │
│  │    Queue    │  │ Notifications│  │      Scheduler          │  │
│  │  Management │  │             │  │  (Background Jobs)      │  │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Infrastructure Layer                       │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐  │
│  │  Datastore  │  │  Messaging  │  │    Configuration        │  │
│  │ (SQLite/PG) │  │  (Event Bus)│  │                         │  │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

## Module Descriptions

### Web Layer (`src/web/`)

Handles HTTP requests, WebSocket connections, and static file serving.

- **Static Files**: Serves the frontend React application
- **WebSocket**: Real-time updates to connected clients
- **AppState**: Shared application state across handlers

### API Layer (`src/api/`)

RESTful API endpoints with v3 (legacy) and v5 (current) compatibility.

- **Series**: CRUD operations for TV series
- **Episodes**: Episode management
- **Indexers**: Indexer configuration and testing
- **Download**: Download client management
- **Queue**: Download queue operations
- **System**: Health checks, backups, updates
- **Config**: Application configuration

### Core Layer (`src/core/`)

Business logic and domain models.

#### TV Module (`core/tv/`)

Domain models and services for TV shows.

- **Series**: TV series entity with metadata
- **Episode**: Individual episodes
- **EpisodeFile**: Downloaded episode files
- **Services**: Business logic for series/episode operations
- **Repositories**: Database access layer

#### Indexers Module (`core/indexers/`)

RSS feed and search integration.

- **Indexer**: Configuration for indexers
- **ReleaseInfo**: Parsed release information
- **RssSync**: RSS feed synchronization
- **Search**: Episode/release searching

#### Download Module (`core/download/`)

Download client integrations.

- **DownloadClient**: Configuration for download clients
- **Clients**: Implementations for qBittorrent, SABnzbd, etc.
- **History**: Download history tracking
- **Queue**: Download queue management

#### Parser Module (`core/parser/`)

Release title parsing.

- **ParsedEpisodeInfo**: Extracted information from release titles
- **Parsing functions**: Regex-based title parsing

#### Profiles Module (`core/profiles/`)

Quality, language, and delay profiles.

- **QualityProfile**: Quality preferences and upgrade paths
- **LanguageProfile**: Language preferences
- **DelayProfile**: Download delay settings

#### Media Files Module (`core/mediafiles/`)

File management and media info.

- **EpisodeFile**: Episode file metadata
- **MediaInfoModel**: Technical media information
- **MediaAnalyzer**: File analysis

#### Notifications Module (`core/notifications/`)

Notification provider integrations.

- **Notification**: Configuration for notification providers
- **Providers**: Discord, Slack, Email, etc.

### Infrastructure Layer

#### Datastore (`core/datastore/`)

Database access and migrations.

- **Database**: Connection pool management
- **Models**: SQLx database models
- **Repositories**: Data access patterns

#### Messaging (`core/messaging.rs`)

Event bus for inter-component communication.

- **EventBus**: Publish-subscribe messaging
- **Message**: Event types

#### Configuration (`core/configuration.rs`)

Application configuration management.

- **AppConfig**: Main configuration structure
- **Environment/ file-based config loading

#### Scheduler (`core/scheduler.rs`)

Background job scheduling.

- **JobScheduler**: Cron-based job scheduling
- **ScheduledJob**: Job definitions

## Data Flow

### Adding a Series

1. User sends POST request to `/api/v5/series`
2. API layer validates request
3. `SeriesService.add_series()` called
4. Series saved to database
5. Metadata refresh job queued
6. Event published to EventBus
7. WebSocket clients notified

### RSS Sync

1. Scheduler triggers RSS sync job
2. `RssSyncService.sync()` called
3. Each enabled indexer polled
4. New releases parsed and stored
5. Matching episodes identified
6. Downloads queued

### Episode Import

1. Download completes
2. Download client notifies pir9
3. `MediaAnalyzer` analyzes file
4. Episode matched to series/episode
5. File renamed and moved
6. Database updated
7. Notifications sent

## Technology Choices

### Why Axum?

- Modern, ergonomic web framework
- Built on Tokio and Hyper
- Excellent middleware support
- Native WebSocket support

### Why SQLx?

- Compile-time checked SQL
- No runtime ORM overhead
- Flexible query building
- Async-first design

### Why Tokio?

- Industry-standard async runtime
- Excellent performance
- Rich ecosystem
- Mature and well-maintained

## Performance Considerations

### Memory Usage

- Zero-copy parsing where possible
- Connection pooling for database
- Efficient data structures

### Concurrency

- Async/await throughout
- Lock-free data structures where appropriate
- Bounded channels for backpressure

### Caching

- In-memory caching for frequently accessed data
- Cache invalidation via EventBus
- Configurable cache sizes

## Security

### Authentication

- JWT-based authentication
- Session management
- Configurable auth methods

### Input Validation

- Request validation at API layer
- SQL injection prevention via SQLx
- XSS protection

### Secrets Management

- API keys stored encrypted
- Environment variable support
- Secure defaults

## Testing Strategy

### Unit Tests

- Business logic testing
- Mock dependencies
- Fast execution

### Integration Tests

- API endpoint testing
- Database integration
- External service mocking

### End-to-End Tests

- Full workflow testing
- Docker-based environment
- CI/CD integration

## Deployment

### Docker

- Multi-stage builds
- Minimal runtime image
- Health checks

### Configuration

- Environment variables
- Volume mounts for data
- Secrets management

### Monitoring

- Structured logging
- Metrics collection
- Health endpoints
