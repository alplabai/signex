# Modules, Crates, and Cargo

> Source: Rust Book chapters 7, 14

---

## Module system

```
Package  → one or more Crates, controlled by Cargo.toml
Crate    → binary (main.rs) or library (lib.rs)
Module   → named namespace inside a crate (mod keyword)
Path     → way to name items (absolute or relative)
```

### Defining modules

```rust
// src/lib.rs or src/main.rs:

mod front_of_house {
    pub mod hosting {         // pub makes the module visible from outside
        pub fn add_to_waitlist() {}
        fn seat_at_table() {}     // private by default
    }

    mod serving {
        fn take_order() {}
    }
}

// Inline module in a file, or in a separate file:
// mod garden;   → looks for src/garden.rs OR src/garden/mod.rs
```

### Paths

```rust
// Absolute path (from crate root):
crate::front_of_house::hosting::add_to_waitlist();

// Relative path:
front_of_house::hosting::add_to_waitlist();

// super:: (parent module):
super::hosting::add_to_waitlist();
```

### `use` — bring names into scope

```rust
use crate::front_of_house::hosting;
hosting::add_to_waitlist();

// Bring function directly (less idiomatic for functions — hides origin):
use crate::front_of_house::hosting::add_to_waitlist;
add_to_waitlist();

// Idiomatic: bring in the parent for functions, full path for types:
use std::collections::HashMap;   // type — full path OK
let mut map = HashMap::new();

// Aliases:
use std::fmt::Result as FmtResult;
use std::io::Result as IoResult;

// Re-exporting (pub use):
pub use crate::front_of_house::hosting;

// Multiple items from same crate:
use std::io::{self, Write};   // std::io and std::io::Write
use std::{cmp::Ordering, io};

// Glob (avoid in production — unclear what's in scope):
use std::collections::*;
```

### File structure

```
src/
  main.rs          (binary crate root)
  lib.rs           (library crate root)
  garden.rs        (module: `mod garden;` in main.rs)
  garden/
    vegetables.rs  (sub-module: `pub mod vegetables;` in garden.rs)
```

### Visibility

```rust
pub             // public to everyone
pub(crate)      // public within this crate only
pub(super)      // public to parent module
pub(in path)    // public to a specific module path
// (default)    // private: only accessible within this module and descendants

pub struct Breakfast {
    pub toast:     String,  // public field
    seasonal_fruit: String, // private field
}
```

---

## Cargo

### Basic commands

```bash
cargo new my_project         # create binary
cargo new my_lib --lib       # create library
cargo build                  # compile (debug)
cargo build --release        # compile (optimised)
cargo run                    # build and run
cargo run -- arg1 arg2       # with arguments
cargo test                   # run all tests
cargo test test_name         # run specific test
cargo check                  # type-check without producing binary (fast)
cargo doc --open             # build and open documentation
cargo fmt                    # format code (rustfmt)
cargo clippy                 # linter
cargo fix                    # auto-fix clippy suggestions
```

### Cargo.toml

```toml
[package]
name = "my_project"
version = "0.1.0"
edition = "2024"
authors = ["Your Name <you@example.com>"]
description = "A description"

[dependencies]
serde       = { version = "1", features = ["derive"] }
tokio       = { version = "1", features = ["full"] }
anyhow      = "1"
rand        = "0.8"

[dev-dependencies]
criterion = "0.5"   # benchmark framework — only for tests/benches

[build-dependencies]
cc = "1"            # only for build.rs

[features]
default = ["feature-a"]
feature-a = []
feature-b = ["serde"]

[[bin]]
name = "my-tool"
path = "src/bin/my-tool.rs"
```

### Semantic versioning

- `"1"` or `"1.0"` → `>=1.0.0, <2.0.0` (allows patch + minor updates)
- `"1.2"` → `>=1.2.0, <2.0.0`
- `"=1.2.3"` → exact version
- `">= 1.2, < 2"` → explicit range
- `"*"` → any version (avoid)

### Workspaces (monorepo)

```toml
# Cargo.toml at workspace root:
[workspace]
members = [
    "crate-a",
    "crate-b",
    "shared-types",
]
resolver = "2"

# All crates share one Cargo.lock and one target/ directory.
# Run from workspace root: cargo build --workspace
# Test all: cargo test --workspace
```

---

## Features

```toml
[features]
default = ["std"]
std     = []
alloc   = []
serde   = ["dep:serde"]

[dependencies]
serde = { version = "1", optional = true }
```

```rust
// Conditional compilation:
#[cfg(feature = "serde")]
use serde::{Serialize, Deserialize};

#[cfg(feature = "std")]
mod std_impl { /* ... */ }
```

---

## Publishing a crate

```bash
# Login:
cargo login YOUR_API_TOKEN

# Check before publish:
cargo publish --dry-run

# Publish:
cargo publish
```

Crates.io requirements:
- Unique name
- `description` and `license` in Cargo.toml
- Semantic versioning
- Published versions are permanent (only yanked, not deleted)

---

## Environment variables

```rust
// Build-time:
let version = env!("CARGO_PKG_VERSION");       // from Cargo.toml
let crate_name = env!("CARGO_PKG_NAME");

// Compile-time from env:
let api_key = option_env!("API_KEY");   // Option<&str>

// Runtime:
let path = std::env::var("HOME").unwrap_or_default();
let args: Vec<String> = std::env::args().collect();
```

---

## Profiles

```toml
[profile.dev]
opt-level = 0     # no optimisation, fast compile
debug = true

[profile.release]
opt-level = 3     # maximum optimisation
debug = false
lto = true        # link-time optimisation (even slower compile, faster binary)
codegen-units = 1 # single codegen unit (better inlining, slower compile)
panic = "abort"   # smaller binary, no stack unwinding
```
