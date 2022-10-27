use crate::{AccountField, AccountsStruct};
use quote::quote;
use std::iter;
use syn::parse::Parse;
use syn::punctuated::Punctuated;
use syn::{ConstParam, LifetimeDef, Token, TypeParam};
use syn::{GenericParam, PredicateLifetime, WhereClause, WherePredicate};

mod __client_accounts;
mod __cpi_client_accounts;
mod constraints;
mod exit;
mod to_account_infos;
mod to_account_metas;
mod try_accounts;

pub fn generate(accs: &AccountsStruct) -> proc_macro2::TokenStream {
    let impl_try_accounts = try_accounts::generate(accs);
    let impl_to_account_infos = to_account_infos::generate(accs);
    let impl_to_account_metas = to_account_metas::generate(accs);
    let impl_exit = exit::generate(accs);

    let __client_accounts_mod = __client_accounts::generate(accs);
    let __cpi_client_accounts_mod = __cpi_client_accounts::generate(accs);

    let name = &accs.ident;

    let last_symbol = match accs.fields.get(accs.fields.len() - 1).unwrap() {
        AccountField::CompositeField(s) => &s.ident,
        AccountField::Field(f) => &f.ident,
    };

    let ParsedGenerics {
        combined_generics,
        trait_generics,
        struct_generics,
        where_clause,
    } = generics(accs);
    let num_accounts_each: Vec<proc_macro2::TokenStream> = accs
        .fields
        .iter()
        .map(|f: &AccountField| {
            let (num_accounts_current, last_field) = match f {
                AccountField::CompositeField(s) => {
                    let composite_field_name = &s.ident;
                    (
                        quote! {
                            self.#composite_field_name.num_accounts()
                        },
                        &s.ident == last_symbol,
                    )
                }
                AccountField::Field(f) => (
                    quote! {
                        1
                    },
                    &f.ident == last_symbol,
                ),
            };
            let plus_sign = if last_field {
                quote! {}
            } else {
                quote! { + }
            };
            quote! {
                #num_accounts_current #plus_sign
            }
        })
        .collect();

    quote! {
        impl<#combined_generics> anchor_lang::NumAccounts for #name <#struct_generics> #where_clause {
             fn num_accounts(&self) -> usize {
                #(#num_accounts_each)*
            }
        }

        #impl_try_accounts
        #impl_to_account_infos
        #impl_to_account_metas
        #impl_exit



        #[cfg(not(feature = "no-client-accounts"))]
        #__client_accounts_mod
        #[cfg(not(feature = "no-cpi-support"))]
        #__cpi_client_accounts_mod
    }
}

fn generics(accs: &AccountsStruct) -> ParsedGenerics {
    let trait_lifetime = accs
        .generics
        .lifetimes()
        .next()
        .cloned()
        .unwrap_or_else(|| syn::parse_str("'info").expect("Could not parse lifetime"));

    let mut where_clause = accs.generics.where_clause.clone().unwrap_or(WhereClause {
        where_token: Default::default(),
        predicates: Default::default(),
    });
    for lifetime in accs.generics.lifetimes().map(|def| &def.lifetime) {
        where_clause
            .predicates
            .push(WherePredicate::Lifetime(PredicateLifetime {
                lifetime: lifetime.clone(),
                colon_token: Default::default(),
                bounds: iter::once(trait_lifetime.lifetime.clone()).collect(),
            }))
    }
    let trait_lifetime = GenericParam::Lifetime(trait_lifetime);

    ParsedGenerics {
        combined_generics: if accs.generics.lifetimes().next().is_some() {
            accs.generics.params.clone()
        } else {
            iter::once(trait_lifetime.clone())
                .chain(accs.generics.params.clone())
                .collect()
        },
        trait_generics: iter::once(trait_lifetime).collect(),
        struct_generics: accs
            .generics
            .params
            .clone()
            .into_iter()
            .map(|param: GenericParam| match param {
                GenericParam::Const(ConstParam { ident, .. })
                | GenericParam::Type(TypeParam { ident, .. }) => GenericParam::Type(TypeParam {
                    attrs: vec![],
                    ident,
                    colon_token: None,
                    bounds: Default::default(),
                    eq_token: None,
                    default: None,
                }),
                GenericParam::Lifetime(LifetimeDef { lifetime, .. }) => {
                    GenericParam::Lifetime(LifetimeDef {
                        attrs: vec![],
                        lifetime,
                        colon_token: None,
                        bounds: Default::default(),
                    })
                }
            })
            .collect(),
        where_clause,
    }
}

struct ParsedGenerics {
    pub combined_generics: Punctuated<GenericParam, Token![,]>,
    pub trait_generics: Punctuated<GenericParam, Token![,]>,
    pub struct_generics: Punctuated<GenericParam, Token![,]>,
    pub where_clause: WhereClause,
}
