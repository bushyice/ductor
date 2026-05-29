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
pub enum NetworkState {
  #[transition(NetworkUp, requires = NetCap)]
  NetworkDown,

  NetworkUp {
    network: Network<Connected, NetCap>,
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

#[unit(derive(Debug, Clone), states = (NetworkState, AuthState))]
pub struct MyService<S, C> {
  pub state: S,
  pub caps: C,
}

#[spec(for = (NetworkDown, ()), with = NetCap)]
impl MyService {
  #[transit(to = (NetworkUp, ()))]
  pub fn up(self, network: Network<Connected, NetCap>) {
    self.transition_at(|_| NetworkUp { network })
  }
}

#[spec(for = (NetworkUp, ()))]
impl MyService {
  pub fn get_addr(&self) -> SocketAddr {
    self
      .state
      .get::<NetworkUp, _>()
      .network
      .state
      .select(ConnectionState)
      .addr
  }
}

#[spec(for = (NetworkUp, Guest), with = (NetCap, AuthCap))]
impl MyService {
  #[transit(to = ((), Authenticated))]
  pub fn authenticate(self, user: impl Into<String>) {
    self.transition_at(|_| Authenticated { user: user.into() })
  }
}

#[spec(for = (NetworkUp, Authenticated), with = AuthCap)]
impl MyService {
  pub fn get_user(&self) -> &String {
    &self.state.select(AuthState).user
  }
}

#[spec(for = (NetworkUp, Authenticated), with = NetCap)]
impl MyService {
  pub fn ping(&self) {
    println!("Pinging with NetCap only...");
  }
}

fn main() {
  let caps = caps!(NetCap, AuthCap);

  let service = MyService::new(states!(NetworkDown, Guest), caps)
    .up(Network::new(Disconnected, NetCap).connect("127.0.0.1:8080"))
    .authenticate("user");

  service.ping();

  println!(
    "Service connected address: {}@{}",
    service.get_user(),
    service.get_addr(),
  );
}
