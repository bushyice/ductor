//! testing #[unit(default)], #[unit(take)], #[unit(construct = ...)] on custom fields.

use ductor::*;

fn make_port() -> u16 {
  8080
}

#[typestate(derive(Debug, Clone, Copy))]
pub enum Switch {
  #[transition(On)]
  Off,
  #[transition(Off)]
  On,
}

#[unit(derive(Debug))]
pub struct Server {
  #[unit(default)]
  host: String,
  #[unit(construct = make_port())]
  port: u16,
}

#[unit(derive(Debug))]
pub struct Router {
  #[unit(take)]
  address: String,
  #[unit(construct = address.len())]
  port: usize,
}

fn main() {
  // host uses Default (""), port uses construct expression
  let s = Server::new(Off, caps!());
  println!("Server: {s:?}");

  // address is taken as param, port is computed from earlier-declared `address`
  let r = Router::new(Off, caps!(), "10.0.0.1".into());
  println!("Router: {r:?}");
}
