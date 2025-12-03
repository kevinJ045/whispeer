# Project: Rust Encrypted Pub/Sub Broker (whispeer)

## Project Overview
A Rust-native, lightweight, end-to-end **encrypted Pub/Sub broker** supporting:

- Multiple transports: TCP and WebSocket
- Compression and encryption engines
- Typed, async-aware publish/subscribe API
- Plugin system for custom behaviors
- Optional per-topic or global engines

This project targets **secure messaging, real-time applications, and extensible event-driven systems**.

---

## Core Features
1. **Broker Core**
   - Topic registry
   - Subscriber management
   - Publish/subscribe API
2. **Compression Engine**
   - Default: zstd
   - Optional: other compression methods
3. **Encryption Engine**
   - Default: ChaCha20Poly1305 / AES-GCM
   - End-to-end encryption per topic
4. **WebSocket Transport**
   - Optional TCP/WS transport
   - Automatic compression/encryption integration
5. **Async Support**
   - Tokio runtime ready
   - Async subscribers
6. **Plugin System**
   - Logging, persistence, metrics, custom transforms
7. **Typed Messages**
   - Support generic message types via `serde::Serialize/Deserialize`

---

## Milestones & Tasks

### Milestone 1: Core Broker MVP
**Goal:** Functional in-memory pub/sub with simple API

**Tasks:**
- [ ] Define `Broker`, `Topic`, `Subscriber` structs
- [ ] Implement basic `subscribe(topic, callback)`
- [ ] Implement basic `publish(topic, message)`
- [ ] Add simple synchronous subscribers
- [ ] Write tests for publish/subscribe functionality

---

### Milestone 2: Async + Tokio Integration
**Goal:** Async subscriber support with Tokio

**Tasks:**
- [ ] Make subscription callbacks async
- [ ] Support `async move` closures for subscribers
- [ ] Ensure thread-safe broker operations using `Arc<Mutex<>>` or `RwLock`
- [ ] Write async tests for concurrent publish/subscribe

---

### Milestone 3: Compression Engine
**Goal:** Add transparent compression support
**Example**:
```rust
let broker = Broker::new().with(Compression::new());
```

**Tasks:**
- [ ] Integrate `zstd` or `lz4` compression
- [ ] Automatically compress messages before sending
- [ ] Decompress on the subscriber side
- [ ] Add tests for compression correctness and performance
- [ ] Optional: allow per-topic compression settings

---

### Milestone 4: Encryption Engine
**Goal:** End-to-end encrypted messaging

**Tasks:**
- [ ] Integrate `chacha20poly1305` or `aes-gcm` encryption
- [ ] Automatic encryption/decryption for publishers/subscribers
- [ ] Support per-topic or global keys
- [ ] Write tests to verify message confidentiality

---


### Milestone 5: Plugin System
**Goal:** Allow extensibility via plugins

**Tasks:**
- [ ] Define `Plugin` trait with hooks (`on_publish`, `on_subscribe`)
- [ ] Implement default logging plugin
- [ ] Implement example persistence plugin (in-memory, file, or SQLite)
- [ ] Implement trait enforcement system for message types
- [ ] Write tests for plugin system integration

---

### Milestone 6: WebSocket Transport
**Goal:** Real-time messaging over WS as plugin

**Tasks:**
- [ ] Implement WebSocket listener using `tokio-tungstenite`
- [ ] Integrate broker with WebSocket transport
- [ ] Support both async and sync subscribers over WS
- [ ] Ensure compression + encryption works over WS
- [ ] Write WebSocket integration tests

---

### Milestone 7: Typed & Generic Messages
**Goal:** Make broker type-safe with Rust generics

**Tasks:**
- [ ] Support `publish::<T>(topic, data)`
- [ ] Ensure subscribers get correctly typed messages
- [ ] Verify `serde::Serialize/Deserialize` integration
- [ ] Write tests for multiple types

---

### Milestone 8: Stretch Goals / Optional Features
- [ ] Topic wildcards (`"chat/*"` or `"sensor/+/temperature"`)
- [ ] Message persistence and replay
- [ ] QoS options: `at_least_once`, `exactly_once`
- [ ] CLI tool to inspect broker status (topics, subscribers)
- [ ] Rust-native client library for browser/Node.js via WASM
- [ ] Benchmarking and performance optimization

---

## Project Structure (Proposed)

```
src/
├── broker/
│ ├── mod.rs
│ ├── broker.rs # Broker core
│ ├── topic.rs # Topic registry
│ └── subscriber.rs # Subscriber management
├── transport/
│ ├── mod.rs
│ ├── websocket.rs # WS transport
│ └── tcp.rs # TCP transport (optional)
├── engines/
│ ├── mod.rs
│ ├── compression.rs # Compression engine
│ └── encryption.rs # Encryption engine
├── plugins/
│ ├── mod.rs
│ └── logging.rs
├── types/
│ └── message.rs # Typed messages
└── main.rs
```
---

## Tech Stack
- Rust 1.70+
- Tokio for async runtime
- `serde` for typed messages
- `tokio-tungstenite` for WebSocket support
- `zstd` or `lz4` for compression
- `chacha20poly1305` or `aes-gcm` for encryption
- `rsub` for a high-performance pub/sub message broker base using QUIC and built-in TLS 1.3 encryption.
- Whatever else is needed

---

## Simple example

```rust
let broker = Broker::builder(BrokerOptions {})
  .with(Encryption::new())
  .with(Compression::new())
  .with(WebSocket::new("http://localhost:8080"))
  .with(Persistence::sqlite(Path::new("./path/to/db")))
  .build();
// OR
// Only Compression and Encryption
let broker = Broker::build_default().build();
// OR
// With WebSocket and defaults
let broker = Broker::build_default_with_ws("http://localhost:8080").build();
// OR
// With Persistence and defaults
let broker = Broker::build_default_with_persistence(PersistenceType::Sqlite(Path::new("./path/to/db"))).build();
// OR
// With Persistence, WebSocket and defaults
let broker = Broker::build_default_with_persistence_ws("http://localhost:8080", PersistenceType::Sqlite(Path::new("./path/to/db"))).build();

// --- Use:

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct MyData {
  name: String,
  id: i32
}

// Sub:

broker.subscribe("some-event", |my_data: MyData| println!("{my_data:?}"))
broker.subscribe::<MyData>("some-event", |my_data| println!("{my_data:?}"))


// Pub:

let my_data = MyData {
  name: "myname",
  id: 123
}

broker.publish("some-event", my_data)
broker.publish::<MyData>("some-event", my_data)
```

### Example for the message type:
```rust
trait Message: Send + Sync {}
impl<T: Send + Sync> Message for T {}
```


---

## Timeline (Suggested MVP)
| Week | Goal |
|------|------|
| 1-2  | Milestone 1-2: Core broker + async support |
| 3-4  | Milestone 3: Compression |
| 5-6  | Milestone 4: Encryption |
| 7-8  | Milestone 5: WebSocket transport |
| 9    | Milestone 6: Plugins |
| 10   | Milestone 7: Typed messages + cleanup |
| 11+  | Stretch goals, benchmarking, documentation |

---

## Next Steps
1. Start with **Milestone 1 MVP**, purely in-memory pub/sub.
2. Make **API ergonomic** and type-safe.
3. Gradually integrate **async, compression, encryption, and WS**.
4. Expand with **plugins and advanced features** once core stability is proven.
