# TaskFlow

A user-friendly GitHub task manager built in Rust. Makes project management accessible and fun, especially for non-technical team members.

## Features
- 🎯 Simple, conversational interface
- 📋 Syncs with GitHub issues
- 🔄 Multiple repository support
- ⭐ Priority and status tracking
- 🎨 Color-coded interface
- 📅 Due date tracking

## Installation

```bash
# Clone the repository
git clone https://github.com/joe-corcoran/task-flow
cd task-flow

# Build and run
cargo build
cargo run
```

## First-Time Setup
1. Get a GitHub token (the app will guide you)
2. Add your repository details
3. Start creating tasks!

## Using with Existing Projects

Add to your project:
```bash
cd your-project
mkdir tools
cd tools
git clone https://github.com/joe-corcoran/task-flow
```

Add to your project's `Cargo.toml`:
```toml
[
workspace
]
members = [
    ".",
    "tools/task-flow"
]
```

Run from your project:
```bash
cargo run -p task-flow
```

## License
MIT