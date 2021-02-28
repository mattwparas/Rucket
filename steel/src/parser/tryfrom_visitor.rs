use crate::parser::ast::ExprKind;
use crate::parser::visitors::ConsumingVisitorRef;

use crate::rerrs::{ErrorKind, SteelErr};
use crate::rvals::{Result, SteelVal};

use super::ast::Atom;

use std::convert::TryFrom;

use crate::primitives::ListOperations;

pub struct TryFromExprKindForSteelVal {}

impl TryFromExprKindForSteelVal {
    pub fn try_from_expr_kind(e: ExprKind) -> Result<SteelVal> {
        TryFromExprKindForSteelVal {}.visit(e)
    }
}

impl ConsumingVisitorRef for TryFromExprKindForSteelVal {
    type Output = Result<SteelVal>;

    fn visit_if(&self, f: Box<super::ast::If>) -> Self::Output {
        let expr = [
            SteelVal::try_from(f.location)?,
            self.visit(f.test_expr)?,
            self.visit(f.then_expr)?,
            self.visit(f.else_expr)?,
        ];
        ListOperations::built_in_list_func_flat(&expr)
    }

    fn visit_define(&self, define: Box<super::ast::Define>) -> Self::Output {
        let expr = [
            SteelVal::try_from(define.location)?,
            self.visit(define.name)?,
            self.visit(define.body)?,
        ];
        ListOperations::built_in_list_func_flat(&expr)
    }

    fn visit_lambda_function(
        &self,
        lambda_function: Box<super::ast::LambdaFunction>,
    ) -> Self::Output {
        let args = lambda_function
            .args
            .into_iter()
            .map(|x| self.visit(x))
            .collect::<Result<Vec<_>>>()?;

        let expr = [
            SteelVal::try_from(lambda_function.location)?,
            ListOperations::built_in_list_func_flat(&args)?,
            self.visit(lambda_function.body)?,
        ];

        ListOperations::built_in_list_func_flat(&expr)
    }

    fn visit_begin(&self, begin: super::ast::Begin) -> Self::Output {
        let mut exprs = vec![SteelVal::try_from(begin.location)?];
        for expr in begin.exprs {
            exprs.push(self.visit(expr)?);
        }
        ListOperations::built_in_list_func_flat(&exprs)
    }

    fn visit_return(&self, r: Box<super::ast::Return>) -> Self::Output {
        let expr = [SteelVal::try_from(r.location)?, self.visit(r.expr)?];
        ListOperations::built_in_list_func_flat(&expr)
    }

    fn visit_apply(&self, apply: Box<super::ast::Apply>) -> Self::Output {
        let expr = [
            SteelVal::try_from(apply.location)?,
            self.visit(apply.func)?,
            self.visit(apply.list)?,
        ];
        ListOperations::built_in_list_func_flat(&expr)
    }

    fn visit_panic(&self, p: Box<super::ast::Panic>) -> Self::Output {
        let expr = [SteelVal::try_from(p.location)?, self.visit(p.message)?];
        ListOperations::built_in_list_func_flat(&expr)
    }

    fn visit_transduce(&self, transduce: Box<super::ast::Transduce>) -> Self::Output {
        let expr = [
            SteelVal::try_from(transduce.location)?,
            self.visit(transduce.transducer)?,
            self.visit(transduce.func)?,
            self.visit(transduce.initial_value)?,
            self.visit(transduce.iterable)?,
        ];
        ListOperations::built_in_list_func_flat(&expr)
    }

    fn visit_read(&self, read: Box<super::ast::Read>) -> Self::Output {
        let expr = [SteelVal::try_from(read.location)?, self.visit(read.expr)?];
        ListOperations::built_in_list_func_flat(&expr)
    }

    fn visit_execute(&self, execute: Box<super::ast::Execute>) -> Self::Output {
        let mut exprs = vec![
            SteelVal::try_from(execute.location)?,
            self.visit(execute.transducer)?,
            self.visit(execute.collection)?,
        ];

        if let Some(output) = execute.output_type {
            exprs.push(self.visit(output)?);
        }
        ListOperations::built_in_list_func_flat(&exprs)
    }

    fn visit_quote(&self, quote: Box<super::ast::Quote>) -> Self::Output {
        self.visit(quote.expr)
    }

    fn visit_struct(&self, s: Box<super::ast::Struct>) -> Self::Output {
        let fields = s
            .fields
            .into_iter()
            .map(|x| self.visit(x))
            .collect::<Result<Vec<_>>>()?;

        let expr = vec![
            SteelVal::try_from(s.location)?,
            self.visit(s.name)?,
            ListOperations::built_in_list_func_flat(&fields)?,
        ];

        ListOperations::built_in_list_func_flat(&expr)
    }

    fn visit_macro(&self, _m: super::ast::Macro) -> Self::Output {
        // TODO
        stop!(Generic => "internal compiler error - could not translate macro to steel value")
    }

    fn visit_eval(&self, e: Box<super::ast::Eval>) -> Self::Output {
        let expr = [SteelVal::try_from(e.location)?, self.visit(e.expr)?];
        ListOperations::built_in_list_func_flat(&expr)
    }

    fn visit_atom(&self, a: Atom) -> Self::Output {
        SteelVal::try_from(a.syn)
    }

    fn visit_list(&self, l: super::ast::List) -> Self::Output {
        let items: std::result::Result<Vec<_>, SteelErr> =
            l.args.into_iter().map(|x| self.visit(x)).collect();

        ListOperations::built_in_list_func_flat(&items?)
    }

    fn visit_syntax_rules(&self, _l: super::ast::SyntaxRules) -> Self::Output {
        // TODO
        stop!(Generic => "internal compiler error - could not translate syntax-rules to steel value")
    }

    fn visit_set(&self, s: Box<super::ast::Set>) -> Self::Output {
        let expr = [SteelVal::try_from(s.location)?, self.visit(s.expr)?];
        ListOperations::built_in_list_func_flat(&expr)
    }

    fn visit_require(&self, _s: super::ast::Require) -> Self::Output {
        stop!(Generic => "internal compiler error - could not translate require to steel value")
    }
}
