extern crate proc_macro;
extern crate proc_macro2;
#[macro_use]
extern crate syn;
extern crate quote;
use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{
    punctuated::Punctuated, Data, DeriveInput, Expr, ExprLit, FnArg, Ident, ItemFn, Lit, Meta,
    ReturnType, Signature, Type,
};

#[proc_macro_derive(Steel)]
pub fn derive_steel(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    match &input.data {
        Data::Struct(_) | Data::Enum(_) => {
            let gen = quote! {
                impl steel::rvals::Custom for #name {}
            };

            return gen.into();
        }
        _ => {
            let output = quote! { #input };
            output.into()
        }
    }
}

fn parse_key_value_pair(args: &Punctuated<Meta, Token![,]>) -> (String, String) {
    for nested_meta in args.iter() {
        if let Meta::NameValue(n) = nested_meta {
            let key = n.path.get_ident().unwrap().to_string();
            if let Expr::Lit(ExprLit {
                lit: Lit::Str(s), ..
            }) = &n.value
            {
                return (key, s.value());
            }
        }

        panic!("Expected a key value pair");
    }

    panic!("Expected a key value pair");
}

fn parse_doc_comment(input: ItemFn) -> Option<String> {
    let maybe_str_literals = input
        .attrs
        .into_iter()
        .filter_map(|attr| match attr.meta {
            Meta::NameValue(name_value) if name_value.path.is_ident("doc") => {
                Some(name_value.value)
            }
            _ => None,
        })
        .map(|expr| match expr {
            Expr::Lit(ExprLit {
                lit: Lit::Str(s), ..
            }) => Ok(s.value()),
            e => Err(e),
        })
        .collect::<Result<Vec<_>, _>>();

    let literals = match maybe_str_literals {
        Ok(lits) => lits,
        Err(_) => {
            return None;
            // Error::new(expr.span(), "Doc comment is not a string literal")
            //     .into_compile_error()
            //     .into()
        }
    };

    if literals.len() == 0 {
        return None;
        // Error::new(ident.span(), "No doc comment found on this type")
        //     .into_compile_error()
        //     .into();
    }

    let trimmed: Vec<_> = literals
        .iter()
        .flat_map(|lit| lit.split("\n").collect::<Vec<_>>())
        .map(|line| line.trim().to_string())
        .collect();

    Some(trimmed.join("\n"))
}

// See REmacs : https://github.com/remacs/remacs/blob/16b6fb9319a6d48fbc7b27d27c3234990f6718c5/rust_src/remacs-macros/lib.rs#L17-L161
// TODO: Pass the new name in to this function
#[proc_macro_attribute]
pub fn function(
    args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let args = parse_macro_input!(args with Punctuated::<Meta, Token![,]>::parse_terminated);

    let (_, value) = parse_key_value_pair(&args);
    let function_name_with_colon = value.clone() + ": ";

    let input = parse_macro_input!(input as ItemFn);

    let mut modified_input = input.clone();
    // let ident = input.sig.ident.clone();
    let sign: Signature = input.clone().sig;

    let maybe_doc_comments = parse_doc_comment(input);

    modified_input.attrs = Vec::new();

    // let sign: Signature = input.clone().sig;

    let return_type: ReturnType = sign.output;

    let ret_val = match return_type {
        ReturnType::Default => quote! {
            Ok(SteelVal::Void)
        },
        ReturnType::Type(_, r) => {
            if let Type::Path(val) = *r {
                let last = val.path.segments.into_iter().last();
                if let Some(last) = last {
                    match last.ident.into_token_stream().to_string().as_str() {
                        "Result" => quote! { res },
                        _ => quote! {
                            res.into_steelval()
                        },
                    }
                } else {
                    quote! {
                        Ok(SteelVal::Void)
                    }
                }
            } else {
                quote! {
                    res.into_steelval()
                }
            }
        }
    };

    let mut type_vec: Vec<Box<Type>> = Vec::new();

    for arg in sign.inputs {
        if let FnArg::Typed(pat_ty) = arg.clone() {
            type_vec.push(pat_ty.ty);
        }
    }

    let arity_number = type_vec.len();

    let conversion_functions = type_vec.clone().into_iter().map(|x| {
        if let Type::Reference(_) = *x {
            quote! { primitive_as_ref }
        } else {
            quote! { from_steelval }
        }
    });

    let arg_enumerate = type_vec.into_iter().enumerate();
    let arg_type = arg_enumerate.clone().map(|(_, x)| x);
    let arg_index = arg_enumerate.clone().map(|(i, _)| i);
    let function_names_with_colon = std::iter::repeat(function_name_with_colon);
    let function_name = sign.ident.clone();
    let _arity_name = Ident::new(
        &(function_name.to_string().to_uppercase() + "_ARITY"),
        sign.ident.span(),
    );
    let copied_function_name = Ident::new(
        &("steel_".to_string() + &function_name.to_string()),
        sign.ident.span(),
    );

    let doc_name = Ident::new(
        &(function_name.to_string().to_uppercase() + "_DEFINITION"),
        sign.ident.span(),
    );

    let definition_struct = if let Some(doc) = maybe_doc_comments {
        quote! {
            pub const #doc_name: crate::steel_vm::builtin::NativeFunctionDefinition = crate::steel_vm::builtin::NativeFunctionDefinition {
                name: #value,
                func: #copied_function_name,
                arity: crate::steel_vm::builtin::Arity::Exact(#arity_number),
                doc: Some(crate::steel_vm::builtin::MarkdownDoc(#doc))
            };
        }
    } else {
        quote! {
            pub const #doc_name: crate::steel_vm::builtin::NativeFunctionDefinition = crate::steel_vm::builtin::NativeFunctionDefinition {
                name: #value,
                func: #copied_function_name,
                arity: crate::steel_vm::builtin::Arity::Exact(#arity_number),
                doc: None
            };
        }
    };

    let output = quote! {
        // Not sure why, but it says this is unused even when generating functions
        // marked as pub
        #[allow(dead_code)]
        #modified_input

        #definition_struct

        pub fn #copied_function_name(args: &[SteelVal]) -> std::result::Result<SteelVal, crate::rerrs::SteelErr> {

            use crate::rvals::{IntoSteelVal, FromSteelVal, PrimitiveAsRef};


            if args.len() != #arity_number {
                crate::stop!(ArityMismatch => format!("{} expected {} arguments, got {}", #value, #arity_number.to_string(), args.len()))
            }

            let res = #function_name(
                #(
                    // TODO: Distinguish reference types here if possible - make a special implementation
                    // for builtin pointer types here to distinguish them
                    <#arg_type>::#conversion_functions(&args[#arg_index])
                        .map_err(|mut err| {
                            err.prepend_message(#function_names_with_colon);
                            err.set_kind(crate::rerrs::ErrorKind::TypeMismatch);
                            err
                        } )?,
                )*
            );

            #ret_val
        }
    };

    // Uncomment this to see the generated code
    // eprintln!("{}", output.to_string());

    output.into()
}
