---
title: Installation
description: Install Graphos for Python or Rust.
---

# Installation

Graphos can be used from both Python and Rust. Choose the installation method for your preferred language.

## Python

### Using uv (Recommended)

[uv](https://github.com/astral-sh/uv) is a fast Python package installer:

```bash
uv add pygraphos
```

### Using pip (alternative)

```bash
pip install pygraphos  # If uv is not available
```

### Verify Installation

```python
import graphos

# Print version
print(graphos.__version__)

# Create a test database
db = graphos.Database()
print("Graphos installed successfully!")
```

### Platform Support

| Platform | Architecture | Support |
|----------|--------------|---------|
| Linux    | x86_64       | :material-check: Full |
| Linux    | aarch64      | :material-check: Full |
| macOS    | x86_64       | :material-check: Full |
| macOS    | arm64 (M1/M2)| :material-check: Full |
| Windows  | x86_64       | :material-check: Full |

## Rust

### Using Cargo

Add Graphos to your project:

```bash
cargo add graphos
```

Or add it manually to your `Cargo.toml`:

```toml
[dependencies]
graphos = "0.1"
```

### Feature Flags

Graphos supports optional features:

```toml
[dependencies]
graphos = { version = "0.1", features = ["full"] }
```

| Feature | Description |
|---------|-------------|
| `default` | Core functionality |
| `full` | All features enabled |

### Verify Installation

```rust
use graphos::Database;

fn main() -> Result<(), graphos::Error> {
    let db = Database::open_in_memory()?;
    println!("Graphos installed successfully!");
    Ok(())
}
```

## Building from Source

### Clone the Repository

```bash
git clone https://github.com/StevenBtw/graphos.git
cd graphos
```

### Build Rust Crates

```bash
cargo build --workspace --release
```

### Build Python Package

```bash
cd crates/graphos-python
uv add maturin
maturin develop --release
```

## Next Steps

Now that you have Graphos installed, continue to the [Quick Start](quickstart.md) guide.
