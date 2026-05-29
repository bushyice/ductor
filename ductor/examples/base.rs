use ductor::*;
use std::net::SocketAddr;

#[cap(derive(Debug, Clone))]
pub struct NetCap;

#[cap]
pub struct AuthCap;

#[typestate(derive(Debug, Clone, Copy))]
pub enum ConnectionState {
  #[transition(Connected, requires = NetCap)]
  Disconnected,

  Connected {
    addr: SocketAddr,
  },
}

#[typestate(derive(Debug, Clone))]
pub enum AuthState {
  #[transition(Authenticated, requires = (NetCap, AuthCap))]
  Guest,

  Authenticated {
    user: String,
  },
}

#[unit(derive(Debug, Clone), states = ConnectionState)]
pub struct Network<S, C> {
  pub state: S,
  pub caps: C,
}

#[spec(for = Disconnected, with = NetCap)]
impl Network {
  #[transit(to = Connected)]
  pub fn connect(self, addr: &str) {
    self.transition(|_| Connected {
      addr: addr.parse().unwrap(),
    })
  }
}

#[unit(derive(Debug, Clone), states = (ConnectionState, AuthState))]
pub struct MyService<S, C> {
  pub state: S,
  pub caps: C,
}

#[spec(for = (Disconnected, ()), with = NetCap)]
impl MyService {
  #[transit(to = (Connected, ()))]
  pub fn connect(self, addr: &str) {
    self.transition_at(|_| Connected {
      addr: addr.parse().unwrap(),
    })
  }
}

#[spec(for = (Connected, ()), with = NetCap)]
impl MyService {
  pub fn get_addr(&self) -> SocketAddr {
    self.state.get::<Connected, _>().addr
  }
}

#[spec(for = (Connected, Guest), with = (NetCap, AuthCap))]
impl MyService {
  #[transit(to = ((), Authenticated))]
  pub fn authenticate(self, user: impl Into<String>) {
    self.transition_at(|_| Authenticated { user: user.into() })
  }
}

#[spec(for = (Connected, Authenticated), with = AuthCap)]
impl MyService {
  pub fn get_user(&self) -> &String {
    &self.state.select(AuthState).user
  }
}

#[spec(for = (Connected, Authenticated), with = NetCap)]
impl MyService {
  pub fn ping(&self) {
    println!("Pinging with NetCap only...");
  }
}

fn main() {
  let caps = caps!(NetCap, AuthCap);

  let service = MyService::new(states!(Disconnected, Guest), caps)
    .connect("127.0.0.1:8080")
    .authenticate("user");

  service.ping();

  println!(
    "Service connected address: {}@{}",
    service.get_user(),
    service.get_addr(),
  );

  let net = Network::new(Disconnected, NetCap);
  let net = net.connect("127.0.0.1:80");
  println!("Network state: {:?}", net.state);
}
