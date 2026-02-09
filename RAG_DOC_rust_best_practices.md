# Rust best practices for AI-assisted development

This reference document provides comprehensive Rust guidance for AI systems assisting developers. It covers idiomatic patterns, tooling, performance, async programming, and application-specific practices. Each section begins with principles, then provides specific patterns and code examples.

## Core ownership and borrowing patterns

Rust's ownership system is the foundation of its memory safety guarantees. Three rules govern all Rust code: **each value has exactly one owner**, when the owner goes out of scope the value is dropped, and ownership can be transferred (moved) between variables.

**Borrowing** allows temporary access without transferring ownership. The two borrowing rules are: any number of immutable references (`&T`) OR exactly one mutable reference (`&mut T`) at a time, and references must always be valid. Prefer borrowing over ownership transfer when possibleâ€”it's more flexible and avoids unnecessary data movement.

```rust
// Prefer slices over owned type references - more flexible
fn process_data(data: &[u8]) { }     // Accepts &Vec<u8>, &[u8; N], slices
fn process_text(s: &str) { }         // Accepts &String, &str, string literals

// Return owned data when caller needs ownership
fn create_user(name: &str) -> User {
    User { name: name.to_string() }
}
```

**Non-Lexical Lifetimes** (NLL) end references at last use, not scope end. Structure code to minimize nested borrows. Use `.clone()` only when necessaryâ€”it has runtime cost. For `Option` fields, prefer `self.field.take()` over cloning.

### Lifetime annotations follow predictable rules

The compiler elides lifetimes in **87% of cases** using three rules: each elided input lifetime becomes a distinct parameter, if there's exactly one input lifetime it applies to all outputs, and `&self`/`&mut self` lifetimes apply to outputs. Explicit annotations are required only when multiple input lifetimes create ambiguity.

```rust
// Elision handles most cases - don't add unnecessary annotations
fn first_word(s: &str) -> &str { /* compiler infers lifetimes */ }

// Explicit annotation needed: ambiguous - which input does output borrow from?
fn longest<'a>(x: &'a str, y: &'a str) -> &'a str {
    if x.len() > y.len() { x } else { y }
}

// Structs holding references need lifetime parameters
struct Excerpt<'a> {
    part: &'a str,  // Struct cannot outlive borrowed data
}
```

Use descriptive lifetime names for complex cases (`'src`, `'de` instead of `'a`). Reserve `'static` for truly static data compiled into the binary.

## Smart pointer selection guide

| Pointer | Use case | Thread-safe | Interior mutability |
|---------|----------|-------------|---------------------|
| `Box<T>` | Heap allocation, recursive types, large data | Data only | No |
| `Rc<T>` | Shared ownership, single-threaded | No | No |
| `Arc<T>` | Shared ownership, multi-threaded | Yes | No |
| `RefCell<T>` | Runtime borrow checking, single-threaded | No | Yes |
| `Mutex<T>` | Synchronized access, multi-threaded | Yes | Yes |

```rust
// Recursive types require Box (unknown size at compile time)
enum List {
    Cons(i32, Box<List>),
    Nil,
}

// Shared mutable state pattern: Arc<Mutex<T>> for threads
let counter = Arc::new(Mutex::new(0));
let counter_clone = Arc::clone(&counter);
thread::spawn(move || {
    *counter_clone.lock().unwrap() += 1;
});

// Single-threaded interior mutability: Rc<RefCell<T>>
let shared = Rc::new(RefCell::new(vec![]));
shared.borrow_mut().push(1);

// Use Weak<T> to break reference cycles (parent-child relationships)
struct Node {
    parent: RefCell<Weak<Node>>,     // Weak prevents cycles
    children: RefCell<Vec<Rc<Node>>>,
}
```

**Critical**: Never use `Rc` in multi-threaded codeâ€”it will compile but cause data races. Use `Arc` instead.

## Type system patterns and traits

### Generics and trait bounds

Use `where` clauses for complex bounds. Minimize constraintsâ€”only require traits actually used.

```rust
// Clear, readable bounds with where clause
fn process<T, U>(data: T, config: U) -> Result<Output, Error>
where
    T: AsRef<[u8]> + Send,
    U: Config + Default,
{
    // Implementation
}

// Bad: over-constraining (Clone not needed for references)
fn process<T: Clone>(data: &T) { }
// Good: minimal constraints
fn process<T>(data: &T) { }
```

**Associated types vs generics**: Use associated types when there's one logical type per implementation (like `Iterator::Item`). Use generic parameters when a trait can be implemented multiple times for different types (like `From<T>`).

### Standard traits to implement

Types should eagerly implement these traits where applicable:

- **`Debug`** â€” Required for all public types (use `#[derive(Debug)]`)
- **`Clone`**, **`Copy`** â€” If semantically appropriate (Copy for small, trivially copyable types)
- **`PartialEq`**, **`Eq`**, **`PartialOrd`**, **`Ord`** â€” For comparable types
- **`Hash`** â€” If `Eq` is implemented (needed for HashMap keys)
- **`Default`** â€” For types with sensible default values
- **`Display`** â€” For user-facing output
- **`Send`**, **`Sync`** â€” Where possible for thread safety

### The newtype pattern provides type safety at zero cost

Wrap primitive types to prevent mixing incompatible values and enable custom trait implementations.

```rust
struct UserId(u64);
struct OrderId(u64);

// Compiler prevents mixing user IDs with order IDs
fn get_user(id: UserId) -> User { /* ... */ }
fn get_order(id: OrderId) -> Order { /* ... */ }

// Implement Display differently for sensitive data
struct Password(String);
impl std::fmt::Display for Password {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "********")  // Never expose actual password
    }
}
```

### Builder pattern for complex construction

Use builders when constructing types with many optional parameters or validation requirements.

```rust
struct ServerConfig {
    host: String,
    port: u16,
    max_connections: usize,
}

struct ServerConfigBuilder {
    host: Option<String>,
    port: Option<u16>,
    max_connections: Option<usize>,
}

impl ServerConfigBuilder {
    fn new() -> Self {
        Self { host: None, port: None, max_connections: None }
    }
    
    fn host(mut self, host: impl Into<String>) -> Self {
        self.host = Some(host.into());
        self
    }
    
    fn port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }
    
    fn build(self) -> Result<ServerConfig, ConfigError> {
        Ok(ServerConfig {
            host: self.host.ok_or(ConfigError::MissingHost)?,
            port: self.port.unwrap_or(8080),
            max_connections: self.max_connections.unwrap_or(100),
        })
    }
}

// Fluent API usage
let config = ServerConfigBuilder::new()
    .host("localhost")
    .port(3000)
    .build()?;
```

### Typestate pattern encodes state machines in the type system

Invalid state transitions become compile errors rather than runtime bugs.

```rust
struct Unvalidated;
struct Validated;

struct Form<State> {
    data: String,
    _state: std::marker::PhantomData<State>,
}

impl Form<Unvalidated> {
    fn new(data: String) -> Self {
        Form { data, _state: std::marker::PhantomData }
    }
    
    fn validate(self) -> Result<Form<Validated>, ValidationError> {
        // Validation logic...
        Ok(Form { data: self.data, _state: std::marker::PhantomData })
    }
}

impl Form<Validated> {
    fn submit(&self) -> Result<(), SubmitError> {
        // Only validated forms can be submitted
        Ok(())
    }
}

// Compile error: can't call submit() on unvalidated form
// let form = Form::new("data".into());
// form.submit();  // Error: method not found
```

## Error handling strategies

### Result and Option usage

**`Result<T, E>`** is for recoverable errors (I/O, parsing, network). **`Option<T>`** is for values that may be absent without being errors. **`panic!`** is reserved for unrecoverable bugs and invariant violations.

```rust
// Result for operations that can fail
fn read_config(path: &str) -> Result<Config, ConfigError> {
    let content = std::fs::read_to_string(path)?;
    let config: Config = serde_json::from_str(&content)?;
    Ok(config)
}

// Option for absence without error
fn find_user(id: u64) -> Option<User> {
    users.get(&id).cloned()
}
```

The `?` operator propagates errors concisely and automatically converts error types via `From::from()`.

### When to panic vs return Result

**Use `panic!`** when: a bug has occurred that the programmer should fix, an invariant or contract has been violated, the bad state is unexpected (not occasional like invalid user input), or your code cannot continue safely.

**Use `Result`** when: failure is expected or possible, the caller can meaningfully recover, or you're writing library code (let callers decide how to handle errors).

**Legitimate `unwrap()`/`expect()` uses**: tests and examples, prototyping (mark with TODO), and when you can prove the operation won't fail:

```rust
// Hardcoded valid IP - we know this won't fail
let addr: IpAddr = "127.0.0.1".parse().expect("hardcoded IP should be valid");
```

### Custom errors with thiserror vs anyhow

| Use case | Recommendation |
|----------|----------------|
| Callers need to match on error variants | Custom enum with `thiserror` |
| Callers just propagate or log errors | `anyhow::Error` |
| Library code | Custom types for API stability |
| Application code | `anyhow` is often simpler |

```rust
// thiserror for libraries - structured errors callers can match on
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("connection failed: {0}")]
    Connection(#[from] std::io::Error),
    
    #[error("query failed: {0}")]
    Query(String),
    
    #[error("record not found: {id}")]
    NotFound { id: u64 },
}

// anyhow for applications - convenient error handling with context
use anyhow::{Context, Result, bail};

fn process_file(path: &str) -> Result<()> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path))?;
    
    if content.is_empty() {
        bail!("File {} is empty", path);
    }
    
    Ok(())
}
```

**Never use `()` as an error type**â€”it provides no information about what went wrong.

## Module organization and visibility

### File structure conventions

```
my_project/
â”œâ”€â”€ Cargo.toml
â”œâ”€â”€ src/
â”‚   â”œâ”€â”€ lib.rs              # Library crate root
â”‚   â”œâ”€â”€ main.rs             # Binary crate root
â”‚   â”œâ”€â”€ module_name.rs      # Module as single file (preferred)
â”‚   â””â”€â”€ complex_module/     # Module with submodules
â”‚       â”œâ”€â”€ mod.rs
â”‚       â””â”€â”€ submodule.rs
â”œâ”€â”€ tests/                  # Integration tests
â”œâ”€â”€ examples/               # Example programs
â””â”€â”€ benches/                # Benchmarks
```

Modern convention (post-Rust 2018): Prefer `module_name.rs` over `module_name/mod.rs` for cleaner file navigation.

### Visibility levels and best practices

| Modifier | Visibility |
|----------|------------|
| (none) | Private to current module and descendants |
| `pub(super)` | Visible to parent module |
| `pub(crate)` | Visible anywhere in the crate |
| `pub` | Fully public (part of crate's API) |

**Principle: Minimize visibility.** Once public, you can't make it private without breaking changes. Start private, expose only what's needed.

```rust
// Internal helper - visible within crate, not to users
pub(crate) fn internal_helper() { }

// Public struct with mixed visibility fields
pub struct User {
    pub name: String,           // Public API
    pub(crate) internal_id: u64, // Internal use only
    password_hash: String,       // Private
}
```

### Re-exports flatten API surface

Hide internal module structure while exposing clean public APIs:

```rust
// lib.rs
mod internal;
mod types;

// Users see these at crate root, not nested in modules
pub use types::{Config, Error, Result};
pub use internal::process;

// Use #[doc(hidden)] for items that must be public but shouldn't be documented
#[doc(hidden)]
pub mod __private { }
```

### Workspace organization for large projects

```toml
# Root Cargo.toml (virtual manifest)
[workspace]
resolver = "2"
members = ["crates/*"]

[workspace.dependencies]
serde = { version = "1.0", features = ["derive"] }
tokio = { version = "1", features = ["full"] }

[workspace.lints.clippy]
all = { level = "warn", priority = -1 }
```

Member crates inherit shared configuration:

```toml
# crates/my-lib/Cargo.toml
[package]
name = "my-lib"

[dependencies]
serde.workspace = true

[lints]
workspace = true
```

## Cargo and tooling configuration

### Essential Cargo.toml patterns

```toml
[package]
name = "my-crate"
version = "0.1.0"
edition = "2024"
rust-version = "1.75"    # Minimum Supported Rust Version
license = "MIT OR Apache-2.0"

[lib]
name = "my_crate"
path = "src/lib.rs"

[features]
default = ["std"]
std = []
serde = ["dep:serde"]    # Optional dependency activation

[dependencies]
serde = { version = "1.0", features = ["derive"], optional = true }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }

[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }

[profile.release]
lto = "thin"             # Link-time optimization
codegen-units = 1        # Better optimization
strip = true             # Smaller binaries
```

### Clippy configuration

Configure lints in `Cargo.toml` for team consistency:

```toml
[lints.clippy]
all = { level = "warn", priority = -1 }
pedantic = { level = "warn", priority = -1 }
correctness = { level = "deny", priority = -1 }

# Specific lints
unwrap_used = "warn"
expect_used = "warn"
dbg_macro = "warn"
todo = "warn"
missing_errors_doc = "warn"
missing_panics_doc = "warn"

# Allow some pedantic lints that are too strict
module_name_repetitions = "allow"
must_use_candidate = "allow"
```

Run clippy in CI with warnings as errors: `cargo clippy -- -D warnings`

### rustfmt configuration

Create `rustfmt.toml` for consistent formatting:

```toml
edition = "2024"
max_width = 100
tab_spaces = 4
imports_granularity = "Module"
group_imports = "StdExternalCrate"
format_code_in_doc_comments = true
```

### Security auditing with cargo-deny

```toml
# deny.toml
[advisories]
vulnerability = "deny"
unmaintained = "warn"

[licenses]
unlicensed = "deny"
allow = ["MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause"]

[bans]
multiple-versions = "warn"
deny = [{ name = "openssl" }]  # Prefer rustls
```

## Testing patterns

### Unit tests organization

```rust
pub fn add(a: i32, b: i32) -> i32 { a + b }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_positive() {
        assert_eq!(add(2, 3), 5);
    }

    #[test]
    #[should_panic(expected = "overflow")]
    fn test_overflow() {
        // Test panic conditions
    }

    #[test]
    fn test_with_result() -> Result<(), String> {
        // Tests can return Result for cleaner error handling
        if add(2, 2) == 4 { Ok(()) } else { Err("wrong".into()) }
    }
    
    #[test]
    #[ignore]  // Skip normally, run with: cargo test -- --ignored
    fn expensive_test() { }
}
```

### Doc tests serve as both documentation and tests

```rust
/// Divides two numbers.
///
/// # Examples
///
/// ```
/// use my_crate::divide;
/// assert_eq!(divide(10, 2), Ok(5));
/// ```
///
/// ```should_panic
/// use my_crate::divide;
/// divide(1, 0).unwrap();  // Panics on division by zero
/// ```
///
/// # Errors
///
/// Returns `DivisionError::DivideByZero` if `b` is zero.
pub fn divide(a: i32, b: i32) -> Result<i32, DivisionError> { }
```

### Property-based testing with proptest

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_reverse_twice_is_identity(input: Vec<i32>) {
        let reversed: Vec<_> = input.iter().rev().rev().cloned().collect();
        prop_assert_eq!(input, reversed);
    }

    #[test]
    fn test_parse_positive_numbers(n in 1..1000i32) {
        let s = n.to_string();
        prop_assert_eq!(s.parse::<i32>().unwrap(), n);
    }
}
```

### Benchmark testing with Criterion

```rust
// benches/my_benchmark.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_function(c: &mut Criterion) {
    c.bench_function("my_function", |b| {
        b.iter(|| my_function(black_box(1000)))
    });
}

criterion_group!(benches, bench_function);
criterion_main!(benches);
```

## Async and concurrency patterns

### Decision framework for async vs sync

| Use case | Recommendation | Rationale |
|----------|----------------|-----------|
| Many concurrent network connections | Async (Tokio) | Scales efficiently to thousands |
| Parallel CPU computation | Rayon | Designed for data parallelism |
| Blocking database/file operations | `spawn_blocking` | Avoids blocking async runtime |
| Simple single request | Sync | No async overhead needed |

### Core async principles

**Futures are lazy**â€”they don't execute until awaited. **Never block in async code** for more than 10-100 microseconds between `.await` points.

```rust
#[tokio::main]
async fn main() {
    // Concurrent execution with join!
    let (a, b) = tokio::join!(fetch_data(), fetch_config());
    
    // First-completed with select!
    tokio::select! {
        result = operation_a() => handle_a(result),
        result = operation_b() => handle_b(result),
    }
}

// Spawning independent tasks
let handle = tokio::spawn(async move {
    // Task must own its data ('static) and be Send
    process_data(data).await
});
let result = handle.await?;
```

### Blocking operations in async code

```rust
// BAD: blocks the runtime
std::thread::sleep(Duration::from_secs(1));

// GOOD: yields to runtime
tokio::time::sleep(Duration::from_secs(1)).await;

// GOOD: run blocking I/O on dedicated thread pool
let data = tokio::task::spawn_blocking(|| {
    std::fs::read("large_file.txt")
}).await??;

// GOOD: CPU-heavy work with Rayon
async fn parallel_compute(data: Vec<i32>) -> i32 {
    let (tx, rx) = tokio::sync::oneshot::channel();
    rayon::spawn(move || {
        let result: i32 = data.par_iter().sum();
        let _ = tx.send(result);
    });
    rx.await.unwrap()
}
```

### Channel selection guide

| Channel | Producers | Consumers | Use case |
|---------|-----------|-----------|----------|
| `tokio::sync::mpsc` | Multiple | Single | Async task communication |
| `tokio::sync::broadcast` | Single | Multiple | Pub/sub patterns |
| `tokio::sync::oneshot` | Single | Single | Single response |
| `tokio::sync::watch` | Single | Multiple | Latest-value sharing |
| `crossbeam-channel` | Multiple | Multiple | High-performance sync |

### Mutex patterns in async code

**Use `std::sync::Mutex`** for short-lived locks that don't span `.await` points:

```rust
let data = Arc::new(std::sync::Mutex::new(HashMap::new()));
let value = {
    let guard = data.lock().unwrap();
    guard.get("key").cloned()
};  // Lock released before any .await
```

**Use `tokio::sync::Mutex`** only when you must hold the lock across `.await`:

```rust
let data = Arc::new(tokio::sync::Mutex::new(vec![]));
let mut guard = data.lock().await;
guard.push(fetch_item().await);  // Safe: async mutex
```

### Send and Sync requirements

Tasks spawned on Tokio must be `Send`. Data held across `.await` points must be `Send`.

```rust
// ERROR: Rc is not Send
tokio::spawn(async {
    let rc = Rc::new("hello");
    yield_now().await;  // rc held across .await
});

// FIX: Use Arc, or ensure non-Send values don't cross .await
tokio::spawn(async {
    let arc = Arc::new("hello");
    yield_now().await;
    println!("{}", arc);
});
```

## Performance optimization patterns

### Zero-cost abstractions and iterators

Iterators often outperform indexed loops because they eliminate bounds checks. The compiler can prove "access safety" at compile time.

```rust
// Iterator approach - often FASTER than manual loops
let sum: i32 = numbers.iter()
    .filter(|&&x| x % 2 == 0)
    .map(|&x| x * x)
    .sum();

// Single pass, no intermediate allocations
let result: Vec<_> = data.iter()
    .filter_map(|x| x.parse::<i32>().ok())
    .collect();
```

### String handling optimization

| Type | Ownership | Use case |
|------|-----------|----------|
| `&str` | Borrowed | Read-only string data |
| `String` | Owned | Mutable, heap-allocated |
| `Cow<'a, str>` | Either | Defer allocation until modification |

```rust
use std::borrow::Cow;

fn normalize_text(input: &str) -> Cow<'_, str> {
    if input.contains('\t') {
        // Only allocate when modification needed
        Cow::Owned(input.replace('\t', "    "))
    } else {
        // Zero-cost - just returns reference
        Cow::Borrowed(input)
    }
}
```

### Collection best practices

- **Pre-allocate** when size is known: `Vec::with_capacity(n)`
- **Avoid `Vec<Vec<_>>`** for matricesâ€”use flat arrays with index calculation
- **Avoid `LinkedList`**â€”`Vec` is almost always faster (10x+ for traversal)
- **Handle 0, 1, 2 element cases specially** when optimization matters

### Release build configuration

```toml
[profile.release]
opt-level = 3           # Max optimization
lto = "fat"             # Link-time optimization (10-20% speedup)
codegen-units = 1       # Better optimization, slower compile
panic = "abort"         # Smaller binary, no unwinding
strip = "symbols"       # Remove debug symbols
```

For minimum binary size, use `opt-level = "z"` instead.

## FFI and unsafe code guidelines

### When unsafe is necessary

Unsafe Rust provides five capabilities: dereferencing raw pointers, calling unsafe functions, accessing mutable statics, implementing unsafe traits, and accessing union fields.

**Legitimate uses**: C library interop, performance-critical manual memory management, hardware abstractions, and low-level data structures.

### Safe abstractions over unsafe

**Wrap unsafe code in safe APIs**. The unsafe block is an implementation detail; the public API should be safe.

```rust
pub struct SafeBuffer {
    ptr: *mut u8,
    len: usize,
}

impl SafeBuffer {
    pub fn new(size: usize) -> Self {
        let ptr = unsafe { libc::malloc(size) as *mut u8 };
        Self { ptr, len: size }
    }
    
    // Safe public API hiding unsafe implementation
    pub fn write(&mut self, offset: usize, data: &[u8]) -> Result<(), Error> {
        if offset + data.len() > self.len {
            return Err(Error::OutOfBounds);
        }
        unsafe {
            std::ptr::copy_nonoverlapping(
                data.as_ptr(),
                self.ptr.add(offset),
                data.len()
            );
        }
        Ok(())
    }
}

impl Drop for SafeBuffer {
    fn drop(&mut self) {
        unsafe { libc::free(self.ptr as *mut libc::c_void); }
    }
}
```

### C interop patterns

```rust
// Use #[repr(C)] for FFI structs
#[repr(C)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

// Declare external C functions
unsafe extern "C" {
    fn c_function(x: libc::c_int) -> libc::c_int;
}

// Expose Rust to C with #[no_mangle]
#[no_mangle]
pub extern "C" fn rust_function(x: i32) -> i32 {
    x * 2
}

// String handling at FFI boundaries
use std::ffi::{CStr, CString};

// Rust -> C
let c_string = CString::new("hello").unwrap();
unsafe { c_function(c_string.as_ptr()); }

// C -> Rust
unsafe {
    let rust_str = CStr::from_ptr(c_pointer).to_str().unwrap();
}
```

**Convention**: Create `foo-sys` crate for raw bindings, `foo` crate for safe Rust wrapper.

## Application-specific patterns

### CLI tools with clap

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "myapp", version, about)]
struct Cli {
    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,
    
    /// Config file path
    #[arg(short, long, env = "MYAPP_CONFIG")]
    config: Option<std::path::PathBuf>,
    
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Process input files
    Process {
        #[arg(required = true)]
        files: Vec<std::path::PathBuf>,
    },
    /// Show status
    Status,
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Process { files } => process_files(&files, cli.verbose),
        Commands::Status => show_status(),
    }
}
```

**Use `PathBuf`** for file argumentsâ€”not all paths are valid UTF-8.

### Web services with axum

```rust
use axum::{
    routing::{get, post},
    extract::{State, Json, Path},
    http::StatusCode,
    Router,
};
use std::sync::Arc;

#[derive(Clone)]
struct AppState {
    db: DatabasePool,
}

#[tokio::main]
async fn main() {
    let state = Arc::new(AppState { db: create_pool().await });
    
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/users", post(create_user))
        .route("/users/:id", get(get_user))
        .with_state(state);
    
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn get_user(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u64>,
) -> Result<Json<User>, StatusCode> {
    state.db.find_user(id).await
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn create_user(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateUserRequest>,
) -> (StatusCode, Json<User>) {
    let user = state.db.create_user(payload).await;
    (StatusCode::CREATED, Json(user))
}
```

### Systems programming patterns

```rust
// Memory-mapped files for large file processing
use memmap2::Mmap;
use std::fs::File;

fn process_large_file(path: &str) -> std::io::Result<()> {
    let file = File::open(path)?;
    let mmap = unsafe { Mmap::map(&file)? };
    // Access file as byte slice - lazy loading, no full copy
    process_bytes(&mmap[..]);
    Ok(())
}

// Signal handling
use signal_hook::{consts::SIGINT, iterator::Signals};

fn setup_signal_handler() -> std::io::Result<()> {
    let mut signals = Signals::new(&[SIGINT])?;
    std::thread::spawn(move || {
        for _ in signals.forever() {
            println!("Shutting down...");
            std::process::exit(0);
        }
    });
    Ok(())
}
```

## Common anti-patterns to avoid

### Ownership and borrowing anti-patterns
- **Fighting the borrow checker** â€” Restructure code to work with ownership, not against it
- **Overusing `.clone()`** â€” Prefer references when possible; profile before adding clones
- **Using `Rc` in multi-threaded code** â€” Use `Arc` instead

### Error handling anti-patterns
- **Using `()` as error type** â€” Provides no information about what went wrong
- **`unwrap()` everywhere** â€” Handle errors properly with `?` or use `expect()` with explanation
- **Ignoring errors silently** â€” `let _ = fallible_operation();` discards important information
- **Panic in library code** â€” Let callers decide how to handle expected failures

### Async anti-patterns
- **Blocking in async** â€” Use `tokio::time::sleep`, not `std::thread::sleep`
- **Holding `std::sync::Mutex` across `.await`** â€” Use `tokio::sync::Mutex` or restructure
- **Forgetting futures are lazy** â€” `async_fn();` does nothing; must `.await`
- **Non-Send types across `.await`** â€” Use thread-safe alternatives (`Arc` not `Rc`)

### Performance anti-patterns
- **`Vec<Vec<_>>` for matrices** â€” Use flat array with index calculation
- **`LinkedList`** â€” Almost always slower than `Vec`
- **Indexed loops when iterators work** â€” Iterators eliminate bounds checks
- **Benchmarking debug builds** â€” Always use `--release`

### Module organization anti-patterns
- **Making everything `pub`** â€” Start private, expose only what's needed
- **Glob imports in production** â€” `use some_crate::*` causes unclear provenance
- **Giant monolithic modules** â€” Split into focused submodules

## CI/CD pipeline essentials

A complete Rust CI pipeline should include:

```yaml
# GitHub Actions example
jobs:
  check:
    steps:
      - run: cargo fmt --check
      - run: cargo clippy -- -D warnings
      - run: cargo test --all-features
      - run: cargo doc --no-deps
        env:
          RUSTDOCFLAGS: -Dwarnings
      - run: cargo deny check  # Security/license audit
```

Use `Swatinem/rust-cache` for dependency caching. Consider `cargo-nextest` for **3x faster** test execution. Run MSRV checks to verify minimum supported Rust version.

## Conclusion

This document provides a comprehensive reference for idiomatic Rust development. The key principles are: leverage the ownership system rather than fighting it, prefer borrowing over cloning, use the type system to encode invariants, handle errors explicitly with appropriate granularity, minimize visibility and API surface, and choose async vs sync based on workload characteristics. When assisting developers, prioritize correctness and clarity first, then optimize for performance only when profiling indicates a need.

