use ductor::*;

#[cap(derive(Debug, Clone, Copy))]
pub struct IgnitionKey;

#[cap(derive(Debug, Clone, Copy))]
pub struct Fuel;

#[typestate(derive(Debug, Clone, Copy))]
pub enum EngineState {
  // needs both key and fuel to start
  #[transition(Starting, requires = (IgnitionKey, Fuel))]
  Stopped,

  #[transition(Running)]
  Starting,

  #[transition(Stopped)]
  Running,
}

#[unit(derive(Debug, Clone), states = EngineState)]
pub struct Engine<S, C> {
  pub state: S,
  pub caps: C,
}

#[spec(for = Stopped, with = (IgnitionKey, Fuel))]
impl Engine {
  #[transit(to = Starting)]
  pub fn start(self) {
    self.transition(|Stopped| Starting)
  }
}

#[spec(for = Starting)]
impl Engine {
  #[transit(to = Running)]
  pub fn rev(self) {
    self.transition(|Starting| Running)
  }
}

#[typestate(derive(Debug, Clone, Copy))]
pub enum TransmissionState {
  #[transition(FirstGear)]
  Neutral,

  #[transition(SecondGear)]
  FirstGear,

  #[transition(ThirdGear)]
  SecondGear,

  #[transition(SecondGear)]
  ThirdGear,
}

#[unit(derive(Debug, Clone), states = TransmissionState)]
pub struct Transmission<S, C> {
  pub state: S,
  pub caps: C,
}

#[spec(for = Neutral)]
impl Transmission {
  #[transit(to = FirstGear)]
  pub fn shift_up(self) {
    self.transition(|Neutral| FirstGear)
  }
}

// - vehicle: combines engine + transmission

#[unit(derive(Debug, Clone), states = (EngineState, TransmissionState))]
pub struct Vehicle<S, C> {
  pub state: S,
  pub caps: C,
}

#[spec(for = (Stopped, Neutral), with = (IgnitionKey, Fuel))]
impl Vehicle {
  #[transit(to = (Starting, ()))]
  pub fn start_engine(self) {
    self.transition_at(|Stopped| Starting)
  }
}

#[spec(for = (Starting, ()))]
impl Vehicle {
  #[transit(to = (Running, ()))]
  pub fn rev_engine(self) {
    self.transition_at(|Starting| Running)
  }
}

#[spec(for = (Running, Neutral))]
impl Vehicle {
  #[transit(to = ((), FirstGear))]
  pub fn shift_to_first(self) {
    self.transition_at(|Neutral| FirstGear)
  }
}

fn main() {
  let caps = caps!(IgnitionKey, Fuel);

  let vehicle = Vehicle::new(states!(Stopped, Neutral), caps)
    .start_engine()
    .rev_engine()
    .shift_to_first();

  println!("Vehicle state: {vehicle:?}");
}
