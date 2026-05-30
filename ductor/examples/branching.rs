//! one-to-many transitions where a single state branches to multiple targets.
//!
//! ascii flow:
//!
//!                   ┌──────────┐
//!                   │ Pending  │
//!                   └────┬─────┘
//!                   ┌────┴──────┐
//!              ┌────▼────┐ ┌────▼─────┐
//!              │Confirmed│ │Cancelled │
//!              └────┬────┘ └──────────┘
//!              ┌────┴─────┐
//!         ┌────▼───┐ ┌────▼───┐
//!         │Shipped │ │Returned│
//!         └───┬────┘ └────────┘
//!        ┌────┴──────┐
//!   ┌────▼────┐ ┌────▼───┐
//!   │Delivered│ │  Lost  │
//!   └─────────┘ └────────┘

#![allow(unused)]

use ductor::*;

// caps

#[cap(derive(Debug, Clone))]
pub struct PaymentCap;

#[cap(derive(Debug, Clone))]
pub struct AdminCap;

// order state machine

#[typestate(derive(Debug, Clone))]
pub enum OrderState {
  // pending can go to confirmed (needs payment) OR cancelled (no caps).
  #[transition(Confirmed, requires = PaymentCap)]
  #[transition(Cancelled)]
  Pending {
    item: String,
  },

  // confirmed can go to shipped OR cancelled (needs admin override).
  #[transition(Shipped)]
  #[transition(Cancelled, requires = AdminCap)]
  Confirmed {
    item: String,
  },

  // shipped can go to delivered OR lost.
  #[transition(Delivered)]
  #[transition(Lost)]
  Shipped {
    item: String,
    tracking: String,
  },

  // terminal states, no way out
  Cancelled {
    item: String,
  },
  Delivered {
    item: String,
  },
  Lost {
    item: String,
  },
}

// the unit

#[unit(derive(Debug, Clone), states = OrderState)]
pub struct Order;

// spec'd methods

#[spec(for = Pending, with = PaymentCap)]
impl Order {
  #[transit(to = Confirmed)]
  pub fn confirm(self) {
    self.transition(|Pending { item }| Confirmed { item })
  }
}

// cancelling from pending doesn't need any caps, so no `with =`.
#[spec(for = Pending)]
impl Order {
  #[transit(to = Cancelled)]
  pub fn cancel(self) {
    self.transition(|Pending { item }| Cancelled { item })
  }
}

#[spec(for = Confirmed)]
impl Order {
  #[transit(to = Shipped)]
  pub fn ship(self, tracking: &str) {
    self.transition(|Confirmed { item }| Shipped {
      item,
      tracking: tracking.to_string(),
    })
  }
}

#[spec(for = Confirmed, with = AdminCap)]
impl Order {
  #[transit(to = Cancelled)]
  pub fn cancel_with_admin(self) {
    self.transition(|Confirmed { item }| Cancelled { item })
  }
}

#[spec(for = Shipped)]
impl Order {
  #[transit(to = Delivered)]
  pub fn deliver(self) {
    self.transition(|Shipped { item, .. }| Delivered { item })
  }

  #[transit(to = Lost)]
  pub fn report_lost(self) {
    self.transition(|Shipped { item, .. }| Lost { item })
  }
}

fn main() {
  // branch 1: pending -> confirmed -> shipped -> delivered
  let caps = caps!(PaymentCap, AdminCap);
  let delivered = Order::new(
    Pending {
      item: "Laptop".into(),
    },
    caps,
  )
  .confirm() // pending -> confirmed (needs PaymentCap)
  .ship("TRACK-123")
  .deliver(); // shipped -> delivered

  println!("Delivered: {delivered:?}");

  // branch 2: pending -> cancelled (no cap needed at all)
  let caps = caps!();
  let cancelled = Order::new(
    Pending {
      item: "Mouse".into(),
    },
    caps,
  )
  .cancel();

  println!("Cancelled: {cancelled:?}");

  // branch 3: confirmed -> cancelled (needs AdminCap)
  let caps = caps!(PaymentCap, AdminCap);
  let cancelled2 = Order::new(
    Pending {
      item: "Keyboard".into(),
    },
    caps,
  )
  .confirm()
  .cancel_with_admin();

  println!("Admin cancelled: {cancelled2:?}");

  // branch 4: shipped -> lost
  let caps = caps!(PaymentCap, AdminCap);
  let lost = Order::new(
    Pending {
      item: "Monitor".into(),
    },
    caps,
  )
  .confirm()
  .ship("TRACK-456")
  .report_lost();

  println!("Lost: {lost:?}");

  // things that won't compile (uncomment to watch it fail):
  // .confirm() without PaymentCap:
  // Order::new(Pending { item: "X" }, caps!()).confirm();
  //   ^^^^ missing PaymentCap

  // .cancel_with_admin() without AdminCap on a confirmed order:
  // Order::new(Pending { item: "X" }, caps!(PaymentCap))
  //   .confirm().cancel_with_admin();
  //   ^^^^ missing AdminCap
}
