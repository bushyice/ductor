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

// a unit is a generic container for a state + capabilities.
// must have `state: State` and `caps: Caps` fields.
#[unit(derive(Debug))]
pub struct Lock<State, Caps> {
  pub state: State,
  pub caps: Caps,
}

fn main() {
  let lock = Lock::new(Closed, HasKey)
    .transition(|Closed| Open)
    .transition(|Open| Closed);

  println!("Lock: {lock:?}");
}
