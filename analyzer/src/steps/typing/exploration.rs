use crate::engine::Engine;
use crate::reef::{ReefId, Reefs};
use crate::relations::Relations;
use crate::steps::typing::function::Return;
use crate::types::ctx::TypeContext;
use crate::types::engine::TypedEngine;
use crate::types::Typing;

/// The support for type analysis.
pub(super) struct Exploration {
    pub(super) type_engine: TypedEngine,
    pub(super) typing: Typing,
    pub(super) ctx: TypeContext,
    pub(super) returns: Vec<Return>,
}

#[derive(Clone, Copy)]
pub struct ReefTypes<'a> {
    pub context: &'a TypeContext,
    pub engine: &'a TypedEngine,
    pub typing: &'a Typing,
}

/// A proxy that let the user access to any reef's engine, relations and types data.
/// If the given reef identifier is the `current_reef`, the types data stored in `current` is returned instead
/// of the data stored by the `reefs`.
#[derive(Clone, Copy)]
pub struct UniversalReefAccessor<'a, 'e> {
    reefs: &'a Reefs<'e>,
    current_reef: ReefId,
    current: ReefTypes<'e>,
}

impl<'a, 'e> UniversalReefAccessor<'a, 'e> {
    pub fn get_types(&self, id: ReefId) -> Option<ReefTypes<'a>> {
        if id == self.current_reef {
            Some(self.current)
        } else {
            self.reefs.get_reef(id).map(|reef| ReefTypes {
                context: &reef.type_context,
                engine: &reef.typed_engine,
                typing: &reef.typing,
            })
        }
    }

    pub fn get_engine(&self, id: ReefId) -> Option<&'a Engine<'e>> {
        self.reefs.get_reef(id).map(|r| &r.engine)
    }

    pub fn get_relations(&self, id: ReefId) -> Option<&'a Relations> {
        self.reefs.get_reef(id).map(|r| &r.relations)
    }
}

impl Exploration {
    pub(super) fn prepare(&mut self) {
        self.returns.clear();
    }

    /// returns an universal accessor that will return the exploration's types data
    /// if the accessed reef's identifier is equal to given `current_reef`
    pub(crate) fn universal_accessor<'a, 'e>(
        &'e self,
        current_reef: ReefId,
        reefs: &'a Reefs<'e>,
    ) -> UniversalReefAccessor<'a, 'e> {
        UniversalReefAccessor {
            reefs,
            current_reef,
            current: ReefTypes {
                context: &self.ctx,
                engine: &self.type_engine,
                typing: &self.typing,
            },
        }
    }
}
