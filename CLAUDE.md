# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Thinkaroo is an AI-powered test preparation application for kids, built with Rust and Axum. The app helps children prepare for standardized tests through interactive practice sessions in three core areas:

- **Math**: AI-generated math problems appropriate for the child's level
- **Reading Comprehension**: AI-generated passages with comprehension questions
- **Vocabulary**: AI-generated vocabulary exercises and word usage tests

### Key Features

- **AI Test Generation**: Each test section is dynamically generated using generative AI to provide fresh, varied practice material
- **AI-Powered Grading**: Student responses are evaluated using generative AI for nuanced feedback
- **Personalized Feedback**: The system provides constructive feedback on mistakes and suggests specific improvements
- **Adaptive Learning**: Recommendations based on performance patterns to help students improve

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

### Planned Architecture Components

- **API Layer**: RESTful API endpoints for test generation, submission, and grading
- **AI Service Layer**: Integration with generative AI APIs (e.g., Claude, OpenAI) for content generation and grading
- **Test Modules**: Separate modules for Math, Reading, and Vocabulary test types
- **Feedback Engine**: System for analyzing mistakes and generating improvement recommendations
- **Data Layer**: Storage for test sessions, student responses, and performance history

## Project Structure

- `src/main.rs` - Main application entry point containing server setup and route definitions
- `Cargo.toml` - Project dependencies and metadata (Rust 2024 edition)

### Planned Structure

```
src/
├── main.rs                 # Server entry point and route definitions
├── api/                    # API endpoint handlers
│   ├── mod.rs
│   ├── math.rs            # Math test endpoints
│   ├── reading.rs         # Reading comprehension endpoints
│   └── vocabulary.rs      # Vocabulary test endpoints
├── services/              # Business logic
│   ├── mod.rs
│   ├── ai_client.rs       # AI API integration
│   ├── test_generator.rs  # Test generation logic
│   └── grader.rs          # Grading and feedback logic
├── models/                # Data structures
│   ├── mod.rs
│   ├── test.rs            # Test models
│   └── response.rs        # Student response models
└── utils/                 # Helper functions
    └── mod.rs
```

## Development Notes

- The server listens on port 8080
- Currently implements a single health check endpoint at `/health`
- Will require API keys for AI services (consider using environment variables)
- Consider rate limiting for AI API calls
- Implement proper error handling throughout the application
- Consider adding authentication/authorization for student sessions
