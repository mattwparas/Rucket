extern crate proc_macro;
extern crate proc_macro2;
#[macro_use]
extern crate syn;
extern crate quote;
extern crate steel;
use proc_macro::TokenStream;
use quote::quote;
use syn::FnArg;
use syn::ItemFn;
use syn::ReturnType;
use syn::Signature;
use syn::Type;
use syn::{Data, DataStruct, DeriveInput, Fields};

/// Derives the `CustomType` trait for the given struct, and also implements the
/// `StructFunctions` trait, which generates the predicate, constructor, and the getters
/// and setters for using the struct inside the interpreter.
#[proc_macro_derive(Scheme)]
pub fn derive_scheme(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = &input.ident;

    if let Data::Struct(DataStruct {
        fields: Fields::Unnamed(_),
        ..
    }) = &input.data
    {
        let gen = quote! {

            impl crate::rvals::CustomType for #name {
                fn box_clone(&self) -> Box<dyn CustomType> {
                    Box::new((*self).clone())
                }
                fn as_any(&self) -> Box<dyn Any> {
                    Box::new((*self).clone())
                }
                fn new_steel_val(&self) -> SteelVal {
                    SteelVal::Custom(Box::new(self.clone()))
                }
            }
            impl From<#name> for SteelVal {
                fn from(val: #name) -> SteelVal {
                    val.new_steel_val()
                }
            }

            impl From<&SteelVal> for #name {
                fn from(val: &SteelVal) -> #name {
                    unwrap!(val.clone(), #name).unwrap()
                }
            }

            impl TryFrom<SteelVal> for #name {
                type Error = SteelErr;
                fn try_from(value: SteelVal) -> std::result::Result<#name, Self::Error> {
                    unwrap!(value.clone(), #name)
                }
            }
        };

        return gen.into();
    };

    let fields = match &input.data {
        Data::Struct(DataStruct {
            fields: Fields::Named(fields),
            ..
        }) => &fields.named,
        Data::Struct(DataStruct {
            fields: Fields::Unnamed(fields),
            ..
        }) => &fields.unnamed,
        _ => panic!("expected a struct with named or unnamed fields"),
    };

    let field_name = fields.iter().map(|field| &field.ident);
    let field_name2 = field_name.clone();
    let field_type = fields.iter().map(|field| &field.ty);
    let field_type2 = field_type.clone();

    let gen = quote! {

        impl crate::rvals::CustomType for #name {
            fn box_clone(&self) -> Box<dyn CustomType> {
                Box::new((*self).clone())
            }
            fn as_any(&self) -> Box<dyn Any> {
                Box::new((*self).clone())
            }
            fn new_steel_val(&self) -> SteelVal {
                SteelVal::Custom(Box::new(self.clone()))
            }
        }

        impl From<#name> for SteelVal {
            fn from(val: #name) -> SteelVal {
                val.new_steel_val()
            }
        }


        impl From<&SteelVal> for #name {
            fn from(val: &SteelVal) -> #name {
                unwrap!(val.clone(), #name).unwrap()
            }
        }

        impl TryFrom<SteelVal> for #name {
            type Error = SteelErr;
            fn try_from(value: SteelVal) -> std::result::Result<#name, Self::Error> {
                unwrap!(value.clone(), #name)
            }
        }


        impl crate::rvals::StructFunctions for #name {
            fn generate_bindings() -> Vec<(String, SteelVal)> {
                use std::convert::TryFrom;
                use steel::rvals::SteelVal;
                use steel::rerrs::SteelErr;
                use steel::unwrap;
                use steel::stop;
                use std::rc::Rc;
                let mut vec_binding = vec![];

                // generate predicate
                let name = concat!(stringify!(#name), "?").to_string();
                let func =
                        SteelVal::FuncV(|args: Vec<Rc<SteelVal>>| -> Result<Rc<SteelVal>, SteelErr> {
                        let mut args_iter = args.into_iter();
                        if let Some(first) = args_iter.next() {
                            return Ok(Rc::new(SteelVal::BoolV(unwrap!((*first).clone(), #name).is_ok())));
                        }
                        stop!(ArityMismatch => "set! expected 2 arguments"); // TODO
                    });
                vec_binding.push((name, func));

                // generate constructor
                let name = concat!(stringify!(#name)).to_string();
                let func =
                        SteelVal::FuncV(|args: Vec<Rc<SteelVal>>| -> Result<Rc<SteelVal>, SteelErr> {
                            let mut args_iter = args.into_iter();
                            let new_struct = #name {
                                #(
                                    #field_name2: {
                                    if let Some(arg) = args_iter.next() {
                                        match arg.as_ref() {
                                            SteelVal::Custom(_) => unwrap!((*arg).clone(), #field_type2)?,
                                            _ => <#field_type2>::try_from(&(*arg).clone())?
                                        }
                                    } else {
                                        stop!(ArityMismatch => "Struct not given correct arguments");
                                    }},

                                )*
                            };
                            Ok(Rc::new(new_struct.new_steel_val()))
                        });
                vec_binding.push((name, func));

                #(
                    // generate setters
                    let name = concat!("set-", stringify!(#name), "-", stringify!(#field_name), "!").to_string();
                    let func =
                            SteelVal::FuncV(|args: Vec<Rc<SteelVal>>| -> Result<Rc<SteelVal>, SteelErr> {
                            let mut args_iter = args.into_iter();
                            if let (Some(first), Some(second)) = (args_iter.next(), args_iter.next()) {
                                let mut my_struct = unwrap!((*first).clone(), #name)?;
                                my_struct.#field_name = match second.as_ref() {
                                    SteelVal::Custom(_) => {
                                        unwrap!((*second).clone(), #field_type)?
                                    },
                                    _ => {
                                        <#field_type>::try_from(&(*second).clone())?
                                        }
                                };
                                return Ok(Rc::new(my_struct.new_steel_val()));
                            } else {
                                stop!(ArityMismatch => "set! expected 2 arguments");
                            }
                        });
                    vec_binding.push((name, func));

                    // generate getters
                    let name = concat!(stringify!(#name), "-", stringify!(#field_name)).to_string();
                    let func =
                            SteelVal::FuncV(|args: Vec<Rc<SteelVal>>| -> Result<Rc<SteelVal>, SteelErr> {
                            let mut args_iter = args.into_iter();
                            if let Some(first) = args_iter.next() {
                                let my_struct = unwrap!((*first).clone(), #name)?;
                                let return_val: SteelVal = my_struct.#field_name.into();
                                return Ok(Rc::new(return_val));
                            }
                            stop!(ArityMismatch => "set! expected 2 arguments");
                        });
                    vec_binding.push((name, func));
                ) *
                vec_binding
            }
        }
    };

    gen.into()
}

/// Catch all attribute for embedding structs into the `SteelInterpreter`.
/// Derives Scheme, Clone, and Debug on the attached struct.
/// # Example
/// ```ignore
///
/// #[steel]
/// pub struct Foo {
///     bar: f64,
///     qux: String
/// }
///
/// ```
#[proc_macro_attribute]
pub fn steel(
    _metadata: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let input: proc_macro2::TokenStream = input.into();
    let output = quote! {
        #[derive(Clone, Debug, Scheme)]
        #input
    };
    output.into()
}

// See REmacs : https://github.com/remacs/remacs/blob/16b6fb9319a6d48fbc7b27d27c3234990f6718c5/rust_src/remacs-macros/lib.rs#L17-L161
// attribute to transform function into a Steel Embeddable FuncV
#[proc_macro_attribute]
pub fn function(
    _metadata: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as ItemFn);

    let mut modified_input = input.clone();
    modified_input.attrs = Vec::new();

    // This snags the `Signature` from the function definition
    let sign: Signature = input.clone().sig;

    // This is the `ReturnType`
    let return_type: ReturnType = sign.output;

    let ret_val = match return_type {
        ReturnType::Default => quote! {
            Ok(Rc::new(SteelVal::Void))
        },
        ReturnType::Type(_, _) => quote! {
            Ok(Rc::new(SteelVal::try_from(res)?))
        },
    };

    let mut type_vec: Vec<Box<Type>> = Vec::new();

    for arg in sign.inputs {
        if let FnArg::Typed(pat_ty) = arg.clone() {
            type_vec.push(pat_ty.ty);
        }
    }

    let arg_enumerate = type_vec.into_iter().enumerate();
    let arg_type = arg_enumerate.clone().map(|(_, x)| x);
    let arg_index = arg_enumerate.clone().map(|(i, _)| i);
    let function_name = sign.ident;

    let output = quote! {
        pub fn #function_name(args: Vec<Rc<SteelVal>>) -> std::result::Result<Rc<SteelVal>, SteelErr> {
            #modified_input

            let res = #function_name(
                #(
                    unwrap!((*(args[#arg_index])).clone(), #arg_type)?,
                )*
            );

            #ret_val
        }
    };

    // eprintln!("{}", output.to_string());

    output.into()
}
