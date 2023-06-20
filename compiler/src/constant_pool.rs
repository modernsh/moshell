use indexmap::IndexSet;

use crate::r#type::transform_to_vm_type;
use analyzer::types::hir::TypeId;
use analyzer::types::Typing;

/// Contains the constants defined in constant pool
#[derive(Default)]
pub struct ConstantPool {
    pub constants: IndexSet<PoolConstant>,
}

#[derive(Hash, Eq, PartialEq)]
pub enum PoolConstant {
    String(String),
    Signature(FunctionSignature),
}

impl ConstantPool {
    /// inserts if not already contained a string in the constant pool, returning it's pool identifier
    pub fn insert_string(&mut self, str: &str) -> u32 {
        self.insert(PoolConstant::String(str.to_string()))
    }

    /// inserts if not already contained a function signature in the constant pool, returning it's pool identifier
    pub fn insert_signature(&mut self, signature: FunctionSignature) -> u32 {
        self.insert(PoolConstant::Signature(signature))
    }

    fn insert(&mut self, constant: PoolConstant) -> u32 {
        self.constants.insert_full(constant).0 as u32
    }
}

#[derive(Hash, Eq, PartialEq)]
pub struct FunctionSignature {
    pub name: u32,
    pub params: Vec<u32>,
    pub ret: u32,
}

impl FunctionSignature {
    pub fn make(
        name: &str,
        params: impl Iterator<Item = TypeId>,
        return_type: TypeId,
        typing: &Typing,
        cp: &mut ConstantPool,
    ) -> Self {
        let name = cp.insert_string(name);

        let mut map_type = |ty| {
            let ty = typing.get_type(ty).unwrap();
            transform_to_vm_type(ty, cp)
        };

        Self {
            name,
            params: params.map(&mut map_type).collect(),
            ret: map_type(return_type),
        }
    }
}
