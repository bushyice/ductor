#![no_std]

extern crate alloc;

use alloc::boxed::Box;

/// TODO: Extend limits beyond 7

/// marks something as a capability type.
///
/// you can use it through `#[cap]` or just implement it manually on a struct.
/// capabilities get attached to a `Unit` and the compiler checks them whenever
/// a transition or spec block requires them.
pub trait Capability {}

/// marker for things that can show up as requirements on a transition.
///
/// anything that's a `Capability` also counts as a `Requirement`. tuples of
/// requirements (up to 7) work too, and so does `NoRequirement`.
pub trait Requirement {}

impl<C: Capability> Requirement for C {}
impl<C: Capability> Satisfies<C, IsSelf> for C {}

/// checks whether a caps container satisfies a given requirement.
///
/// the `M` type param disambiguates which capability in a tuple we're looking
/// at. you don't really need to implement this yourself since the macros and
/// blanket impls handle it all.
#[diagnostic::on_unimplemented(
  label = "requirement unsatisfied",
  message = "`{Self}` does not satisfy requirement or capability of `{R}`"
)]
pub trait Satisfies<R: Requirement, M = ()> {}

/// extension trait that helps disambiguate `Satisfies` resolution.
pub trait SatisfiesExt<R: Requirement, M> {}
impl<C, R, M> SatisfiesExt<R, M> for C
where
  R: Requirement,
  C: Satisfies<R, M>,
{
}

/// marker type for when a capability satisfies itself directly.
pub struct IsSelf;
/// marker type for when no capability is needed (`NoRequirement`).
pub struct IsNone;
/// wraps a tuple of markers for multi-capability requirements.
pub struct IsTuple<M>(pub M);
/// marker for proxy capability delegation (`#[cap(as = ...)]`).
pub struct IsAs;
/// per-position proxy markers so impls don't overlap.
pub struct IsAs0;
pub struct IsAs1;
pub struct IsAs2;
pub struct IsAs3;
pub struct IsAs4;
pub struct IsAs5;
pub struct IsAs6;

/// when there's no requirement at all.
///
/// transitions without a `requires` clause get this as their
/// `Transition::Requirements` type. everything satisfies it.
pub struct NoRequirement;
impl Requirement for NoRequirement {}
impl<C> Satisfies<NoRequirement, IsNone> for C {}

macro_rules! impl_requirement_tuple {
  ($($T:ident),+) => {
    impl<$($T: Requirement),+> Requirement for ($($T,)+) {}
  };
}

impl Requirement for () {}

macro_rules! impl_tuple_satisfies {
  (
    [$($Req:ident),+];
    [$($Marker:ident),+]
  ) => {
    impl<Caps, $($Req,)+ $($Marker,)+>
      Satisfies<
        ($($Req,)+),
        IsTuple<($($Marker,)+)>
      >
    for Caps
    where
      $(
        $Req: Requirement,
        Caps: Satisfies<$Req, $Marker>,
      )+
    {}
  };
}

impl<C> Satisfies<(), IsTuple<()>> for C {}

/// a newtype wrapper around a tuple of capabilities.
///
/// you usually make these with the `caps!()` macro. the type param `C` is
/// the tuple type, like `(NetCap, AuthCap)`. access it through a `Unit`
/// via `.caps()`.
#[derive(Debug, Clone)]
pub struct Caps<C>(pub C);

impl<CapsTuple, R, Marker> Satisfies<R, Marker> for Caps<CapsTuple>
where
  Caps<CapsTuple>: HasCap<R, Marker>,
  R: Requirement,
{
}

/// pulls out a specific capability by type and marker index.
pub trait HasCap<Target, Marker> {
  fn get_cap(&self) -> &Target;
}

macro_rules! impl_has_cap {
  ($($T:ident),+) => {
    impl_has_cap!(
      @unwrap
      Caps<($($T,)+)>;
      ($($T),+);
      [$($T),+];
      [0 1 2 3 4 5 6];
      [Is0 Is1 Is2 Is3 Is4 Is5 Is6]
    );
  };

  (
    @unwrap
    $Container:ty;
    ($($All:ident),+);
    [$Head:ident $(, $Tail:ident)*];
    [$IdxHead:tt $($IdxTail:tt)*];
    [$MarkerHead:ident $($MarkerTail:ident)*]
  ) => {
    impl<$($All),+>
        HasCap<$Head, $MarkerHead>
    for $Container
    {
      #[inline(always)]
      fn get_cap(&self) -> &$Head {
        &self.0.$IdxHead
      }
    }

    impl_has_cap!(
      @unwrap
      $Container;
      ($($All),+);
      [$($Tail),*];
      [$($IdxTail)*];
      [$($MarkerTail)*]
    );
  };

  (
    @unwrap
    $Container:ty;
    ($($All:ident),+);
    [];
    [$($IdxTail:tt)*];
    [$($MarkerTail:ident)*]
  ) => {};
}

/// wraps up one or more capability values into a `Caps` tuple.
///
/// # example
/// ```
/// use ductor_core::*;
/// struct MyCap;
/// impl Capability for MyCap {}
/// let c = caps!(MyCap);
/// ```
#[macro_export]
macro_rules! caps {
  ($($s:expr),* $(,)?) => {
    $crate::Caps(($($s,)*))
  };
}

/// wraps up one or more state values into a `States` tuple.
///
/// handy when you're creating a unit with multiple state families.
///
/// # example
/// ```
/// use ductor_core::*;
/// struct MyFamily;
/// impl StateFamily for MyFamily {}
/// #[derive(Debug, Clone)]
/// struct MyState;
/// impl IsState for MyState { type Family = MyFamily; }
/// let s = states!(MyState);
/// ```
#[macro_export]
macro_rules! states {
  ($($s:expr),* $(,)?) => {
    $crate::States(($($s,)*))
  };
}

/// constraint that a `States` tuple follows a given family set.
///
/// makes sure a unit's state tuple has states from the right families in
/// the right order. generated automatically by `#[unit(states = ...)]`.
#[diagnostic::on_unimplemented(
  label = "state / family mismatch",
  message = "`{Self}` does not place its states in the families expected by `{F}` \
             check your `#[unit(states = ...)]` annotation"
)]
pub trait Follows<F> {}

macro_rules! impl_follows {
  (
    [$($S:ident),+];
    [$($F:ident),+]
  ) => {
    impl<$($S,)+ $($F,)+> Follows<($($F,)+)> for States<($($S,)+)>
    where
      $(
        $S: IsState<Family = $F>,
      )+
    {}
  };
}

/// marker trait for a family of related states.
///
/// each family is one state machine. generated by `#[typestate]` (the enum
/// itself becomes the family struct) and by `#[unit]` (creates a private
/// `{UnitName}Family` struct).
pub trait StateFamily {}

/// links a state struct back to its owning `StateFamily`.
///
/// generated by `#[typestate]` for each variant of the enum.
#[diagnostic::on_unimplemented(
  label = "not a recognised state type",
  message = "`{Self}` is not registered as a state, did you forget to derive \
             `#[typestate]` on the enum?"
)]
pub trait IsState {
  /// the family this state belongs to.
  type Family: StateFamily;
}

/// declares that a state machine family supports going from `From` to `To`.
///
/// the `Requirements` associated type says which caps need to be present.
/// generated by `#[transition(Target, requires = ...)]`.
#[diagnostic::on_unimplemented(
  label = "unknown transition",
  message = "`{Self}` does not transition from `{From}` to `{To}`"
)]
pub trait Transition<From, To> {
  /// the capability requirements that must be satisfied.
  type Requirements: Requirement;
}

/// marker trait for state tuples that represent a set of states.
pub trait StateSet {}

/// a newtype wrapper around a tuple of state values.
///
/// each element is a concrete state from a specific `StateFamily`. you
/// usually make these with `states!()`. indexed access comes from
/// `StateAccess`.
#[derive(Debug, Clone)]
pub struct States<S>(pub S);

/// trait for pulling out a state by type from a tuple-like container.
pub trait GetState<Target> {
  fn get(&self) -> &Target;
}

/// indexed state extraction from a state tuple.
///
/// `IndexMarker` tells us which position the target state lives at.
#[diagnostic::on_unimplemented(
  label = "state not found at this position",
  message = "`{Self}` does not contain `{Target}` at position `{IndexMarker}` \
             the type or index is wrong"
)]
pub trait HasState<Target, IndexMarker> {
  /// destructure the tuple to own the state.
  fn get_state(self) -> Target;
  /// borrow the state from the tuple.
  fn get_state_ref(&self) -> &Target;
}

/// family-based state access on a state tuple.
///
/// lets you grab the state belonging to a specific family, no matter where
/// it is in the tuple.
#[diagnostic::on_unimplemented(
  label = "family not found in tuple",
  message = "`{Self}` does not contain a state from family `{Family}` \
             the family or tuple configuration is wrong"
)]
pub trait HasFamily<Family, Marker> {
  type State: IsState<Family = Family>;

  fn get_family_state(&self) -> &Self::State;
}

/// convenience trait for selecting a state by family value.
#[diagnostic::on_unimplemented(
  label = "family not present",
  message = "`{Self}` does not contain a state from family `{Family}`"
)]
pub trait SelectFamily<Family> {
  type State: IsState<Family = Family>;

  fn select(&self, _: Family) -> &Self::State;
}

/// destructure-and-replace a state value (you probably want `ReplaceStateAt`).
pub trait ReplaceState<From, To> {
  type Output;
  fn replace(self, new: To) -> Self::Output;
}

/// marker trait for index-level position types (`Is0`, `Is1`, ...).
pub trait IndexMarker {}

macro_rules! make_idx_marker {
  ($name:ident) => {
    /// index marker for tuple position.
    pub struct $name;
    impl IndexMarker for $name {}
  };
}

/// destructure a state tuple, swap one element by index, and rebuild it.
///
/// used internally by `transition_at()` to type-safely update a single
/// state in a multi-state unit.
#[diagnostic::on_unimplemented(
  label = "cannot replace state at this position",
  message = "`{Self}` cannot replace `{From}` with `{To}` at `{IndexMarker}` \
             the index or state types are wrong"
)]
pub trait ReplaceStateAt<IndexMarker, From, To, Out> {
  fn replace_with<F>(self, f: F) -> Out
  where
    F: FnOnce(From) -> To;
}

impl<From, To> ReplaceStateAt<IsSelf, From, To, To> for From
where
  From: IsState,
  To: IsState,
{
  fn replace_with<F>(self, f: F) -> To
  where
    F: FnOnce(From) -> To,
  {
    f(self)
  }
}

/// convenience trait with `.take()`, `.get()`, and `.select()` on state
/// tuples and single states.
///
/// - `take::<T, M>()` -- destructure to own a specific state type.
/// - `get::<T, M>()` -- borrow a specific state type.
/// - `select::<F, M>(family)` -- borrow a state by its family.
///
/// blanket-implemented for all `IsState` and `States<(...)>`.
pub trait StateAccess: Sized {
  #[inline(always)]
  fn take<Target, IndexMarker>(self) -> Target
  where
    Self: HasState<Target, IndexMarker>,
  {
    HasState::<Target, IndexMarker>::get_state(self)
  }

  #[inline(always)]
  fn get<Target, IndexMarker>(&self) -> &Target
  where
    Self: HasState<Target, IndexMarker>,
  {
    HasState::<Target, IndexMarker>::get_state_ref(self)
  }

  #[inline(always)]
  fn select<Family, Marker>(&self, _: Family) -> &<Self as HasFamily<Family, Marker>>::State
  where
    Self: HasFamily<Family, Marker>,
  {
    <Self as HasFamily<Family, Marker>>::get_family_state(self)
  }
}

impl<S: IsState> StateAccess for S {}

impl<S: IsState> HasState<S, IsSelf> for S {
  fn get_state(self) -> S {
    self
  }
  fn get_state_ref(&self) -> &S {
    self
  }
}

impl<S: IsState> HasFamily<<S as IsState>::Family, IsSelf> for S {
  type State = S;
  fn get_family_state(&self) -> &S {
    self
  }
}

macro_rules! impl_is_state {
  ($($T:ident),+) => {
    impl<$($T),+> StateSet for States<($($T,)+)> {}

    impl<$($T),+> StateAccess for States<($($T,)+)> {}

    impl_is_state!(
      @unwrap
      States<($($T,)+)>;
      ($($T),+);
      [$($T),+];
      [0 1 2 3 4 5 6];
      [Is0 Is1 Is2 Is3 Is4 Is5 Is6]
    );

    impl_is_state!(
      @has_family
      States<($($T,)+)>;
      ($($T),+);
      [$($T),+];
      [0 1 2 3 4 5 6 7];
      [Is0 Is1 Is2 Is3 Is4 Is5 Is6 Is7]
    );
  };

  (
    @unwrap
    $TargetContainer:ty;
    ($($AllTypes:ident),+);
    [$Head:ident $(, $Tail:ident)*];
    [$IdxHead:tt $($IdxTail:tt)*];
    [$MarkerHead:ident $($MarkerTail:ident)*]
  ) => {
    impl<$($AllTypes),+> HasState<$Head, $MarkerHead> for $TargetContainer {
      #[inline(always)]
      fn get_state(self) -> $Head {
        self.0.$IdxHead
      }
      #[inline(always)]
      fn get_state_ref(&self) -> &$Head {
        &self.0.$IdxHead
      }
    }

    impl_is_state!(
      @unwrap
      $TargetContainer;
      ($($AllTypes),+);
      [$($Tail),*];
      [$($IdxTail)*];
      [$($MarkerTail)*]
    );
  };

  (@unwrap $TargetContainer:ty; ($($AllTypes:ident),+); []; [$($IdxTail:tt)*]; [$($MarkerTail:ident)*]) => {};

  (
    @has_family
    $Container:ty;
    ($($All:ident),+);
    [$Head:ident $(, $Tail:ident)*];
    [$IdxHead:tt $($IdxTail:tt)*];
    [$MarkerHead:ident $($MarkerTail:ident)*]
  ) => {
    impl<$($All),+> HasFamily<<$Head as IsState>::Family,$MarkerHead> for $Container
    where
      $Head: IsState,
    {
      type State = $Head;

      #[inline(always)]
      fn get_family_state(&self,) -> &Self::State {
        &self.0.$IdxHead
      }
    }


    impl_is_state!(
      @has_family
      $Container;
      ($($All),+);
      [$($Tail),*];
      [$($IdxTail)*];
      [$($MarkerTail)*]
    );
  };

  (
    @has_family
    $Container:ty;
    ($($All:ident),+);
    [];
    [$($IdxTail:tt)*];
    [$($MarkerTail:ident)*]
  ) => {};
}

/// impl_replace_state!([Is0, Is1, Is2] => [A0, A1, A2]);
macro_rules! impl_replace_state {
  ([$($Idx:ty),*] => [$($A:ident),*]) => {
    impl_replace_state!(@internal [$($Idx),*] [] [$($A)*] []);
  };

  (@internal [$($Idx:ty),*] [$($Before:ident)*] [] [$($ProcessedIdx:ty)*]) => {};

  (
    @internal
    [$CurrentIdx:ty $(, $NextIdx:ty)*]
    [$($Before:ident)*]
    [$Head:ident $($Tail:ident)*]
    [$($ProcessedIdx:ty)*]
  ) => {
    impl<$($Before,)* $Head, $($Tail,)* To>
      ReplaceStateAt<
        $CurrentIdx,
        $Head,
        To,
        States<($($Before,)* To, $($Tail,)*)>
      >
      for States<($($Before,)* $Head, $($Tail,)*)>
    where
      $($Before: IsState,)*
      $Head: IsState,
      $($Tail: IsState,)*
      To: IsState,
    {
      fn replace_with<FN>(self, f: FN) -> States<($($Before,)* To, $($Tail,)*)>
      where
        FN: FnOnce($Head) -> To,
      {
        #[allow(non_snake_case)]
        let ($($Before,)* target, $($Tail,)*) = self.0;
        States(($($Before,)* f(target), $($Tail,)*))
      }
    }

    impl_replace_state!(
      @internal
      [$($NextIdx),*]
      [$($Before)* $Head]
      [$($Tail)*]
      [$($ProcessedIdx)* $CurrentIdx]
    );
  };
}

// Behold! Mountains!!

make_idx_marker!(Is0);
make_idx_marker!(Is1);
make_idx_marker!(Is2);
make_idx_marker!(Is3);
make_idx_marker!(Is4);
make_idx_marker!(Is5);
make_idx_marker!(Is6);

impl<S0, F0> Follows<F0> for S0 where S0: IsState<Family = F0> {}

impl_follows!(
  [S0];
  [F0]
);

impl_follows!(
  [S0, S1];
  [F0, F1]
);

impl_follows!(
  [S0, S1, S2];
  [F0, F1, F2]
);

impl_follows!(
  [S0, S1, S2, S3];
  [F0, F1, F2, F3]
);

impl_follows!(
  [S0, S1, S2, S3, S4];
  [F0, F1, F2, F3, F4]
);

impl_follows!(
  [S0, S1, S2, S3, S4, S5];
  [F0, F1, F2, F3, F4, F5]
);

impl_follows!(
  [S0, S1, S2, S3, S4, S5, S6];
  [F0, F1, F2, F3, F4, F5, F6]
);

impl_has_cap!(A);
impl_has_cap!(A, B);
impl_has_cap!(A, B, C);
impl_has_cap!(A, B, C, D);
impl_has_cap!(A, B, C, D, E);
impl_has_cap!(A, B, C, D, E, F);
impl_has_cap!(A, B, C, D, E, F, G);

impl_requirement_tuple!(A);
impl_requirement_tuple!(A, B);
impl_requirement_tuple!(A, B, C);
impl_requirement_tuple!(A, B, C, D);
impl_requirement_tuple!(A, B, C, D, E);
impl_requirement_tuple!(A, B, C, D, E, F);
impl_requirement_tuple!(A, B, C, D, E, F, G);

impl_tuple_satisfies!([A]; [MA]);
impl_tuple_satisfies!([A, B]; [MA, MB]);
impl_tuple_satisfies!([A, B, C]; [MA, MB, MC]);
impl_tuple_satisfies!([A, B, C, D]; [MA, MB, MC, MD]);
impl_tuple_satisfies!([A, B, C, D, E]; [MA, MB, MC, MD, ME]);
impl_tuple_satisfies!([A, B, C, D, E, F]; [MA, MB, MC, MD, ME, MF]);
impl_tuple_satisfies!([A, B, C, D, E, F, G]; [MA, MB, MC, MD, ME, MF, MG]);

impl_is_state!(A, B);
impl_is_state!(A, B, C);
impl_is_state!(A, B, C, D);
impl_is_state!(A, B, C, D, E);
impl_is_state!(A, B, C, D, E, F);
impl_is_state!(A, B, C, D, E, F, G);

impl_replace_state!([Is0, Is1] => [A0, A1]);
impl_replace_state!([Is0, Is1, Is2] => [A0, A1, A2]);
impl_replace_state!([Is0, Is1, Is2, Is3] => [A0, A1, A2, A3]);
impl_replace_state!([Is0, Is1, Is2, Is3, Is4] => [A0, A1, A2, A3, A4]);
impl_replace_state!([Is0, Is1, Is2, Is3, Is4, Is5] => [A0, A1, A2, A3, A4, A5]);
impl_replace_state!([Is0, Is1, Is2, Is3, Is4, Is5, Is6] => [A0, A1, A2, A3, A4, A5, A6]);

/// a typed unit with a state and capabilities.
///
/// this is the main component in ductor. a `Unit` wraps a state (single or
/// tuple) and a set of caps, and makes sure at compile time that
/// transitions and method calls are valid for the current state/cap combo.
///
/// generated by the `#[unit]` attribute.
pub trait Unit {
  /// the state type (single state or `States<(...)>` tuple).
  type State;
  /// the capability container type (a `Caps<(...)>` tuple).
  type Caps;
  /// the family for this unit (used in unit-level `#[spec]` blocks).
  type Family: StateFamily;

  /// borrow the current state.
  fn state(&self) -> &Self::State;
  /// borrow the current capabilities.
  fn caps(&self) -> &Self::Caps;
}

/// trait for single-state transitions.
///
/// the `#[unit]` macro implements this for every state that has a valid
/// `Transition` to `Target`. the `transition()` method only compiles when
/// this trait is satisfied.
#[diagnostic::on_unimplemented(
  label = "cannot transition to this state",
  message = "`{Self}` cannot transition to `{Target}`. either the transition \
             is not declared in the state machine or the required capabilities \
             are missing"
)]
pub trait CanTransitionTo<Target, C, F, M> {
  /// the output unit type after the transition.
  type Out;
  /// do the transition.
  fn perform_transition(self, f: F) -> Self::Out;
}

/// trait for multi-state (indexed) transitions.
///
/// works through the `transition_at()` method on units with tuple states.
/// lets you transition one state machine inside a unit that manages several.
#[diagnostic::on_unimplemented(
  label = "cannot transition to this state",
  message = "`{Self}` cannot transition from `{From}` to `{To}`. either the transition \
             is not declared in the state machine or the required capabilities \
             are missing"
)]
pub trait CanTransitionAt<From, To, Caps, F> {
  /// the output unit type after the transition.
  type Out;

  /// do the indexed transition.
  fn perform_transition(self, f: F) -> Self::Out;
}

/// a type-erased state that remembers its family.
///
/// `Of<F>` represents "some state in family F" without revealing
/// the exact variant. you erase the concrete type with `into_unknown()`
/// on any unit, and recover it with `as_some::<S>()` / `into_some::<S>()`.
pub struct Of<F: StateFamily> {
  inner: Box<dyn core::any::Any>,
  _family: core::marker::PhantomData<F>,
}

impl<F: StateFamily> Of<F> {
  /// wrap a concrete state into the erased wrapper.
  ///
  /// checked at compile time: `S` must be a state belonging to family `F`.
  pub fn new<S: IsState<Family = F> + 'static>(state: S) -> Self {
    Of {
      inner: Box::new(state),
      _family: core::marker::PhantomData,
    }
  }

  /// wrap a value without checking the family.
  ///
  /// used internally by `into_unknown()` where the family is known to be
  /// correct by construction (it uses the unit's own family marker).
  pub fn new_unchecked<S: 'static>(state: S) -> Self {
    Of {
      inner: Box::new(state),
      _family: core::marker::PhantomData,
    }
  }

  /// borrow the inner value as a specific state.
  ///
  /// returns `None` if the concrete type doesn't match.
  pub fn as_some<S: IsState<Family = F> + 'static>(&self) -> Option<&S> {
    let any: &dyn core::any::Any = &*self.inner;
    any.downcast_ref::<S>()
  }

  /// unwrap the inner value as a specific state.
  ///
  /// returns `None` if the concrete type doesn't match
  /// (the inner value is consumed either way).
  pub fn into_some<S: IsState<Family = F> + 'static>(self) -> Option<S> {
    let any = self.inner;
    any.downcast::<S>().ok().map(|b| *b)
  }

  /// check whether the inner value is a specific state.
  pub fn is<S: IsState<Family = F> + 'static>(&self) -> bool {
    let any: &dyn core::any::Any = &*self.inner;
    any.downcast_ref::<S>().is_some()
  }
}

impl<F: StateFamily> IsState for Of<F> {
  type Family = F;
}

impl<F: StateFamily> core::fmt::Debug for Of<F> {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    write!(f, "Of<{}>", core::any::type_name::<F>())
  }
}
