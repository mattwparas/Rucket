use crate::parser::{
    ast::{Atom, ExprKind, Quote},
    interner::InternedString,
    tokens::TokenType,
};

use super::VisitorMutRefUnit;
use std::collections::HashSet;

/*
Steps for doing having scoped macros

- A requires a macro (or macros) from B
    - Expand A with macros from A
    - Then expand A with macros from B / C / D
        - A must be done first, but then the rest can be done in phases
    - Copy the code for B, mangle it and then include it in A directly
*/

pub fn collect_globals(exprs: &[ExprKind]) -> HashSet<InternedString> {
    let mut global_defs = HashSet::new();

    for expr in exprs {
        match expr {
            ExprKind::Define(d) => {
                if let Some(name) = d.name.atom_identifier() {
                    if name.resolve().starts_with("mangler") {
                        continue;
                    }
                    global_defs.insert(*name);
                }
            }
            ExprKind::Begin(b) => {
                let collected_defs = collect_globals(&b.exprs);
                global_defs.extend(collected_defs);
            }
            _ => {}
        }
    }

    global_defs
}

pub struct NameMangler {
    globals: HashSet<InternedString>,
    prefix: String,
}

impl NameMangler {
    pub fn new(globals: HashSet<InternedString>, prefix: String) -> Self {
        Self { globals, prefix }
    }

    pub fn mangle_vars(&mut self, exprs: &mut [ExprKind]) {
        for expr in exprs {
            self.visit(expr);
        }
    }
}

pub fn mangle_vars_with_prefix(prefix: String, exprs: &mut [ExprKind]) {
    let globals = collect_globals(exprs);

    let mut name_mangler = NameMangler { globals, prefix };

    for expr in exprs {
        name_mangler.visit(expr);
    }
}

impl VisitorMutRefUnit for NameMangler {
    #[inline]
    fn visit_atom(&mut self, a: &mut Atom) {
        if let TokenType::Identifier(i) = &mut a.syn.ty {
            if self.globals.contains(i) {
                let new_str = i.resolve();

                *i = (self.prefix.clone() + new_str).into();

                // i.insert_str(0, &self.prefix);
            }
        }
    }

    /// We don't want quoted values to be mangled since those should match
    /// the real name given
    #[inline]
    fn visit_quote(&mut self, _q: &mut Quote) {}
}

#[cfg(test)]
mod name_mangling_tests {
    use super::*;

    use crate::parser::parser::Parser;

    #[test]
    fn basic_mangling() {
        let expr = r#"
           (define (foo x y z) (let ((a 10) (b 20)) (bar (+ x y z a b))))
           (define (bar applesauce) (+ applesauce 10))
        "#;

        let mut parsed = Parser::parse(expr).unwrap();

        mangle_vars_with_prefix("--test--".to_string(), &mut parsed);

        let expected = Parser::parse(
            r#"
            (define (--test--foo x y z) (let ((a 10) (b 20)) (--test--bar (+ x y z a b))))
            (define (--test--bar applesauce) (+ applesauce 10))
        "#,
        )
        .unwrap();

        assert_eq!(parsed, expected);
    }

    #[test]
    fn shadowed_global_still_mangled() {
        let expr = r#"
        (define (foo x y z) (let ((foo 10) (b 20)) (foo (+ bar y z a b))))
        (define (bar applesauce) (+ applesauce 10))
     "#;

        let mut parsed = Parser::parse(expr).unwrap();

        mangle_vars_with_prefix("--test--".to_string(), &mut parsed);

        let expected = Parser::parse(
            r#"
            (define (--test--foo x y z) (let ((--test--foo 10) (b 20)) (--test--foo (+ --test--bar y z a b))))
            (define (--test--bar applesauce) (+ applesauce 10))
     "#,
        )
        .unwrap();

        assert_eq!(parsed, expected);
    }

    #[test]
    fn still_collect_defines_in_begins() {
        let expr = r#"
        (begin 
            (begin 
                (begin 
                    (begin 
                        (define x 10)
                    ) 
                    (define y 20)
                )
            )
        )
        "#;

        let mut parsed = Parser::parse(expr).unwrap();

        mangle_vars_with_prefix("--test--".to_string(), &mut parsed);

        let expected = Parser::parse(
            r#"
        (begin 
            (begin 
                (begin 
                    (begin 
                        (define --test--x 10)
                    ) 
                    (define --test--y 20)
                )
            )
        )
        "#,
        )
        .unwrap();

        assert_eq!(parsed, expected);
    }
}
