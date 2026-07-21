use ductor::*;

// `wrapped` wraps everything in `mod Door { ... }` with family as `State`
// `family_name` is so you can rename the family struct
#[typestate(wrapped, family_name = "Family", derive(Debug))]
pub enum Door {
  #[transition(Closed)]
  Open,

  #[transition(Open)]
  Closed,
}

#[cap(derive(Debug))]
pub struct HasKey;

#[unit(derive(Debug), states = Door::Family)]
pub struct Lock;

fn main() {
  use Door::Closed;

  let lock = Lock::new(Door::Open, HasKey)
    .transition(|Door::Open| Door::Closed)
    .transition(|Closed| Door::Open);

  println!("Lock: {lock:?}");
}
