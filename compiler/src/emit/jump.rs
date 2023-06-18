use analyzer::types::hir::{Conditional, Loop};
use analyzer::types::INT;
use std::mem::size_of;

use crate::bytecode::{Bytecode, Opcode};
use crate::constant_pool::ConstantPool;
use crate::emit::{emit, EmissionState};

pub fn emit_conditional(
    conditional: &Conditional,
    emitter: &mut Bytecode,
    cp: &mut ConstantPool,
    state: &mut EmissionState,
) {
    emit(&conditional.condition, emitter, cp, state);

    if conditional.condition.ty == INT {
        emitter.emit_code(Opcode::ConvertIntToByte);
        emitter.emit_byte(1);
        emitter.emit_code(Opcode::BXor);
    }

    // If the condition is false, go to ELSE.
    let jump_to_else = emitter.emit_jump(Opcode::IfNotJump);
    // Evaluate the if branch.
    emit(&conditional.then, emitter, cp, state);

    // Go to END.
    let jump_to_end = emitter.emit_jump(Opcode::Jump);

    // ELSE:
    emitter.patch_jump(jump_to_else);
    if let Some(otherwise) = &conditional.otherwise {
        emit(otherwise, emitter, cp, state);
    }

    // END:
    emitter.patch_jump(jump_to_end);
}

pub fn emit_loop(
    lp: &Loop,
    emitter: &mut Bytecode,
    cp: &mut ConstantPool,
    state: &mut EmissionState,
) {
    let start_ip = emitter.len();

    let mut loop_state = EmissionState::new();
    loop_state.enclosing_loop_start = start_ip;

    if let Some(condition) = &lp.condition {
        emit(condition, emitter, cp, state);

        if condition.ty == INT {
            emitter.emit_code(Opcode::ConvertIntToByte);
            emitter.emit_byte(1);
            emitter.emit_code(Opcode::BXor);
        }

        emitter.emit_code(Opcode::IfNotJump);
        let jump_placeholder = emitter.create_placeholder(size_of::<usize>());
        emit(&lp.body, emitter, cp, &mut loop_state);

        // jump back to loop start
        emitter.emit_code(Opcode::Jump);
        emitter.emit_instruction_pointer(start_ip);

        // if condition is false, jump at end of loop
        emitter.fill_in_ip(jump_placeholder, emitter.len());
    } else {
        emit(&lp.body, emitter, cp, &mut loop_state);
        emitter.emit_code(Opcode::Jump);
        emitter.emit_instruction_pointer(start_ip)
    }

    // fill break placeholders
    let current_ip = emitter.len();
    for placeholder in loop_state.enclosing_loop_end_placeholders {
        emitter.fill_in_ip(placeholder, current_ip)
    }
}

pub fn emit_continue(emitter: &mut Bytecode, state: &mut EmissionState) {
    emitter.emit_code(Opcode::Jump);
    emitter.emit_instruction_pointer(state.enclosing_loop_start);
}

pub fn emit_break(emitter: &mut Bytecode, state: &mut EmissionState) {
    emitter.emit_code(Opcode::Jump);
    state
        .enclosing_loop_end_placeholders
        .push(emitter.create_placeholder(size_of::<usize>()));
}
