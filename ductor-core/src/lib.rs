pub trait Capability {}
pub trait Requirement {}

impl<C: Capability> Requirement for C {}
impl<C: Capability> Satisfies<C, IsSelf> for C {}

pub trait Satisfies<R: Requirement, M = ()> {}

pub trait SatisfiesExt<R: Requirement, M> {}
impl<C, R, M> SatisfiesExt<R, M> for C
where
  R: Requirement,
  C: Satisfies<R, M>,
{
}

pub struct IsSelf;
pub struct IsNone;
pub struct IsTuple<M>(pub M);

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

pub struct Caps<C>(pub C);

impl<CapsTuple, R, Marker> Satisfies<R, Marker> for Caps<CapsTuple>
where
  Caps<CapsTuple>: HasCap<R, Marker>,
  R: Requirement,
{
}

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

#[macro_export]
macro_rules! caps {
  ($($s:expr),* $(,)?) => {
    $crate::Caps(($($s,)*))
  };
}

#[macro_export]
macro_rules! states {
  ($($s:expr),* $(,)?) => {
    $crate::States(($($s,)*))
  };
}

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

pub trait Provides<T> {}
pub trait Conflicts<T> {}

pub trait StateFamily {}

pub trait IsState {
  type Family: StateFamily;
}

pub trait Transition<From, To> {
  type Requirements: Requirement;
}

pub trait StateSet {}

#[derive(Debug, Clone)]
pub struct States<S>(pub S);

pub trait GetState<Target> {
  fn get(&self) -> &Target;
}

pub trait HasState<Target, IndexMarker> {
  fn get_state(self) -> Target;
  fn get_state_ref(&self) -> &Target;
}

pub trait HasFamily<Family, Marker> {
  type State: IsState<Family = Family>;

  fn get_family_state(&self) -> &Self::State;
}

pub trait SelectFamily<Family> {
  type State: IsState<Family = Family>;

  fn select(&self, _: Family) -> &Self::State;
}

pub trait ReplaceState<From, To> {
  type Output;
  fn replace(self, new: To) -> Self::Output;
}

pub trait IndexMarker {}

macro_rules! make_idx_marker {
  ($name:ident) => {
    pub struct $name;
    impl IndexMarker for $name {}
  };
}

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

pub trait Unit {
  type State;
  type Caps;
  type Family: StateFamily;

  fn state(&self) -> &Self::State;
  fn caps(&self) -> &Self::Caps;
}

pub trait CanTransitionTo<Target, C, F, M> {
  type Out;
  fn perform_transition(self, f: F) -> Self::Out;
}

pub trait CanTransitionAt<From, To, Caps, F> {
  type Out;

  fn perform_transition(self, f: F) -> Self::Out;
}
