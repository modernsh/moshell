
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use crate::classes::ClassType;
use crate::types::{DefinedType, Type};
use crate::lang_types::*;

/// A type environment.
///
/// Contexts track substitutions and generate fresh type variables.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TypeContext<'a> {
    /// Records the type of each class by name.
    classes: HashMap<DefinedType, Rc<ClassType>>,

    dependencies: Vec<&'a TypeContext<'a>>,
}


//as current structures does not handle random accesses, we cannot share type contexts between threads
//then, we create
thread_local! {
    pub static LANG: TypeContext<'static> = TypeContext::lang();
}

impl<'a> TypeContext<'a> {
    ///Definitions of the lang type context.
    pub fn lang() -> Self {
        let mut ctx = TypeContext::default();

        const MSG: &str = "lang type registration";

        ctx.define_root(float()).expect(MSG);
        ctx.define_root(bool()).expect(MSG);
        ctx.define_root(str()).expect(MSG);
        ctx.define_root(unit()).expect(MSG);

        ctx.define_specialized(&float(), int()).expect(MSG);
        ctx.define_specialized(&int(), exitcode()).expect(MSG);

        ctx
    }

    /// Creates and registers a new ClassType for given type, the given type must be subtype of given type
    pub fn define_specialized(&mut self, super_type: &DefinedType, registered: DefinedType) -> Result<(), String> {
        if self.classes.contains_key(&registered) {
            return Err(format!("type already contained in context {}", registered).to_owned())
        }

        let sup = self.lookup_definition(super_type)?;

        self.classes.insert(
            registered.clone(),
            Rc::new(ClassType {
                base: registered,
                super_type: Some(sup.clone()),
                identity: 0,
            }),
        );
        Ok(())
    }


    /// Creates and registers a new ClassType for given type, the given type must be subtype of given type
    fn define_root(&mut self, root: DefinedType) -> Result<(), String> {
        if self.classes.contains_key(&root) {
            return Err(format!("type already contained in context {}", root).to_owned())
        }

        self.classes.insert(
            root.clone(),
            Rc::new(ClassType {
                base: root,
                super_type: None,
                identity: 0,
            }),
        );
        Ok(())
    }

    ///perform a class type lookup based on the defined type.
    /// If the type is not directly found in this context, then the context
    /// will lookup in parent's context.
    pub fn lookup_definition(&self, tpe: &DefinedType) -> Result<Rc<ClassType>, String> {
        match self.classes.get(&tpe) {
            Some(v) => Ok(v.clone()),
            None => {
                let iter = self.dependencies.iter();
                for dep in iter {
                    if let Some(found) = dep.lookup_definition(tpe).ok() {
                        return Ok(found)
                    }
                }
                Err("Unknown type".to_owned())
            }
        }
    }
    /*
        pub fn resolve(&self, declared_type: &TypeScheme) -> Result<Variable, String> {
            match declared_type {
                TypeScheme::Monotype(t) => self.resolve_monotype(t),
                TypeScheme::Polytype { .. } => todo!("resolve polytype"),
            }
        }
        */
    /*
        pub fn resolve_monotype(&self, declared_type: &Type) -> Result<Variable, String> {
            match declared_type {
                Type::Variable(v) => self
                    .substitution
                    .get(v)
                    .map(|t| self.resolve(t))
                    .unwrap_or(Ok(*v)),
                Type::Defined(name, args) => {
                    let var = self
                        .classes
                        .get(name)
                        .ok_or_else(|| format!("Unknown class {}", name))?;
                    let class = self
                        .definitions
                        .get(var)
                        .ok_or_else(|| format!("Unknown class {}", name))?;
                    assert_eq!(class.type_args.len(), args.len());
                    assert_eq!(class.type_args.len(), 0);
                    Ok(*var)
                }
            }
        }
    */
    pub fn unify(&self, t1: &Type, t2: &Type) -> Result<Type, String> {
        self.unify_internal(t1, t2)
    }

    pub(crate) fn fork(&self) -> TypeContext {
        TypeContext {
            dependencies: vec!(self),
            ..Default::default()
        }
    }

    ///Find largest possible type between two class types
    fn unify_internal(&self, t1: &Type, t2: &Type) -> Result<Type, String> {
        match (t1, t2) {
            (any, Type::Nothing) => Ok(any.clone()),
            (Type::Nothing, any) => Ok(any.clone()),

            (Type::Unknown, _) => Ok(Type::Unknown),
            (_, Type::Unknown) => Ok(Type::Unknown),

            (Type::Defined(def1 @ DefinedType::Parameterized(_)),
                Type::Defined(def2 @ DefinedType::Parameterized(_))) => {
                let cl1 = self.lookup_definition(def1)?;

                cl1.unify_base(self, def2)
                    .and_then(|opt|
                        opt.map(Type::Defined)
                            .ok_or("Type 1 and Type 2 are not inferable".to_owned())
                    )
            }

            (Type::Defined(DefinedType::Callable(_)), _) => {
                Err("Cannot handle callables yet".to_owned())
            }
            (_, Type::Defined(DefinedType::Callable(_))) => {
                Err("Cannot handle callables yet".to_owned())
            }
            (_, _) => Err(format!("Incompatible types {:?} and {:?}", t1, t2))
        }
    }

}