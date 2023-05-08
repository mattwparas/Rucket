use crate::{builtin_stop, stop};
use crate::{
    gc::{get_object_count, Gc},
    rvals::FutureResult,
};
use crate::{
    rvals::{poll_future, Result, SteelVal},
    steel_vm::vm::VmCore,
};

use futures::future::join_all;

// use async_compat::Compat;

use futures::FutureExt;

pub struct MetaOperations {}
impl MetaOperations {
    pub fn inspect_bytecode() -> SteelVal {
        SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
            // let mut error_message = String::new();

            if args.len() == 1 {
                match &args[0] {
                    SteelVal::Closure(bytecode_lambda) => {
                        crate::core::instructions::pretty_print_dense_instructions(
                            &bytecode_lambda.body_exp(),
                        );
                        Ok(SteelVal::Void)
                    }
                    SteelVal::ContractedFunction(c) => {
                        if let SteelVal::Closure(bytecode_lambda) = &c.function {
                            crate::core::instructions::pretty_print_dense_instructions(
                                &bytecode_lambda.body_exp(),
                            );
                            Ok(SteelVal::Void)
                        } else {
                            stop!(TypeMismatch => "inspect-bytecode expects a closure object");
                        }
                    }
                    _ => {
                        stop!(TypeMismatch => "inspect-bytecode expects a closure object");
                    }
                }
            } else {
                stop!(ArityMismatch => "inspect-bytecode takes only one argument");
            }
        })
    }

    pub fn active_objects() -> SteelVal {
        SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
            if !args.is_empty() {
                stop!(ArityMismatch => "active-object-count expects only one argument");
            }
            Ok(SteelVal::IntV(get_object_count() as isize))
        })
    }

    pub fn memory_address() -> SteelVal {
        SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
            if args.len() != 1 {
                stop!(ArityMismatch => "memory address takes one address")
            }

            // let memory_address = format!("{:p}", &args[0].as_ptr());

            Ok(SteelVal::StringV("TODO".into())) // TODO come back here
        })
    }

    pub fn assert_truthy() -> SteelVal {
        SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
            if args.len() != 1 {
                stop!(ArityMismatch => "assert takes one argument")
            }
            if let SteelVal::BoolV(true) = &args[0] {
                Ok(SteelVal::Void)
            } else {
                panic!("Value given not true!")
            }
        })
    }

    // Uses a generic executor w/ the compat struct in order to allow tokio ecosystem functions inside
    // the interpreter
    // pub fn exec_async() -> SteelVal {
    //     SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
    //         let mut executor = LocalPool::new();

    //         let joined_futures: Vec<_> = args
    //             .into_iter()
    //             .map(|x| {
    //                 if let SteelVal::FutureV(f) = x {
    //                     Ok(f.unwrap().into_shared())
    //                 } else {
    //                     stop!(TypeMismatch => "exec-async given non future")
    //                 }
    //             })
    //             .collect::<Result<Vec<_>>>()?;

    //         let futures = join_all(joined_futures);

    //         // spawner.spawn_local_obj(joined_futures);

    //         // let future = LocalFutureObj::new(Box::pin(async {}));
    //         // spawner.spawn_local_obj(future);
    //         // executor.run_until(future);
    //         Ok(SteelVal::VectorV(Gc::new(
    //             executor
    //                 .run_until(Compat::new(futures))
    //                 .into_iter()
    //                 .collect::<Result<_>>()?,
    //         )))
    //     })
    // }

    pub fn poll_value() -> SteelVal {
        SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
            if args.len() != 1 {
                stop!(Generic => "poll! only takes one argument");
            }

            if let SteelVal::FutureV(fut) = args[0].clone() {
                let fut = fut.unwrap();
                let ready = poll_future(fut.into_shared());
                match ready {
                    Some(v) => v,
                    None => Ok(SteelVal::BoolV(false)),
                }
            } else {
                stop!(Generic => "poll! accepts futures only");
            }
        })
    }

    pub fn block_on() -> SteelVal {
        SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
            if args.len() != 1 {
                stop!(Generic => "block-on! only takes one argument");
            }

            if let SteelVal::FutureV(fut) = args[0].clone() {
                loop {
                    let fut = fut.unwrap();
                    let ready = poll_future(fut.into_shared());
                    if let Some(v) = ready {
                        return v;
                    }
                }
            } else {
                stop!(Generic => "block-on! accepts futures only");
            }
        })
    }

    pub fn join_futures() -> SteelVal {
        SteelVal::FuncV(|args: &[SteelVal]| -> Result<SteelVal> {
            if args.is_empty() {
                stop!(Generic => "join! requires at least one argument");
            }

            let joined_futures: Vec<_> = args
                .iter()
                .map(|x| {
                    if let SteelVal::FutureV(f) = x {
                        Ok(f.unwrap().into_shared())
                    } else {
                        stop!(TypeMismatch => "join! given non future")
                    }
                })
                .collect::<Result<Vec<_>>>()?;

            let futures = join_all(joined_futures).map(|x| {
                x.into_iter()
                    .collect::<Result<im_rc::Vector<_>>>()
                    .map(|x| SteelVal::VectorV(Gc::new(x)))
            });

            Ok(SteelVal::FutureV(Gc::new(FutureResult::new(Box::pin(
                futures,
            )))))
        })
    }
}

pub(crate) fn steel_box(ctx: &mut VmCore, args: &[SteelVal]) -> Option<Result<SteelVal>> {
    if args.len() != 1 {
        builtin_stop!(ArityMismatch => "box takes one argument, found: {}", args.len())
    }

    let arg = args[0].clone();

    // Allocate the variable directly on the heap
    let allocated_var = ctx.thread.heap.allocate(
        arg,
        ctx.thread.stack.iter(),
        ctx.thread.stack_frames.iter().map(|x| x.function.as_ref()),
        ctx.thread.global_env.roots(),
    );

    Some(Ok(SteelVal::Boxed(allocated_var)))
}
