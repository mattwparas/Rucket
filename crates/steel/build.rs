// build.rs

use std::env;
use std::fs;
use std::path::Path;

use steel_gen::generate_opcode_map;
use steel_gen::OpCode::*;

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("dynamic.rs");
    // fs::write(
    //     &dest_path,
    //     r#"

    //     pub fn message() -> &'static str {
    //         println!("{:?}", OpCode::FUNC);
    //         "Hello, World!"
    //     }
    //     "#,
    // )
    // .unwrap();

    // TODO: Come up with better way for this to make it in
    let patterns: Vec<Vec<(steel_gen::OpCode, usize)>> = vec![
        vec![
            (MOVEREADLOCAL0, 0),
            (LOADINT2, 225),
            (SUB, 2),
            (CALLGLOBAL, 1),
        ],
        vec![(READLOCAL0, 0), (LOADINT1, 219), (SUB, 2), (CALLGLOBAL, 1)],
        vec![(READLOCAL0, 0), (LOADINT2, 225), (LTE, 2), (IF, 7)],
    ];

    fs::write(&dest_path, generate_opcode_map(patterns)).unwrap();
    println!("cargo:rerun-if-changed=build.rs");
}
