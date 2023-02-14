use crate::ast::variable::TypedVariable;
use crate::ast::Expr;
use lexer::token::Token;

/// A call to a function or a command.
#[derive(Debug, Clone, PartialEq)]
pub struct Call<'a> {
    /// The arguments of the command.
    pub arguments: Vec<Expr<'a>>,
}

/// A function declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct FunDeclaration<'a> {
    pub name: Token<'a>,
    pub parameters: Vec<TypedVariable<'a>>,
    pub body: Vec<Expr<'a>>,
}