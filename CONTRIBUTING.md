# Contributing to Pir9

Thank you for your interest in contributing to Pir9! This document provides guidelines and instructions for contributing.

## Code of Conduct

This project and everyone participating in it is governed by our Code of Conduct. By participating, you are expected to uphold this code.

## How Can I Contribute?

### Reporting Bugs

Before creating bug reports, please check the existing issues to see if the problem has already been reported. When you are creating a bug report, please include as many details as possible:

- **Use a clear and descriptive title**
- **Describe the exact steps to reproduce the problem**
- **Provide specific examples to demonstrate the steps**
- **Describe the behavior you observed and what behavior you expected**
- **Include code samples and screenshots if applicable**

### Suggesting Enhancements

Enhancement suggestions are tracked as GitHub issues. When creating an enhancement suggestion, please include:

- **Use a clear and descriptive title**
- **Provide a step-by-step description of the suggested enhancement**
- **Provide specific examples to demonstrate the enhancement**
- **Explain why this enhancement would be useful**

### Pull Requests

1. Fork the repository
2. Create a new branch from `main` for your feature or bug fix
3. Make your changes
4. Ensure your code follows the style guidelines
5. Add tests for your changes
6. Update documentation as needed
7. Submit a pull request

## Development Setup

### Prerequisites

- Rust 1.93+ (install via [rustup](https://rustup.rs/))
- SQLite (optional, bundled by default)
- Docker (optional, for containerized development)

### Building

```bash
# Clone your fork
git clone https://github.com/YOUR_USERNAME/pir9.git
cd pir9

# Build
cargo build

# Run tests
cargo test

# Run with logging
RUST_LOG=debug cargo run
```

### Project Structure

```
pir9/
├── src/
│   ├── main.rs          # Application entry point
│   ├── cli.rs           # CLI tool
│   ├── api/             # REST API layer
│   ├── core/            # Business logic
│   └── web/             # Web layer
├── migrations/          # Database migrations
├── tests/               # Integration tests
└── docs/                # Documentation
```

## Coding Guidelines

### Rust Style Guide

We follow the standard Rust style guidelines:

- Use `rustfmt` for formatting: `cargo fmt`
- Use `clippy` for linting: `cargo clippy`
- Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)

### Code Organization

- **Modules**: Organize code by feature/domain
- **Error Handling**: Use `anyhow` for application errors, `thiserror` for library errors
- **Async**: Use `tokio` for async runtime
- **Logging**: Use `tracing` for structured logging

### Documentation

- Document all public APIs with doc comments
- Include examples in doc comments where helpful
- Update README.md if adding new features

### Testing

- Write unit tests for business logic
- Write integration tests for API endpoints
- Aim for high test coverage on critical paths

```bash
# Run all tests
cargo test

# Run tests with coverage
cargo tarpaulin

# Run specific test
cargo test test_name
```

## Database Migrations

When making schema changes:

1. Create a new migration file in `migrations/`
2. Name it with the next sequence number: `002_add_new_table.sql`
3. Include both up and down migrations
4. Test migrations on a fresh database

```bash
# Create new migration
echo "-- Migration description" > migrations/002_description.sql
```

## API Compatibility

When modifying API endpoints:

- Maintain backward compatibility when possible
- Version breaking changes (v3, v5, etc.)
- Update API documentation
- Add tests for new endpoints

## Commit Messages

Use clear and meaningful commit messages:

- Use the present tense ("Add feature" not "Added feature")
- Use the imperative mood ("Move cursor to..." not "Moves cursor to...")
- Limit the first line to 72 characters
- Reference issues and pull requests liberally after the first line

Example:
```
Add support for custom formats

- Implement custom format specification parsing
- Add database schema for custom formats
- Update API endpoints

Fixes #123
```

## Release Process

1. Update version in `Cargo.toml`
2. Update `CHANGELOG.md`
3. Create a git tag: `git tag v0.1.0`
4. Push tag: `git push origin v0.1.0`
5. Create GitHub release

## Getting Help

- Join our [Discord](https://discord.gg/pir9)
- Check existing [issues](https://github.com/pir9/pir9/issues)
- Read the [documentation](https://github.com/pir9/pir9)

## License

By contributing, you agree that your contributions will be licensed under the GPL-3.0 License.
