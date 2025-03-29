# Raft Chat

A distributed chat application implementing the Raft consensus protocol in Rust.

## Overview

This project implements a distributed chat system using the Raft consensus protocol. The system will demonstrate key concepts of distributed systems including:

- Leader election
- Log replication
- Membership changes
- Fault tolerance
- Network communication

## Prerequisites

- Rust (latest stable version)
- Cargo (comes with Rust)

## Building

```bash
cargo build
```

## Running

```bash
cargo run
```

## Project Structure

```
raft-chat/
├── src/
│   ├── main.rs           # Application entry point
│   ├── raft/             # Raft protocol implementation
│   ├── network/          # Network communication layer
│   └── storage/          # Persistent storage
├── tests/                # Integration tests
└── Cargo.toml           # Project dependencies and metadata
```

## Development Status

🚧 This project is currently under development.

## License

MIT License 