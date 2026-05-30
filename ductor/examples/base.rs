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

// `states = ConnectionState` says the State generic must be a single state
// from the ConnectionState family.
#[unit(derive(Debug, Clone), states = ConnectionState)]
pub struct Network<S, C> {
  pub state: S,
  pub caps: C,
}

// methods available when Network is Disconnected and we've got NetCap.
#[spec(for = Disconnected, with = NetCap)]
impl Network {
  // `#[transit(to = Connected)]` rewrites the return type so the caller
  // sees the new state in the type system.
  #[transit(to = Connected)]
  pub fn connect(self, addr: &str) {
    self.transition(|_| Connected {
      addr: addr.parse().unwrap(),
    })
  }
}

// `states = (NetworkState, AuthState)` means position 0 is from NetworkState
// family and position 1 from AuthState family.
#[unit(derive(Debug, Clone), states = (NetworkState, AuthState))]
pub struct MyService<S, C> {
  pub state: S,
  pub caps: C,
}

// spec: NetworkDown, any AuthState, needs NetCap.
// `()` is a wildcard for "any state".
#[spec(for = (NetworkDown, ()), with = NetCap)]
impl MyService {
  // `to = (NetworkUp, ())` -- `()` wildcard means keep the existing auth state.
  #[transit(to = (NetworkUp, ()))]
  pub fn up(self, network: Network<Connected, NetCap>) {
    self.transition_at(|_| NetworkUp { network })
  }
}

// spec: NetworkUp + any AuthState, no cap needed.
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

// spec: NetworkUp + Guest, needs both NetCap and AuthCap.
#[spec(for = (NetworkUp, Guest), with = (NetCap, AuthCap))]
impl MyService {
  // `to = ((), Authenticated)` -- `()` keeps NetworkUp unchanged.
  #[transit(to = ((), Authenticated))]
  pub fn authenticate(self, user: impl Into<String>) {
    self.transition_at(|_| Authenticated { user: user.into() })
  }
}

// spec: NetworkUp + Authenticated, needs AuthCap.
#[spec(for = (NetworkUp, Authenticated), with = AuthCap)]
impl MyService {
  pub fn get_user(&self) -> &String {
    &self.state.select(AuthState).user
  }
}

// spec: NetworkUp + Authenticated, needs NetCap.
// multiple `#[spec]` blocks for the same state is totally fine.
#[spec(for = (NetworkUp, Authenticated), with = NetCap)]
impl MyService {
  pub fn ping(&self) {
    println!("Pinging with NetCap only...");
  }
}

fn main() {
  let caps = caps!(NetCap, AuthCap);

  // build a chain: create service -> bring up network -> authenticate.
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
