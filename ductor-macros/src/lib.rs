use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
  Fields, GenericParam, Ident, ItemEnum, ItemStruct, Token, Type,
  ext::IdentExt,
  parenthesized,
  parse::{Parse, ParseStream},
  parse_macro_input,
  punctuated::Punctuated,
};

struct AttrArgs {
  derives: Vec<syn::Path>,
  states: Option<SpecValue>,
  as_caps: Option<SpecValue>,
  prefixed: bool,
  wrapped: bool,
  family_name: Option<String>,
}

impl Parse for AttrArgs {
  fn parse(input: ParseStream) -> syn::Result<Self> {
    let mut derives = Vec::new();
    let mut states = None;
    let mut as_caps = None;
    let mut prefixed = false;
    let mut wrapped = false;
    let mut family_name = None;
    while !input.is_empty() {
      if input.peek(Ident)
        || input.peek(Token![type])
        || input.peek(Token![const])
        || input.peek(Token![as])
      {
        let id = Ident::parse_any(input)?;
        if id == "derive" {
          let content;
          parenthesized!(content in input);
          let paths: Punctuated<syn::Path, Token![,]> =
            content.parse_terminated(syn::Path::parse, Token![,])?;
          derives.extend(paths);
        } else if id == "states" {
          input.parse::<Token![=]>()?;
          let val = parse_spec_value(input)?;
          states = Some(val);
        } else if id == "as" {
          input.parse::<Token![=]>()?;
          let val = parse_spec_value(input)?;
          as_caps = Some(val);
        } else if id == "prefixed" {
          prefixed = true;
        } else if id == "wrapped" {
          wrapped = true;
        } else if id == "family_name" {
          input.parse::<Token![=]>()?;
          let lit: syn::LitStr = input.parse()?;
          family_name = Some(lit.value());
        }
      }
      if input.peek(Token![,]) {
        input.parse::<Token![,]>()?;
      } else {
        break;
      }
    }
    Ok(AttrArgs {
      derives,
      states,
      as_caps,
      prefixed,
      wrapped,
      family_name,
    })
  }
}

fn parse_spec_value(input: ParseStream) -> syn::Result<SpecValue> {
  if input.peek(syn::token::Paren) {
    let content;
    parenthesized!(content in input);
    let tys: Punctuated<syn::Type, Token![,]> =
      content.parse_terminated(syn::Type::parse, Token![,])?;
    Ok(SpecValue::Tuple(tys.into_iter().collect()))
  } else {
    Ok(SpecValue::Single(input.parse()?))
  }
}

struct TransitionAttr {
  target: syn::Ident,
  requires: Option<Type>,
  derives: Vec<syn::Path>,
}

impl Parse for TransitionAttr {
  fn parse(input: ParseStream) -> syn::Result<Self> {
    let target: syn::Ident = input.parse()?;
    let mut requires = None;
    let mut derives = Vec::new();

    if input.peek(Token![,]) {
      input.parse::<Token![,]>()?;
      let lookahead = input.lookahead1();
      if lookahead.peek(syn::Ident) {
        let key: syn::Ident = input.parse()?;
        if key == "derive" {
          let content;
          parenthesized!(content in input);
          let paths: Punctuated<syn::Path, Token![,]> =
            content.parse_terminated(syn::Path::parse, Token![,])?;
          derives.extend(paths);
        } else if key == "requires" {
          input.parse::<Token![=]>()?;
          requires = Some(input.parse()?);
        }
      }
    }

    Ok(TransitionAttr {
      target,
      requires,
      derives,
    })
  }
}

/// turns an enum into a state machine.
///
/// each variant becomes its own struct. the enum name becomes a family struct
/// implementing `StateFamily`. every variant gets an `IsState` impl.
///
/// if you slap `#[transition(Target, requires = Type)]` on a variant, it
/// generates a `Transition` impl with optional capability requirements. :D
///
/// # attr arguments
/// - `derive(Trait, ...)`: passes derives through to every generated struct.
/// - `prefixed`: names state structs as `{Enum}{Variant}` (as `DoorOpen`)
///   instead of just `{Variant}`.
/// - `wrapped`: places all generated items inside `mod {Enum} { ... }`,
///   so you access states as `{Enum}::{Variant}` and the family
///   struct as `{Enum}::State`.
/// - `family_name = "Name"`: to set the family name (defaults to `State`
///   with `wrapped`).
///
/// # variant attrs
/// - `#[transition(Target)]`: marks a valid transition from this state to `Target`.
/// - `#[transition(Target, requires = Type)]`: same but needs a capability.
///
/// # example
/// ```ignore
/// #[typestate(derive(Debug))]
/// pub enum Door {
///   #[transition(Closed)]
///   Open,
///   #[transition(Open)]
///   Closed,
/// }
/// ```
#[proc_macro_attribute]
pub fn typestate(attr: TokenStream, item: TokenStream) -> TokenStream {
  let args = parse_macro_input!(attr as AttrArgs);
  let input = parse_macro_input!(item as ItemEnum);

  let derives = if args.derives.is_empty() {
    quote! {}
  } else {
    let paths = &args.derives;
    quote! { #[derive(#( #paths ),*)] }
  };

  let enum_name = &input.ident;
  let visibility = &input.vis;

  let family_ident = if let Some(ref name) = args.family_name {
    format_ident!("{}", name)
  } else if args.wrapped {
    format_ident!("State")
  } else {
    enum_name.clone()
  };

  let inner_visibility = if args.wrapped {
    quote! { pub }
  } else {
    quote! { #visibility }
  };

  let mut generated_structs = Vec::new();
  let mut transition_impls = Vec::new();
  let mut is_state_impls = Vec::new();

  for variant in &input.variants {
    let variant_name = &variant.ident;
    let fields = &variant.fields;
    let mut derives = derives.clone();

    for attr in &variant.attrs {
      if attr.path().is_ident("transition") {
        let transition_data: TransitionAttr = attr
          .parse_args()
          .expect("Failed to parse transition attribute");

        if !transition_data.derives.is_empty() {
          let paths = &transition_data.derives;
          derives = quote! { #[derive(#( #paths ),*)] };
        }

        let target = &transition_data.target;
        let req = transition_data
          .requires
          .map(|r| quote! { #r })
          .unwrap_or_else(|| quote! { ductor::NoRequirement });

        if args.prefixed {
          let from_type = format_ident!("{}{}", enum_name, variant_name);
          let to_type = format_ident!("{}{}", enum_name, target);
          transition_impls.push(quote! {
            impl ductor::Transition<#from_type, #to_type> for #family_ident {
              type Requirements = #req;
            }
          });
        } else {
          transition_impls.push(quote! {
            impl ductor::Transition<#variant_name, #target> for #family_ident {
              type Requirements = #req;
            }
          });
        }
      }
    }

    let state_type = if args.prefixed {
      format_ident!("{}{}", enum_name, variant_name)
    } else {
      variant_name.clone()
    };
    let struct_gen = match fields {
      Fields::Named(f) => quote! {
        #derives
        #inner_visibility struct #state_type #f
      },
      Fields::Unnamed(f) => quote! {
        #derives
        #inner_visibility struct #state_type #f;
      },
      Fields::Unit => quote! {
        #derives
        #inner_visibility struct #state_type;
      },
    };
    generated_structs.push(struct_gen);

    is_state_impls.push(quote! {
      impl ductor::IsState for #state_type {
        type Family = #family_ident;
      }
    });
  }

  let family_struct = quote! {
    #derives
    #inner_visibility struct #family_ident;
    impl ductor::StateFamily for #family_ident {}
  };

  let expanded = if args.wrapped {
    quote! {
      #[allow(non_snake_case)]
      #visibility mod #enum_name {
        use super::*;

        #( #generated_structs )*

        #family_struct

        #( #is_state_impls )*
        #( #transition_impls )*
      }
    }
  } else {
    quote! {
      #( #generated_structs )*

      #family_struct

      #( #is_state_impls )*
      #( #transition_impls )*
    }
  };

  TokenStream::from(expanded)
}

#[derive(Clone)]
enum FieldInit {
  Default,
  Take,
  Construct(syn::Expr),
}

struct UnitFieldAttr {
  kind: FieldInit,
}

impl syn::parse::Parse for UnitFieldAttr {
  fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
    let id = syn::Ident::parse_any(input)?;
    match id.to_string().as_str() {
      "default" => Ok(UnitFieldAttr {
        kind: FieldInit::Default,
      }),
      "take" => Ok(UnitFieldAttr {
        kind: FieldInit::Take,
      }),
      "construct" => {
        input.parse::<Token![=]>()?;
        let expr: syn::Expr = input.parse()?;
        Ok(UnitFieldAttr {
          kind: FieldInit::Construct(expr),
        })
      }
      other => Err(syn::Error::new(
        id.span(),
        format!("expected `default`, `take`, or `construct`, got `{other}`"),
      )),
    }
  }
}

fn get_field_init(field: &syn::Field) -> FieldInit {
  for attr in &field.attrs {
    if attr.path().is_ident("unit") {
      if let Ok(parsed) = attr.parse_args::<UnitFieldAttr>() {
        return parsed.kind;
      }
    }
  }
  FieldInit::Default
}

/// generates a typed container (unit) for states and capabilities.
///
/// just write your struct -- no need for `state: State` and `caps: Caps`
/// fields or generics, the macro adds them automatically :D
///
/// it generates:
/// - a `Unit` impl with associated `State`, `Caps`, and `Family` types.
/// - a `new(state, caps)` constructor.
/// - a `transition()` method for single-state units.
/// - a `transition_at()` method for multi-state units.
/// - `CanTransitionTo` impls for every allowed transition.
///
/// if your struct has extra fields, annotate how they're initialized:
///
/// | annotation | behavior |
/// |------------|----------|
/// | `#[unit(default)]` | `Default::default()` (this is the default if omitted) |
/// | `#[unit(take)]` | added as a parameter to `new()` |
/// | `#[unit(construct = expr)]` | computed via the given expression (can reference `state`, `caps`, and earlier fields) |
///
/// # attr arguments (struct-level)
/// - `derive(Trait, ...)`: passed through to the struct.
/// - `states = Type` or `states = (Type, ...)`: constrains the state tuple
///   via `Follows` so positions match up with state families.
///
/// # example
/// ```ignore
/// #[unit(derive(Debug))]
/// pub struct Lock;
///
/// #[unit(derive(Debug))]
/// pub struct Service {
///   #[unit(default)]
///   url: String,
///   #[unit(take)]
///   port: u16,
///   #[unit(construct = format!("{url}:{port}"))]
///   addr: String,
/// }
/// ```
#[proc_macro_attribute]
pub fn unit(attr: TokenStream, item: TokenStream) -> TokenStream {
  let args = parse_macro_input!(attr as AttrArgs);
  let mut input = parse_macro_input!(item as ItemStruct);

  if !args.derives.is_empty() {
    let paths = &args.derives;
    input
      .attrs
      .push(syn::parse_quote! { #[derive(#( #paths ),*)] });
  }

  let name = &input.ident;

  let has_state_generic = input
    .generics
    .params
    .iter()
    .any(|param| matches!(param, GenericParam::Type(t) if t.ident == "State"));
  let has_caps_generic = input
    .generics
    .params
    .iter()
    .any(|param| matches!(param, GenericParam::Type(t) if t.ident == "Caps"));

  if !has_state_generic {
    input.generics.params.push(syn::parse_quote! { State });
  }
  if !has_caps_generic {
    input.generics.params.push(syn::parse_quote! { Caps });
  }

  let mut all_custom: Vec<(syn::Ident, syn::Type, FieldInit)> = Vec::new();

  match &mut input.fields {
    Fields::Unit => {
      let dummy: syn::ItemStruct = syn::parse_quote! {
        struct Dummy {
          pub state: State,
          pub caps: Caps,
        }
      };
      input.fields = dummy.fields;
    }
    Fields::Named(named) => {
      let mut custom_raw: Vec<syn::Field> = named
        .named
        .iter()
        .filter(|f| {
          f.ident
            .as_ref()
            .map_or(true, |id| id != "state" && id != "caps")
        })
        .cloned()
        .collect();

      for f in &custom_raw {
        if let Some(ref ident) = f.ident {
          let init = get_field_init(f);
          all_custom.push((ident.clone(), f.ty.clone(), init));
        }
      }

      for f in &mut custom_raw {
        f.attrs.retain(|a| !a.path().is_ident("unit"));
      }

      named.named.clear();
      named.named.push(syn::parse_quote! { pub state: State });
      named.named.push(syn::parse_quote! { pub caps: Caps });
      named.named.extend(custom_raw);
    }
    Fields::Unnamed(_) => {
      return syn::Error::new(input.ident.span(), "#[unit] does not support tuple structs")
        .to_compile_error()
        .into();
    }
  }

  let (impl_generics, type_generics, where_clause) = input.generics.split_for_impl();
  let mut where_clause = where_clause
    .cloned()
    .unwrap_or_else(|| syn::parse_quote! { where });

  if let Some(states_spec) = args.states {
    let follows_ty: syn::Type = match states_spec {
      SpecValue::Single(ty) => syn::parse_quote! { #ty },
      SpecValue::Tuple(tys) => syn::parse_quote! { (#(#tys,)*) },
    };
    where_clause
      .predicates
      .push(syn::parse_quote! { State: ductor::Follows<#follows_ty> });
  }

  let custom_type_idents: Vec<&Ident> = input
    .generics
    .params
    .iter()
    .filter_map(|param| {
      if let GenericParam::Type(t) = param {
        if t.ident != "State" && t.ident != "Caps" {
          return Some(&t.ident);
        }
      }
      None
    })
    .collect();

  let type_args_with_state_replaced = |replacement: &Ident| -> Vec<proc_macro2::TokenStream> {
    input
      .generics
      .params
      .iter()
      .map(|param| match param {
        GenericParam::Type(t) if t.ident == "State" => quote! { #replacement },
        GenericParam::Type(t) => {
          let id = &t.ident;
          quote! { #id }
        }
        GenericParam::Lifetime(l) => {
          let lt = &l.lifetime;
          quote! { #lt }
        }
        GenericParam::Const(c) => {
          let id = &c.ident;
          quote! { #id }
        }
      })
      .collect()
  };

  let all_type_args: Vec<proc_macro2::TokenStream> =
    type_args_with_state_replaced(&format_ident!("State"));
  let for_type_args: Vec<proc_macro2::TokenStream> =
    type_args_with_state_replaced(&format_ident!("From"));
  let to_type_args: Vec<proc_macro2::TokenStream> =
    type_args_with_state_replaced(&format_ident!("To"));
  let out_type_args: Vec<proc_macro2::TokenStream> =
    type_args_with_state_replaced(&format_ident!("Out"));

  let family_name = format_ident!("{}Family", name);

  let type_args_with_state_ts =
    |replacement: proc_macro2::TokenStream| -> Vec<proc_macro2::TokenStream> {
      input
        .generics
        .params
        .iter()
        .map(|param| match param {
          GenericParam::Type(t) if t.ident == "State" => replacement.clone(),
          GenericParam::Type(t) => {
            let id = &t.ident;
            quote! { #id }
          }
          GenericParam::Lifetime(l) => {
            let lt = &l.lifetime;
            quote! { #lt }
          }
          GenericParam::Const(c) => {
            let id = &c.ident;
            quote! { #id }
          }
        })
        .collect()
    };

  let s_args = type_args_with_state_ts(quote! { S });
  let of_args = type_args_with_state_ts(quote! { ductor::Of<<S as ductor::IsState>::Family> });
  let of_family_args = type_args_with_state_ts(quote! { ductor::Of<#family_name> });
  let of_f_args = type_args_with_state_ts(quote! { ductor::Of<F> });

  let default_fields: Vec<(syn::Ident, syn::Type)> = all_custom
    .iter()
    .filter_map(|(id, ty, init)| {
      matches!(init, FieldInit::Default).then(|| (id.clone(), ty.clone()))
    })
    .collect();
  let take_fields: Vec<(syn::Ident, syn::Type)> = all_custom
    .iter()
    .filter_map(|(id, ty, init)| matches!(init, FieldInit::Take).then(|| (id.clone(), ty.clone())))
    .collect();

  let new_where_preds: Vec<syn::WherePredicate> = default_fields
    .iter()
    .map(|(_, ty)| syn::parse_quote! { #ty: ::core::default::Default })
    .collect();
  let new_where: proc_macro2::TokenStream = if new_where_preds.is_empty() {
    quote! {}
  } else {
    quote! { where #(#new_where_preds),* }
  };

  let take_params: Vec<proc_macro2::TokenStream> = take_fields
    .iter()
    .map(|(ident, ty)| quote! { #ident: #ty })
    .collect();

  let field_lets: Vec<proc_macro2::TokenStream> = all_custom
    .iter()
    .map(|(ident, ty, init)| match init {
      FieldInit::Default => quote! { let #ident: #ty = ::core::default::Default::default(); },
      FieldInit::Take => quote! {},
      FieldInit::Construct(expr) => quote! { let #ident = #expr; },
    })
    .collect();

  let field_refs: Vec<&syn::Ident> = all_custom.iter().map(|(id, _, _)| id).collect();

  let custom_field_moves: Vec<proc_macro2::TokenStream> = all_custom
    .iter()
    .map(|(ident, _, _)| quote! { #ident: self.#ident })
    .collect();

  let expanded = quote! {
    #input

    #[derive(Debug, Clone)]
    pub struct #family_name;
    impl ductor::StateFamily for #family_name {}

    impl #impl_generics ductor::Unit for #name #type_generics #where_clause {
      type State = State;
      type Caps = Caps;
      type Family = #family_name;

      fn state(&self) -> &Self::State { &self.state }
      fn caps(&self) -> &Self::Caps { &self.caps }
    }

    impl #impl_generics #name #type_generics #where_clause {
      pub fn new(state: State, caps: Caps, #(#take_params),*) -> Self
      #new_where
      {
        #(#field_lets)*
        Self {
          state,
          caps,
          #(#field_refs,)*
        }
      }

      /// transition from the current state into `Target`.
      ///
      /// the closure receives the current state (consuming it) and returns
      /// the new state. only compiles when a matching `#[transition(Target)]`
      /// exists on the current variant and all required capabilities are met.
      ///
      /// # example
      /// ```ignore
      /// lock.transition(|Closed| Open)
      /// ```
      pub fn transition<Target, CapMarker, F>(self, f: F) -> <Self as ductor::CanTransitionTo<Target, Caps, F, CapMarker>>::Out
      where
        Self: ductor::CanTransitionTo<Target, Caps, F, CapMarker>,
      {
        <Self as ductor::CanTransitionTo<Target, Caps, F, CapMarker>>::perform_transition(self, f)
      }
    }

    impl<#(#custom_type_idents,)* State, Caps> #name <#(#all_type_args),*> {
      /// transition one component of a multi-state tuple.
      ///
      /// `From` / `To` specify the state change, `Marker` picks the tuple
      /// position (`Is0`, `Is1`, …). other positions are unchanged.
      /// only compiles when the component's family declares the transition
      /// and all required capabilities are present.
      ///
      /// # example
      /// ```ignore
      /// service.transition_at::<Connected, Disconnected, Is1, _, _, _>(|c| {
      ///   let Connected = c;
      ///   Disconnected
      /// })
      /// ```
      pub fn transition_at<From, To, Marker, CapMarker, Out, F>(self, f: F) -> #name <#(#out_type_args),*>
      where
        From: ductor::IsState,
        To: ductor::IsState,
        From::Family: ductor::Transition<From, To>,
        Caps: ductor::Satisfies<<From::Family as ductor::Transition<From, To>>::Requirements, CapMarker>,
        State: ductor::ReplaceStateAt<Marker, From, To, Out>,
        F: FnOnce(From) -> To,
      {
        let state =
          <State as ductor::ReplaceStateAt<Marker, From, To, Out>>::replace_with(self.state, f);

        #name {
          state,
          caps: self.caps,
          #(#custom_field_moves),*
        }
      }

      /// erase a single component in a state tuple to `Of<Family>`.
      ///
      /// like `transition_at`, but replaces the target with its erased form.
      /// the other tuple positions stay unchanged.
      ///
      /// # example
      /// ```ignore
      /// // States<(LoggedIn, Connected)> -> States<(Of<Auth>, Connected)>
      /// svc.into_unknown_at::<LoggedIn, _, _>()
      /// ```
      pub fn into_unknown_at<From, Marker, Out>(self) -> #name <Out, Caps>
      where
        From: ductor::IsState + 'static,
        State: ductor::ReplaceStateAt<Marker, From, ductor::Of<<From as ductor::IsState>::Family>, Out>,
      {
        let state =
          <State as ductor::ReplaceStateAt<Marker, From, ductor::Of<<From as ductor::IsState>::Family>, Out>>::replace_with(self.state, |from| ductor::Of::new(from));

        #name {
          state,
          caps: self.caps,
          #(#custom_field_moves),*
        }
      }

      /// check whether the erased component at a tuple position is `S`.
      pub fn is_at<S, Marker>(&self) -> bool
      where
        S: ductor::IsState + 'static,
        State: ductor::HasState<ductor::Of<<S as ductor::IsState>::Family>, Marker>,
      {
        let of: &ductor::Of<<S as ductor::IsState>::Family> =
          <State as ductor::HasState<ductor::Of<<S as ductor::IsState>::Family>, Marker>>::get_state_ref(&self.state);
        of.is::<S>()
      }

      /// recover a concrete state at a specific tuple position.
      ///
      /// symmetric with `into_unknown_at`. checks at runtime whether the
      /// erased component is `S` and returns `None` if it isn't.
      ///
      /// # example
      /// ```ignore
      /// // States<(Of<Auth>, Connected)> -> Option<States<(LoggedIn, Connected)>>
      /// svc.clone().trim_unknown_at::<LoggedIn, _, _>()
      /// ```
      pub fn trim_unknown_at<S, Marker, Out>(self) -> Option<#name <Out, Caps>>
      where
        S: ductor::IsState + 'static,
        State: ductor::HasState<ductor::Of<<S as ductor::IsState>::Family>, Marker>
          + ductor::ReplaceStateAt<Marker, ductor::Of<<S as ductor::IsState>::Family>, S, Out>,
      {
        let is_match = {
          let of: &ductor::Of<<S as ductor::IsState>::Family> =
            <State as ductor::HasState<ductor::Of<<S as ductor::IsState>::Family>, Marker>>::get_state_ref(&self.state);
          of.is::<S>()
        };
        if !is_match { return None; }
        let new_state =
          <State as ductor::ReplaceStateAt<Marker, ductor::Of<<S as ductor::IsState>::Family>, S, Out>>::replace_with(self.state, |of| of.into_some::<S>().unwrap());
        Some(#name {
          state: new_state,
          caps: self.caps,
          #(#custom_field_moves,)*
        })
      }
    }

    impl<#(#custom_type_idents,)* Caps, F, From, To, CapMarker>
      ductor::CanTransitionTo<To, Caps, F, CapMarker>
      for #name <#(#for_type_args),*>
    where
      From: ductor::IsState,
      To: ductor::IsState,
      From::Family: ductor::Transition<From, To>,
      Caps: ductor::Satisfies<<From::Family as ductor::Transition<From, To>>::Requirements, CapMarker>,
      F: FnOnce(From) -> To,
    {
      type Out = #name <#(#to_type_args),*>;
      fn perform_transition(self, f: F) -> Self::Out {
        #name {
          state: f(self.state),
          caps: self.caps,
          #(#custom_field_moves),*
        }
      }
    }

    impl<#(#custom_type_idents,)* S: ductor::IsState + 'static, Caps> #name <#(#s_args),*>
    {
      /// erase the concrete state into `Of<Family>`.
      ///
      /// the returned unit keeps the same caps and custom fields but
      /// no longer reveals which variant of the state machine it is.
      /// recover with `as_some::<S>()` / `into_some::<S>()`.
      pub fn into_unknown(self) -> #name <#(#of_args),*> {
        #name {
          state: ductor::Of::new(self.state),
          caps: self.caps,
          #(#custom_field_moves,)*
        }
      }
    }

    impl<#(#custom_type_idents,)* State: 'static, Caps> #name <#(#all_type_args),*>
    {
      /// erase any state (including `States<(...)>` tuples) into `Of<UnitFamily>`.
      ///
      /// unlike `into_unknown()`, this doesn't require `IsState`, so it works
      /// for tuple states too. the family marker is the unit's own private family.
      pub fn into_unknown_all(self) -> #name <#(#of_family_args),*> {
        #name {
          state: ductor::Of::new_unchecked(self.state),
          caps: self.caps,
          #(#custom_field_moves,)*
        }
      }
    }

    impl<#(#custom_type_idents,)* F: ductor::StateFamily, Caps> #name <#(#of_f_args),*> {
      /// borrow the state as a specific variant.
      ///
      /// returns `None` if the concrete type doesn't match.
      pub fn as_some<S: ductor::IsState<Family = F> + 'static>(&self) -> Option<&S> {
        self.state.as_some::<S>()
      }

      /// check whether the state is a specific variant.
      pub fn is<S: ductor::IsState<Family = F> + 'static>(&self) -> bool {
        self.state.is::<S>()
      }

      /// unwrap into a concrete state variant.
      ///
      /// returns `None` if the concrete type doesn't match
      /// (the unit is consumed either way).
      pub fn into_some<S: ductor::IsState<Family = F> + 'static>(self) -> Option<#name <#(#s_args),*>> {
        let state = self.state.into_some::<S>()?;
        Some(#name {
          state,
          caps: self.caps,
          #(#custom_field_moves,)*
        })
      }

      /// recover the concrete state (symmetric with `into_unknown`).
      pub fn trim_unknown<S: ductor::IsState<Family = F> + 'static>(self) -> Option<#name <#(#s_args),*>> {
        self.into_some::<S>()
      }
    }
  };
  TokenStream::from(expanded)
}

/// marks a struct as a capability type.
///
/// implements `Capability` for the struct. passes through any `derive(...)`.
///
/// with `as = (A, B, ...)` it becomes a **parent capability**: having it
/// in your `Caps` tuple satisfies requirement bounds for `A`, `B`, etc. at
/// compile time. so you don't need to list all the little ones individually :D
///
/// # attr arguments
/// - `derive(Trait, ...)`: passes derives through.
/// - `as = Type` or `as = (Type, ...)`: proxy targets this cap satisfies.
///
/// # example
/// ```ignore
/// #[cap(derive(Debug, Clone))]
/// pub struct Read;
///
/// #[cap(derive(Debug, Clone))]
/// pub struct Write;
///
/// #[cap(as = (Read, Write))]
/// pub struct Admin;
///
/// // caps!(Admin) now satisfies both Read and Write requirements.
/// ```
#[proc_macro_attribute]
pub fn cap(attr: TokenStream, item: TokenStream) -> TokenStream {
  let args = parse_macro_input!(attr as AttrArgs);
  let mut input = parse_macro_input!(item as ItemStruct);

  if !args.derives.is_empty() {
    let paths = &args.derives;
    input
      .attrs
      .push(syn::parse_quote! { #[derive(#( #paths ),*)] });
  }

  let name = &input.ident;
  let mut output = quote! {
    #input
    impl ductor::Capability for #name {}
  };

  if let Some(ref as_caps) = args.as_caps {
    let targets: Vec<&syn::Type> = match as_caps {
      SpecValue::Single(ty) => vec![ty],
      SpecValue::Tuple(tys) => tys.iter().collect(),
    };
    for target in &targets {
      let proxy_impls = gen_as_proxy_impls(name, target);
      output.extend(proxy_impls);
    }
  }

  TokenStream::from(output)
}

fn gen_as_proxy_impls(name: &Ident, target: &syn::Type) -> proc_macro2::TokenStream {
  let max_size: usize = 7;
  let as_markers: &[Ident] = &[
    format_ident!("IsAs0"),
    format_ident!("IsAs1"),
    format_ident!("IsAs2"),
    format_ident!("IsAs3"),
    format_ident!("IsAs4"),
    format_ident!("IsAs5"),
    format_ident!("IsAs6"),
  ];

  let mut impls = proc_macro2::TokenStream::new();

  for size in 1..=max_size {
    for pos in 0..size {
      let before: Vec<Ident> = (0..pos).map(|i| format_ident!("T{i}")).collect();
      let after: Vec<Ident> = (pos + 1..size).map(|i| format_ident!("T{i}")).collect();

      let tuple_types: Vec<_> = {
        let mut types = Vec::new();
        for b in &before {
          types.push(quote! { #b });
        }
        types.push(quote! { #name });
        for a in &after {
          types.push(quote! { #a });
        }
        types
      };

      let marker = &as_markers[pos];
      impls.extend(quote! {
        impl<#(#before,)* #(#after,)*>
          ductor::Satisfies<#target, ductor::#marker>
          for ductor::Caps<(#(#tuple_types,)*)>
        {}
      });
    }
  }

  impls
}

/// attaches an `impl` block to a specific state/capability combo.
///
/// the `for` param says which state (or state tuple) the methods show up for.
/// the `with` param specifies capability requirements. use `_` or `()` as a
/// wildcard for "any state" or "any cap".
///
/// methods with `#[transit]` inside a `#[spec]` block get their return types
/// rewritten to reflect the new state and caps.
///
/// # attr arguments
/// - `for = StateType` -- single-state spec.
/// - `for = (A, B, ...)` -- multi-state spec.
/// - `with = CapType` -- single-cap requirement.
/// - `with = (A, B, ...)` -- multi-cap requirement.
/// - use `_` or `()` as a wildcard.
///
/// # example
/// ```ignore
/// #[spec(for = Connected, with = NetCap)]
/// impl Network {
///   #[transit(to = Disconnected)]
///   pub fn disconnect(self) { ... }
/// }
/// ```
#[proc_macro_attribute]
pub fn spec(attr: TokenStream, item: TokenStream) -> TokenStream {
  let args = match syn::parse::<SpecArgs>(attr) {
    Ok(a) => a,
    Err(e) => return e.to_compile_error().into(),
  };
  let mut input = parse_macro_input!(item as syn::ItemImpl);

  let base_name = if let syn::Type::Path(tp) = &*input.self_ty {
    tp.path.segments.last().map(|s| s.ident.clone())
  } else {
    None
  };

  let mut generics = input.generics.clone();

  let mut gen_counter: usize = 0;
  let mut next_gen = || {
    gen_counter += 1;
    format_ident!("T{}", gen_counter)
  };

  let mut input_state_components: Vec<syn::Type> = Vec::new();

  let state_ty: syn::Type = if let Some(spec_for) = args.spec_for {
    match spec_for {
      SpecValue::Single(ty) => {
        if is_wildcard(&ty) {
          let g = next_gen();
          let ty_gen: syn::Type = syn::parse_quote! { #g };
          generics
            .params
            .push(syn::parse_quote! { #g: ductor::IsState });
          input_state_components.push(ty_gen.clone());
          ty_gen
        } else {
          input_state_components.push(ty.clone());
          ty
        }
      }
      SpecValue::Tuple(tys) => {
        let mut final_tys = Vec::new();
        for ty in tys {
          if is_wildcard(&ty) {
            let g = next_gen();
            let ty_gen: syn::Type = syn::parse_quote! { #g };
            generics
              .params
              .push(syn::parse_quote! { #g: ductor::IsState });
            final_tys.push(quote! { #ty_gen });
            input_state_components.push(ty_gen);
          } else {
            final_tys.push(quote! { #ty });
            input_state_components.push(ty.clone());
          }
        }
        syn::parse_quote! { ductor::States<(#(#final_tys,)*)> }
      }
    }
  } else {
    let g = next_gen();
    let ty_gen: syn::Type = syn::parse_quote! { #g };
    generics.params.push(syn::parse_quote! { #g });
    input_state_components.push(ty_gen.clone());
    ty_gen
  };

  let mut input_caps_components: Vec<syn::Type> = Vec::new();

  let mut cap_satisfies_bound: Option<(syn::Ident, syn::Type)> = None;

  let caps_ty: syn::Type = if let Some(spec_with) = args.spec_with {
    match spec_with {
      SpecValue::Single(ty) => {
        if is_wildcard(&ty) {
          let g = next_gen();
          let ty_gen: syn::Type = syn::parse_quote! { #g };
          generics.params.push(syn::parse_quote! { #g });
          input_caps_components.push(ty_gen.clone());
          ty_gen
        } else {
          let g = next_gen();
          let ty_gen: syn::Type = syn::parse_quote! { #g };
          generics.params.push(syn::parse_quote! { #g });
          cap_satisfies_bound = Some((g.clone(), ty.clone()));
          ty_gen
        }
      }
      SpecValue::Tuple(tys) => {
        let g = next_gen();
        let ty_gen: syn::Type = syn::parse_quote! { #g };
        generics.params.push(syn::parse_quote! { #g });
        let tuple_ty: syn::Type = syn::parse_quote! { (#(#tys,)*) };
        cap_satisfies_bound = Some((g.clone(), tuple_ty.clone()));
        ty_gen
      }
    }
  } else {
    let g = next_gen();
    let ty_gen: syn::Type = syn::parse_quote! { #g };
    generics.params.push(syn::parse_quote! { #g });
    input_caps_components.push(ty_gen.clone());
    ty_gen
  };

  for item in &mut input.items {
    if let syn::ImplItem::Fn(method) = item {
      if let Some((ref g, ref req)) = cap_satisfies_bound {
        method
          .sig
          .generics
          .params
          .push(syn::parse_quote! { CapMarker });
        method
          .sig
          .generics
          .make_where_clause()
          .predicates
          .push(syn::parse_quote! { #g: ductor::Satisfies<#req, CapMarker> });
      }

      let mut transit_attr_idx = None;
      for (i, attr) in method.attrs.iter().enumerate() {
        if attr.path().is_ident("transit") {
          transit_attr_idx = Some(i);
          break;
        }
      }

      if let Some(idx) = transit_attr_idx {
        let attr = method.attrs.remove(idx);
        let transit_args: TransitArgs = attr.parse_args().expect("Failed to parse #[transit] args");

        let target_state_ty: syn::Type = if let Some(to) = transit_args.to {
          match to {
            SpecValue::Single(ty) => {
              if is_wildcard(&ty) {
                input_state_components[0].clone()
              } else {
                ty
              }
            }
            SpecValue::Tuple(tys) => {
              let mut final_tys = Vec::new();
              for (i, ty) in tys.into_iter().enumerate() {
                if is_wildcard(&ty) {
                  if let Some(comp) = input_state_components.get(i) {
                    final_tys.push(quote! { #comp });
                  } else {
                    final_tys.push(quote! { () });
                  }
                } else {
                  final_tys.push(quote! { #ty });
                }
              }
              syn::parse_quote! { ductor::States<(#(#final_tys,)*)> }
            }
          }
        } else {
          state_ty.clone()
        };

        let target_caps_ty: syn::Type = if let Some(with) = transit_args.with {
          match with {
            SpecValue::Single(ty) => {
              if is_wildcard(&ty) {
                caps_ty.clone()
              } else {
                syn::parse_quote! { ductor::Caps<#ty> }
              }
            }
            SpecValue::Tuple(tys) => {
              let mut final_tys = Vec::new();
              for (i, ty) in tys.into_iter().enumerate() {
                if is_wildcard(&ty) {
                  if let Some(comp) = input_caps_components.get(i) {
                    final_tys.push(quote! { #comp });
                  } else {
                    final_tys.push(quote! { () });
                  }
                } else {
                  final_tys.push(quote! { #ty });
                }
              }
              while final_tys.len() < 6 {
                final_tys.push(quote! { () });
              }
              syn::parse_quote! { ductor::Caps<#(#final_tys),*> }
            }
          }
        } else {
          caps_ty.clone()
        };

        if let Some(ref name) = base_name {
          method.sig.output = syn::parse_quote! { -> #name <#target_state_ty, #target_caps_ty> };
        }
      }
    }
  }

  input.generics = generics;

  if let syn::Type::Path(ref mut tp) = *input.self_ty {
    if let Some(last) = tp.path.segments.last_mut() {
      match &mut last.arguments {
        syn::PathArguments::None => {
          last.arguments =
            syn::PathArguments::AngleBracketed(syn::parse_quote! { <#state_ty, #caps_ty> });
        }
        _ => {}
      }
    }
  }

  quote! { #input }.into()
}

fn is_wildcard(ty: &syn::Type) -> bool {
  match ty {
    syn::Type::Path(tp) => tp.path.is_ident("_") || tp.path.is_ident("AnyState"),
    syn::Type::Tuple(tt) => tt.elems.is_empty(),
    _ => false,
  }
}

enum SpecValue {
  Single(syn::Type),
  Tuple(Vec<syn::Type>),
}

struct SpecArgs {
  spec_for: Option<SpecValue>,
  spec_with: Option<SpecValue>,
}

struct TransitArgs {
  to: Option<SpecValue>,
  with: Option<SpecValue>,
}

impl Parse for SpecArgs {
  fn parse(input: ParseStream) -> syn::Result<Self> {
    let mut spec_for = None;
    let mut spec_with = None;

    while !input.is_empty() {
      let key = Ident::parse_any(input)?;
      input.parse::<Token![=]>()?;

      let val = if input.peek(syn::token::Paren) {
        let content;
        parenthesized!(content in input);
        let tys: Punctuated<syn::Type, Token![,]> =
          content.parse_terminated(syn::Type::parse, Token![,])?;
        SpecValue::Tuple(tys.into_iter().collect())
      } else {
        SpecValue::Single(input.parse()?)
      };

      if key == "for" {
        spec_for = Some(val);
      } else if key == "with" {
        spec_with = Some(val);
      }

      if input.peek(Token![,]) {
        input.parse::<Token![,]>()?;
      }
    }

    Ok(SpecArgs {
      spec_for,
      spec_with,
    })
  }
}

impl Parse for TransitArgs {
  fn parse(input: ParseStream) -> syn::Result<Self> {
    let mut to = None;
    let mut with = None;

    while !input.is_empty() {
      let key = Ident::parse_any(input)?;
      input.parse::<Token![=]>()?;

      let val = if input.peek(syn::token::Paren) {
        let content;
        parenthesized!(content in input);
        let tys: Punctuated<syn::Type, Token![,]> =
          content.parse_terminated(syn::Type::parse, Token![,])?;
        SpecValue::Tuple(tys.into_iter().collect())
      } else {
        SpecValue::Single(input.parse()?)
      };

      if key == "to" {
        to = Some(val);
      } else if key == "with" {
        with = Some(val);
      }

      if input.peek(Token![,]) {
        input.parse::<Token![,]>()?;
      }
    }

    Ok(TransitArgs { to, with })
  }
}
