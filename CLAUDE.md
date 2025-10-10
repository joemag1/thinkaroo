# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Thinkaroo is a Rust web server built with the Axum framework, using Tokio for async runtime. The project is in early stages with a basic health check endpoint.

## Build and Run Commands

```bash
# Build the project
cargo build

# Run the server in development mode
cargo run

# Build for production (optimized)
cargo build --release

# Run production build
./target/release/thinkaroo

# Check code without building
cargo check

# Run tests
cargo test

# Run tests with output
cargo test -- --nocaptures

# Format code
cargo fmt

# Lint code
cargo clippy
```

## Architecture

- **Web Framework**: Axum 0.7 - A web application framework that focuses on ergonomics and modularity
- **Async Runtime**: Tokio with full features enabled
- **Server Entry Point**: `src/main.rs` - Currently defines a single `/health` endpoint that returns "OK"
- **Server Binding**: The server binds to `0.0.0.0:8080` by default

## Project Structure

- `src/main.rs` - Main application entry point containing server setup and route definitions
- `Cargo.toml` - Project dependencies and metadata (Rust 2024 edition)

## Development Notes

- The server listens on port 8080
- Currently implements a single health check endpoint at `/health`
- The project uses unwrap() for error handling in the main function - consider adding proper error handling for production use
