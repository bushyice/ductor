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
}

impl Parse for AttrArgs {
  fn parse(input: ParseStream) -> syn::Result<Self> {
    let mut derives = Vec::new();
    let mut states = None;
    while !input.is_empty() {
      if input.peek(Ident) || input.peek(Token![type]) || input.peek(Token![const]) {
        let id = Ident::parse_any(input)?;
        if id == "derive" {
          let content;
          parenthesized!(content in input);
          let paths: Punctuated<syn::Path, Token![,]> =
            content.parse_terminated(syn::Path::parse, Token![,])?;
          derives.extend(paths);
        } else if id == "states" {
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
          states = Some(val);
        }
      }
      if input.peek(Token![,]) {
        input.parse::<Token![,]>()?;
      } else {
        break;
      }
    }
    Ok(AttrArgs { derives, states })
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

        transition_impls.push(quote! {
          impl ductor::Transition<#variant_name, #target> for #enum_name {
            type Requirements = #req;
          }
        });
      }
    }

    let struct_gen = match fields {
      Fields::Named(f) => quote! {
        #derives
        #visibility struct #variant_name #f
      },
      Fields::Unnamed(f) => quote! {
        #derives
        #visibility struct #variant_name #f;
      },
      Fields::Unit => quote! {
        #derives
        #visibility struct #variant_name;
      },
    };
    generated_structs.push(struct_gen);

    is_state_impls.push(quote! {
      impl ductor::IsState for #variant_name {
        type Family = #enum_name;
      }
    });
  }

  let expanded = quote! {
    #( #generated_structs )*

    #derives
    #visibility struct #enum_name;
    impl ductor::StateFamily for #enum_name {}

    #( #is_state_impls )*
    #( #transition_impls )*
  };

  TokenStream::from(expanded)
}

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
  let mut generic_names = Vec::new();
  for param in &input.generics.params {
    if let GenericParam::Type(t) = param {
      generic_names.push(&t.ident);
    }
  }

  let state_gen = generic_names
    .get(0)
    .expect("Orchestrator must have a State generic");
  let caps_gen = generic_names
    .get(1)
    .expect("Orchestrator must have a Caps generic");

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
      .push(syn::parse_quote! { #state_gen: ductor::Follows<#follows_ty> });
  }

  let family_name = format_ident!("{}Family", name);

  let expanded = quote! {
    #input

    #[derive(Debug, Clone)]
    pub struct #family_name;
    impl ductor::StateFamily for #family_name {}

    impl #impl_generics ductor::Unit for #name #type_generics #where_clause {
      type State = #state_gen;
      type Caps = #caps_gen;
      type Family = #family_name;

      fn state(&self) -> &Self::State { &self.state }
      fn caps(&self) -> &Self::Caps { &self.caps }
    }

    impl #impl_generics #name #type_generics #where_clause {
      pub fn new(state: #state_gen, caps: #caps_gen) -> Self {
        Self { state, caps }
      }

      pub fn transition<Target, CapMarker, F>(self, f: F) -> <Self as ductor::CanTransitionTo<Target, #caps_gen, F, CapMarker>>::Out
      where
        Self: ductor::CanTransitionTo<Target, #caps_gen, F, CapMarker>,
      {
        <Self as ductor::CanTransitionTo<Target, #caps_gen, F, CapMarker>>::perform_transition(self, f)
      }
    }


    impl<State, Caps> #name<State, Caps> {
      pub fn transition_at<From, To, Marker, CapMarker, Out, F>(self, f: F) -> #name<Out, Caps>
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
        }
      }
    }



    impl<#caps_gen, F, From, To, CapMarker> ductor::CanTransitionTo<To, #caps_gen, F, CapMarker> for #name<From, #caps_gen>
    where
      From: ductor::IsState,
      To: ductor::IsState,
      From::Family: ductor::Transition<From, To>,
      #caps_gen: ductor::Satisfies<<From::Family as ductor::Transition<From, To>>::Requirements, CapMarker>,
      F: FnOnce(From) -> To,
    {
      type Out = #name<To, #caps_gen>;
      fn perform_transition(self, f: F) -> Self::Out {
        #name {
          state: f(self.state),
          caps: self.caps,
        }
      }
    }

  };
  TokenStream::from(expanded)
}

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
  quote! {
    #input
    impl ductor::Capability for #name {}
  }
  .into()
}

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
