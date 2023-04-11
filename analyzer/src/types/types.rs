use context::display::fmt_comma_separated;
use std::fmt::{Debug, Display, Formatter};

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ParameterizedType {
    pub name: String,
    pub params: Vec<Type>,
}

impl ParameterizedType {
    pub fn cons(name: &str) -> Self {
        Self::parametrized(name, &[])
    }

    pub fn parametrized(name: &str, params: &[Type]) -> Self {
        Self {
            name: name.to_owned(),
            params: params.to_vec(),
        }
    }
}

impl Display for ParameterizedType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)?;
        if !self.params.is_empty() {
            fmt_comma_separated('[', ']', &self.params, f)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum DefinedType {
    /// parametrized or constant type (`List[A]`, `Map[Str, List[B]]`, `Int`).
    Parameterized(ParameterizedType),
    //Callable ?
}

impl DefinedType {
    pub fn cons(name: &str) -> Self {
        Self::parametrized(name, &[])
    }

    pub fn parametrized(name: &str, params: &[Type]) -> Self {
        DefinedType::Parameterized(ParameterizedType::parametrized(name, params))
    }
}

/// Represents [monotypes][1] (fully instantiated, unquantified types).
///
/// [1]: https://en.wikipedia.org/wiki/Hindley–Milner_type_system#Monotypes
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Type {
    ///A Defined, structured types
    Defined(DefinedType),

    ///Special handling for nothing types
    Nothing,

    ///The type isn't known yet
    Unknown,
}

impl Type {
    pub fn cons(name: &str) -> Self {
        Self::parametrized(name, &[])
    }

    pub fn parametrized(name: &str, params: &[Type]) -> Self {
        Type::Defined(DefinedType::parametrized(name, params))
    }
}

impl Display for Type {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Defined(d) => write!(f, "{d}"),
            Type::Unknown => write!(f, "<unknown>"),
            Type::Nothing => write!(f, "Nothing"),
        }
    }
}

impl Display for DefinedType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DefinedType::Parameterized(p) => write!(f, "{p}"),
        }
    }
}