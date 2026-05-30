//! # ductor
//!
//! so this is ductor: a zero-cost rust typestate library for modeling stateful
//! "units" with capability-driven transitions that get checked at compile time.
//! so here's the deal: you describe your state machine as an enum, declare
//! what transitions are valid (and what capabilities they need), and the compiler
//! handles the rest. invalid transitions just don't compile
//!
//! ## quick start
//!
//! ```rust
//! use ductor::*;
//!
//! // step 1: define a state machine as an enum.
//! // each variant becomes a struct, the enum name becomes the family marker.
//! #[typestate(derive(Debug))]
//! pub enum Door {
//!   #[transition(Closed)]
//!   Open,
//!   #[transition(Open)]
//!   Closed,
//! }
//!
//! // step 2: define a capability (optional, only if transitions need caps).
//! #[cap(derive(Debug))]
//! pub struct HasKey;
//!
//! // step 3: define a unit, the typed container that holds state + caps.
//! #[unit(derive(Debug))]
//! pub struct Lock<State, Caps> {
//!   pub state: State,
//!   pub caps: Caps,
//! }
//!
//! // step 4: go wild. wrong transitions won't compile.
//! let lock = Lock::new(Closed, HasKey)
//!   .transition(|Closed| Open)
//!   .transition(|Open| Closed);
//! # let _ = lock;
//! ```
//!
//! ## what's here
//!
//! | attribute | what it does |
//! |---|---|
//! | `#[typestate]` | turns an enum into a state machine |
//! | `#[unit]` | creates a typed container for states + caps |
//! | `#[cap]` | marks a struct as a capability |
//! | `#[spec]` | attaches methods to specific state/cap combos |
//! | `#[transit]` | marks a method inside `#[spec]` as doing a transition |
//!
//! ## the big ideas
//!
//! - **state / statefamily**: a state machine is an enum (the family). each
//!   variant becomes a state struct that can carry data.
//! - **unit**: a struct you define -- the macro adds `state: State` and `caps: Caps`
//!   automatically and generates impls so transitions and cap checks happen at the type level.
//! - **capability**: a marker type a unit carries. transitions and `#[spec]`
//!   blocks can require caps then the compiler checks them all.
//! - **tuple states**: one unit can manage multiple state machines at once
//!   using `States<(A, B, ...)>`. use `transition_at()` to transition one by one.
//!
//! ## examples
//!
//! check the `examples/` directory:
//! - `simple` - tiny door lock, minimal setup
//! - `base` - multi-state network service with the whole kitchen sink
//! - `state_data` - states with data fields flowing through transitions
//! - `capabilities` - gating operations behind capability checks
//! - `multi_unit` - composing multiple units together
//! - `branching` - one state branching to many targets
//! - `proxy_cap` - parent caps that proxy for their children

pub use ductor_core::*;
pub use ductor_macros::*;
