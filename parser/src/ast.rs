use crate::ast::callable::{Call, FunDeclaration, Pipeline, Redirected};
use crate::ast::group::{Block, Parenthesis, Subshell};
use crate::ast::value::{Literal, TemplateString};
use crate::ast::operation::BinaryOperation;
use crate::ast::r#use::Use;
use crate::ast::substitution::Substitution;
use crate::ast::test::{Not, Test};
use crate::ast::variable::{Assign, VarDeclaration, VarReference};
use crate::ast::control_flow::If;
use crate::ast::r#match::Match;

pub mod callable;
pub mod group;
pub mod value;
pub mod operation;
pub mod substitution;
pub mod variable;
pub mod test;
pub mod r#use;
pub mod control_flow;
pub mod r#match;

/// A expression that can be evaluated.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr<'a> {
    Assign(Assign<'a>),
    Binary(BinaryOperation<'a>),
    FunDeclaration(FunDeclaration<'a>),
    Literal(Literal<'a>),

    Match(Match<'a>),

    Call(Call<'a>),
    Pipeline(Pipeline<'a>),
    Redirected(Redirected<'a>),

    Substitution(Substitution<'a>),
    TemplateString(TemplateString<'a>),

    Use(Use<'a>),

    Test(Test<'a>),
    Not(Not<'a>),

    If(If<'a>),

    //var / val handling expressions
    VarReference(VarReference<'a>),
    VarDeclaration(VarDeclaration<'a>),

    //Grouping expressions
    /// a parenthesis expression `( ... )` that contains one value expression
    Parenthesis(Parenthesis<'a>),
    /// a subshell expression `( ... )` that contains several expressions
    Subshell(Subshell<'a>),
    /// a block expression `{ ... }` that contains several expressions
    Block(Block<'a>),
}
