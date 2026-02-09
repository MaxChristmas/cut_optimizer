# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Cut optimizer — a Rust CLI tool for material cutting optimization problems.

## Build & Development Commands

```bash
cargo build              # Debug build
cargo build --release    # Release build
cargo run                # Build and run
cargo test               # Run all tests
cargo test <test_name>   # Run a single test
cargo clippy             # Lint
cargo fmt                # Format code
cargo fmt -- --check     # Check formatting without modifying
```

## Architecture

- **Language:** Rust (edition 2024)
- **Entry point:** `src/main.rs`
- Currently a single-binary project with no external dependencies
