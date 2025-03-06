use std::collections::HashMap;

use alloy::sol_types::SolCall;
use heck::ToSnakeCase;
use proc_macro::TokenStream;
use quote::quote;
use revm::primitives::{Account, Address, ExecutionResult, ResultAndState, TxEnv};
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    spanned::Spanned,
    Attribute, GenericParam, Generics, Ident, ItemStruct, Lifetime, Token, Type, TypeParam,
    Visibility,
};
mod types;
use crate::types::CallOutput;

/// Procedural macro to generate the contract struct with injected lifetimes and two type parameters
#[proc_macro]
pub fn contract(input: TokenStream) -> TokenStream {
    // Parse the input tokens into the ContractInput struct
    let contract_input = parse_macro_input!(input as ContractInput);

    let attrs = contract_input.attrs;
    let vis = contract_input.vis;
    let name = contract_input.name;
    let type_params = contract_input.type_params;

    // Ensure exactly two type parameters are provided
    if type_params.len() != 2 {
        return syn::Error::new(
            contract_input.span,
            "Expected exactly two type parameters, e.g., <T1, T2>",
        )
        .to_compile_error()
        .into();
    }

    let type_param1 = &type_params[0];
    let type_param2 = &type_params[1];

    // Generate the struct definition with injected lifetimes and two type parameters
    let struct_def = quote! {
        #(#attrs)*
        #vis struct #name<'b, 'a, #type_param1, #type_param2>
        where
            #type_param2: revm::Database,
            'a: 'b
        {
            pub evm: &'b mut Evm<'a, #type_param1, #type_param2>,
            pub address: alloy::primitives::Address,
        }

        impl<'b, 'a, #type_param1, #type_param2> #name<'b, 'a, #type_param1, #type_param2>
        where
            #type_param2: revm::Database,
            'a: 'b
        {

            pub fn new(address: alloy::primitives::Address, evm: &'b mut Evm<'a, #type_param1, #type_param2>) -> Self {
                Self { address, evm }
            }

        }

    };

    TokenStream::from(struct_def)
}

/// Procedural macro to add call methods to the struct
#[proc_macro_attribute]
pub fn calls(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the input struct
    let input = parse_macro_input!(item as ItemStruct);
    let struct_name = &input.ident;
    let generics = &input.generics;

    // Parse the attribute arguments as a list of types
    let type_list = parse_macro_input!(attr as TypeList);

    // Generate methods for each type in the calls list
    let mut methods = Vec::new();

    // Extract lifetimes and type parameters from the struct's generics
    let (lifetimes, type_params) = extract_lifetimes_and_type_params(generics);

    // Ensure exactly two lifetimes are present
    if lifetimes.len() != 2 {
        return syn::Error::new(
            generics.span(),
            "Expected exactly two lifetimes in the struct definition",
        )
        .to_compile_error()
        .into();
    }

    // Ensure exactly two type parameters are present
    if type_params.len() != 2 {
        return syn::Error::new(
            generics.span(),
            "Expected exactly two type parameters in the struct definition",
        )
        .to_compile_error()
        .into();
    }

    let lifetime_b = &lifetimes[0];
    let lifetime_a = &lifetimes[1];
    let type_param1 = &type_params[0];
    let type_param2 = &type_params[1];

    for parsed_type in type_list.types {
        // Convert the type to a token stream and then to a string
        let type_tokens = quote!(#parsed_type);
        let type_str = type_tokens.to_string();

        // Remove "Call" from the end of the type name to create the method name
        let method_name_str = if type_str.ends_with("Call") {
            &type_str[..type_str.len() - 4] // Remove "Call"
        } else {
            &type_str
        };

        let method_name_str = method_name_str.to_snake_case();
        // Convert the method name string to an Ident
        let method_name = Ident::new(&method_name_str, parsed_type.span());

        // Generate the method
        let method = quote! {

            /// Note that the `tx_env` `data` & `transact_to` are provided by the implementation.
            pub fn #method_name(&mut self, call: #parsed_type, tx_env: revm::primitives::TxEnv) -> revm::primitives::EVMResultGeneric<crate::types::CallOutput<#parsed_type>, #type_param2::Error> {

                let mut tx_env = tx_env;

                tx_env.data = call.abi_encode().into();
                tx_env.transact_to = revm::primitives::TxKind::Call(self.address);

                let tx = self.evm.tx_mut();
                *tx = tx_env;

                let result_and_state = self.evm.transact()?;

                Ok(types::process_transact_result(result_and_state))
            }
        };

        methods.push(method);
    }

    // Generate generic parameters for the impl block
    let impl_generics = quote! { <#lifetime_b, #lifetime_a, #type_param1, #type_param2> };

    // Generate the impl block with lifetimes and type parameters
    let expanded = quote! {
        // Keep the original struct definition
        #input

        impl #impl_generics #struct_name<#lifetime_b, #lifetime_a, #type_param1, #type_param2>
        where
            #type_param2: revm::Database,
        {
            #(#methods)*
        }
    };

    TokenStream::from(expanded)
}

/// Struct to parse the input of the `contract!` macro
struct ContractInput {
    attrs: Vec<Attribute>,
    vis: Visibility,
    name: Ident,
    type_params: Vec<TypeParam>,
    span: proc_macro2::Span,
}

impl Parse for ContractInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Parse attributes
        let attrs = input.call(Attribute::parse_outer)?;
        // Parse visibility (e.g., pub)
        let vis: Visibility = input.parse()?;
        // Parse struct name
        let name: Ident = input.parse()?;
        // Parse generics (type parameters)
        let generics: Generics = input.parse()?;

        // Extract type parameters
        let type_params = extract_type_params(&generics)?;

        Ok(ContractInput {
            attrs,
            vis,
            name,
            type_params,
            span: input.span(),
        })
    }
}

/// Custom struct to parse a list of types for the `calls` macro
struct TypeList {
    types: Punctuated<Type, Token![,]>,
}

impl Parse for TypeList {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let types = Punctuated::parse_terminated(input)?;
        Ok(TypeList { types })
    }
}

/// Helper function to extract lifetimes and type parameters from Generics
fn extract_lifetimes_and_type_params(generics: &Generics) -> (Vec<Lifetime>, Vec<Ident>) {
    let mut lifetimes = Vec::new();
    let mut type_params = Vec::new();

    for param in &generics.params {
        match param {
            GenericParam::Lifetime(lifetime) => lifetimes.push(lifetime.lifetime.clone()),
            GenericParam::Type(type_param) => type_params.push(type_param.ident.clone()),
            GenericParam::Const(_) => {}
        }
    }

    (lifetimes, type_params)
}

/// Helper function to extract type parameters from Generics
fn extract_type_params(generics: &Generics) -> syn::Result<Vec<TypeParam>> {
    let mut type_params = Vec::new();

    for param in &generics.params {
        if let GenericParam::Type(type_param) = param {
            type_params.push(type_param.clone());
        }
    }

    Ok(type_params)
}
