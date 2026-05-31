use ductor::*;

#[typestate(derive(Debug, Clone))]
pub enum Auth {
  LoggedOut,
  LoggedIn,
}

#[typestate(derive(Debug, Clone))]
pub enum Network {
  Disconnected,
  #[transition(Disconnected)]
  Connected,
}

#[unit(derive(Debug), states = (Auth, Network))]
pub struct Service;

#[typestate(derive(Debug, Clone))]
pub enum Door {
  Open,
  Closed,
}

#[unit(derive(Debug))]
pub struct Lock;

fn main() {
  let svc: Service<States<(Of<Auth>, Connected)>, _> =
    Service::new(states!(Of::new(LoggedIn), Connected), caps!());
  println!("mixed: {svc:?}");

  let svc = svc.transition_at::<Connected, Disconnected, Is1, _, _, _>(|c| {
    let Connected = c;
    Disconnected
  });
  println!("after disconnect: {svc:?}");
  assert!(svc.state.select(Auth).is::<LoggedIn>());

  // recovery
  let svc: Service<States<(LoggedIn, Disconnected)>, _> = svc.trim_unknown_at().unwrap();
  println!("trimmed: {svc:?}");

  let svc2: Service<States<(LoggedIn, Of<Network>)>, _> =
    Service::new(states!(LoggedIn, Connected), caps!()).into_unknown_at(); // or .into_unknown_at::<Connected, _, _>();
  println!("erased component 1: {svc2:?}");
  assert!(svc2.is_at::<Connected, _>()); // is_at is the same as .select(_).is

  let lock: Lock<Of<Door>, _> = Lock::new(Closed, caps!()).into_unknown();
  assert!(lock.is::<Closed>());
  assert!(!lock.is::<Open>());
  assert!(lock.as_some::<Closed>().is_some());

  let concrete = lock.trim_unknown::<Closed>().unwrap(); // same as into_some().unwrap()
  println!("recovered: {concrete:?}");

  let erased: Of<Auth> = Of::new(LoggedIn);
  assert!(erased.is::<LoggedIn>());
  assert!(erased.as_some::<LoggedIn>().is_some());
}
