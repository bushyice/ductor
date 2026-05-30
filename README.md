# ductor

[![crates.io][crates-badge]][crates-url]

[crates-badge]: https://img.shields.io/crates/v/ductor.svg
[crates-url]: https://crates.io/crates/ductor

so, ductor is a zero-cost rust typestate library for modeling stateful "units" with capability-driven transitions that get checked at compile time. 

the idea is you use proc macros to describe what transitions are valid and what capabilities they need, and then the compiler rejects any invalid stuff before it even runs. no runtime panics from being in the wrong state, no missing-capability bugs... that kind of thing

> **ductus** (latin) -- to lead, to conduct, to guide. i thought it fit.

## quick start

add ductor to your cargo.toml:

```toml
[dependencies]
ductor = "0.1"
```

then you define a state machine, a capability, and a unit:

```rust
use ductor::*;

#[typestate(derive(Debug))]
pub enum Door {
  #[transition(Closed)]
  Open,
  #[transition(Open)]
  Closed,
}

#[cap(derive(Debug))]
pub struct HasKey;

#[unit(derive(Debug))]
pub struct Lock;

// try adding a transition that doesn't exist, watch it fail
let lock = Lock::new(Closed, HasKey)
  .transition(|Closed| Open)
  .transition(|Open| Closed);
```

## ok but what's the point

the whole idea is compile-time state machines. you describe your states and transitions once, and the compiler tracks what state everything is in. methods only show up when you're in the right state, transitions only work when you have the right capabilities. it's all types, zero cost.

### typestate pattern

each state is its own type. you can only move between them if the transition is declared. ductor generates all the boilerplate from a simple enum.

```rust
#[typestate]
pub enum Connection {
  #[transition(Connected, requires = Tls)]
  Disconnected,
  Connected { addr: SocketAddr },
}
```

this gives you:
- structs `Disconnected` and `Connected` (with `IsState` impls)
- a family marker `Connection` (implements `StateFamily`)
- a `Transition<Disconnected, Connected>` impl with `Requirements = Tls`

try calling `.transition(|Disconnected| ...)` with a target like `Listening` and the compiler just says no. it's pretty satisfying honestly

### capabilities

capabilities are little marker types your unit carries around. transitions and `#[spec]` blocks can declare what they need, and the compiler checks everything.

```rust
#[cap(derive(Debug))]
pub struct AdminPrivilege;

// this only compiles if your caps include AdminPrivilege:
let area = SecureArea::new(Restricted, caps!(AdminPrivilege))
  .transition(|Restricted| Open);
```

you can also require multiple caps at once with a tuple:

```rust
#[transition(Open, requires = (ReadCap, WriteCap))]
ReadOnly,
```

and with `#[cap(as = (A, B, C))]` you can have parent caps that proxy for others. so `caps!(Admin)` can satisfy requirements for `Read`, `Write`, and `Delete` all at once. neat huh? :)

### tuple states

a single unit can manage multiple independent state machines at the same time using tuple states:

```rust
#[unit(states = (NetworkState, AuthState))]
pub struct MyService;
```

you use `states!(...)` to create the initial tuple and `transition_at()` to transition just one of them:

```rust
let svc = MyService::new(states!(NetworkDown, Guest), caps)
  .up(network)                    // NetworkDown -> NetworkUp
  .authenticate("user");          // Guest -> Authenticated
```

### spec blocks

`#[spec]` blocks let you attach methods to specific state (and capability) combos. methods just don't exist when you're not in the right state, the compiler just hides them.

```rust
#[spec(for = Connected, with = Tls)]
impl Connection {
  pub fn send(&self, data: &[u8]) { /* ... */ }
}

// `send()` only exists on Connection<Connected, Caps<(Tls,)>>:
conn.send(b"hello"); // works
```

use `_` or `()` as wildcards for "any state" or "any cap".

## macro reference

### `#[typestate]`

defines a state machine from an enum. each variant becomes a struct.

| argument | description |
|----------|-------------|
| `derive(Trait, ...)` | forwards derives to all generated structs |

**variant attributes:**

| attribute | description |
|-----------|-------------|
| `#[transition(Target)]` | declares a valid transition to `Target` |
| `#[transition(Target, requires = Type)]` | same but requires a capability |

### `#[cap]`

marks a struct as a capability type. with `as = (A, B, ...)` it acts as a parent cap, having it satisfies requirement bounds for a, b, etc.

| argument | description |
|----------|-------------|
| `derive(Trait, ...)` | forwards derives |
| `as = Type` or `as = (Type, ...)` | proxy targets this cap satisfies |

### `#[unit]`

generates a typed container with `new()`, `transition()`, and `transition_at()`.

| argument | description |
|----------|-------------|
| `derive(Trait, ...)` | forwards derives |
| `states = Type` | constrains the state generic to a single family |
| `states = (A, B, ...)` | constrains to multiple families |

the macro adds `state: State` and `caps: Caps` fields + generics automatically. if your struct has extra fields annotate how they're initialized:

| annotation | behavior |
|------------|----------|
| `#[unit(default)]` | `Default::default()` (this is the default if omitted) |
| `#[unit(take)]` | added as a parameter to `new()` |
| `#[unit(construct = expr)]` | computed via the given expression (can reference `state`, `caps`, and earlier fields) |

```rust
#[unit(derive(Debug))]
pub struct Service {
  #[unit(default)]
  url: String,
  #[unit(take)]
  port: u16,
  #[unit(construct = format!("{url}:{port}"))]
  addr: String,
}
```

### `#[spec]`

attaches methods to a specific state/capability combo.

| argument | description |
|----------|-------------|
| `for = Type` | state type (or tuple) this applies to |
| `for = (A, B, ...)` | tuple state spec; `_` or `()` = wildcard |
| `with = Type` | required capability |
| `with = (A, B, ...)` | multiple required caps |

### `#[transit]`

used inside `#[spec]` blocks to mark a method as a state transition. rewrites the return type to reflect the new state/caps.

| argument | description |
|----------|-------------|
| `to = Type` | target state (or tuple; `_`/`()` = wildcard) |
| `with = Type` | target caps (optional) |

## examples

run any with `cargo run --example <name>`.

| example | file | what it shows |
|---------|------|---------------|
| `simple` | `examples/simple.rs` | the bare minimum door lock thing |
| `base` | `examples/base.rs` | multi-state network service with spec blocks, the works |
| `state_data` | `examples/state_data.rs` | states that carry actual data fields through a workflow |
| `capabilities` | `examples/capabilities.rs` | gating operations behind capabilities, compile-time checks |
| `multi_unit` | `examples/multi_unit.rs` | composing multiple units into a bigger state machine |
| `branching` | `examples/branching.rs` | one-to-many transitions from a single source state |
| `proxy_cap` | `examples/proxy_cap.rs` | parent capability delegation with `#[cap(as = ...)]` |
