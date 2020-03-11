extern crate proc_macro;
extern crate proc_macro2;
#[macro_use]
extern crate syn;
extern crate quote;
extern crate steel;
use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DataStruct, DeriveInput, Fields};

// #[macro_export]
#[proc_macro_derive(Scheme)]
pub fn derive_scheme(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = &input.ident;

    let fields = match &input.data {
        Data::Struct(DataStruct {
            fields: Fields::Named(fields),
            ..
        }) => &fields.named,
        _ => panic!("expected a struct with named fields"),
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


        impl From<SteelVal> for #name {
            fn from(val: SteelVal) -> #name {
                unwrap!(val, #name).unwrap()
            }
        }

        impl crate::rvals::StructFunctions for #name {
            fn generate_bindings() -> Vec<(&'static str, SteelVal)> {
                use std::convert::TryFrom;
                use steel::rvals::SteelVal;
                use steel::rerrs::SteelErr;
                use steel::unwrap;
                use steel::stop;
                let mut vec_binding = vec![];

                // generate predicate
                let name = concat!(stringify!(#name), "?");
                let func =
                        SteelVal::FuncV(|args: Vec<SteelVal>| -> Result<SteelVal, SteelErr> {
                        let mut args_iter = args.into_iter();
                        if let Some(first) = args_iter.next() {
                            return Ok(SteelVal::BoolV(unwrap!(first, #name).is_ok()));
                        }
                        stop!(ArityMismatch => "set! expected 2 arguments");
                    });
                vec_binding.push((name, func));

                // generate constructor
                let name = concat!(stringify!(#name));
                let func =
                        SteelVal::FuncV(|args: Vec<SteelVal>| -> Result<SteelVal, SteelErr> {
                            let mut args_iter = args.into_iter();
                            let new_struct = #name {
                                #(
                                    #field_name2: {
                                    if let Some(arg) = args_iter.next() {
                                        match arg {
                                            SteelVal::Custom(_) => unwrap!(arg, #field_type2)?,
                                            _ => <#field_type2>::try_from(arg)?
                                        }
                                    } else {
                                        stop!(ArityMismatch => "Struct not given correct arguments");
                                    }},

                                )*
                            };
                            Ok(new_struct.new_steel_val())
                        });
                vec_binding.push((name, func));

                #(
                    // generate setters
                    let name = concat!("set-", stringify!(#name), "-", stringify!(#field_name), "!");
                    let func =
                            SteelVal::FuncV(|args: Vec<SteelVal>| -> Result<SteelVal, SteelErr> {
                            let mut args_iter = args.into_iter();
                            if let Some(first) = args_iter.next() {
                                if let Some(second) = args_iter.next() {
                                    let my_struct = unwrap!(first, #name)?;
                                    let new_struct = #name {
                                        #field_name : match second {
                                            SteelVal::Custom(_) => {
                                                unwrap!(second, #field_type)?
                                            },
                                            _ => {
                                                <#field_type>::try_from(second)?
                                                }
                                        },
                                        ..my_struct
                                    };
                                    return Ok(new_struct.new_steel_val());
                                }
                                stop!(ArityMismatch => "set! expected 2 arguments");
                            }
                            stop!(ArityMismatch => "set! expected 2 arguments");
                        });
                    vec_binding.push((name, func));

                    // generate getters
                    let name = concat!(stringify!(#name), "-", stringify!(#field_name));
                    let func =
                            SteelVal::FuncV(|args: Vec<SteelVal>| -> Result<SteelVal, SteelErr> {
                            let mut args_iter = args.into_iter();
                            if let Some(first) = args_iter.next() {
                                let my_struct = unwrap!(first, #name)?;
                                return Ok(my_struct.#field_name.into());
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
