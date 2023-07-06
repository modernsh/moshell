use crate::locals::LocalsLayout;
use crate::r#type::ValueStackSize;
use analyzer::relations::LocalId;
use std::mem::size_of;

#[derive(Debug, Clone)]
pub struct Placeholder {
    pos: u32,
}

/// Holds the currently generated bytecode.
///
/// This struct provides support methods to emit bytecode primitives
/// such as ints, byte, double, constant references etc.
/// Also provides placeholder support for backpatching
#[derive(Default)]
pub struct Bytecode {
    bytes: Vec<u8>,
}

impl Bytecode {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns the number of bytes
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// emits a signed 64 bits integer
    pub fn emit_int(&mut self, value: i64) {
        self.bytes.extend(value.to_be_bytes());
    }

    /// emits an unsigned byte
    pub fn emit_byte(&mut self, value: u8) {
        self.bytes.push(value);
    }

    /// emits an unsigned 32 bits integer
    pub fn emit_u32(&mut self, value: u32) {
        self.bytes.extend(value.to_be_bytes());
    }

    /// emits a signed 64 bits float
    pub fn emit_float(&mut self, value: f64) {
        self.bytes.extend(value.to_be_bytes());
    }

    /// emits a constant pool reference, which is an unsigned 32 bits integer
    pub fn emit_constant_ref(&mut self, constant: u32) {
        self.emit_u32(constant);
    }

    /// Fills an instruction pointer at given instruction pointer in the byte array
    pub fn patch_u32_placeholder(&mut self, placeholder: Placeholder, value: u32) {
        let pos = placeholder.pos as usize;
        self.bytes[pos..pos + size_of::<u32>()].copy_from_slice(&value.to_be_bytes())
    }

    pub fn emit_instruction_pointer(&mut self, ip: usize) {
        self.bytes.extend(ip.to_be_bytes());
    }

    /// expands the byte vector to let a placeholder of the given size,
    /// returning the position of the placeholder in the vector
    pub fn emit_u32_placeholder(&mut self) -> Placeholder {
        let pos = self.bytes.len();
        self.bytes.resize(pos + size_of::<u32>(), 0);
        Placeholder { pos: pos as u32 }
    }
}

/// This structure is a [Bytecode] wrapper and is used
/// to emit bytecode instructions.
pub struct Instructions<'a> {
    pub bytecode: &'a mut Bytecode,
    // offset of where this instruction set_bytes starts in the given bytecode
    pub ip_offset: usize,
}

impl<'a> Instructions<'a> {
    pub fn wrap(bytecode: &'a mut Bytecode) -> Self {
        Self {
            ip_offset: bytecode.len(),
            bytecode,
        }
    }

    pub fn emit_code(&mut self, code: Opcode) {
        self.bytecode.emit_byte(code as u8)
    }

    pub fn emit_pop(&mut self, size: ValueStackSize) {
        let pop_opcode = match size {
            ValueStackSize::Zero => panic!("Cannot pop zero sized types"),
            ValueStackSize::Byte => Opcode::PopByte,
            ValueStackSize::QWord => Opcode::PopQWord,
            ValueStackSize::Reference => Opcode::PopRef,
        };
        self.emit_code(pop_opcode);
    }

    /// emits instructions to assign given local identifier with last operand stack value
    /// assuming the local's size is the given `size` argument
    pub fn emit_set_local(
        &mut self,
        identifier: LocalId,
        size: ValueStackSize,
        layout: &LocalsLayout,
    ) {
        let opcode = match size {
            ValueStackSize::Byte => Opcode::SetByte,
            ValueStackSize::QWord => Opcode::SetQWord,
            ValueStackSize::Reference => Opcode::SetRef,
            ValueStackSize::Zero => panic!("set_local for value whose type is zero-sized"),
        };
        self.emit_code(opcode);
        let at = layout.get_index(identifier);
        self.bytecode.emit_u32(at);
    }

    /// emits instructions to push to operand stack given local identifier
    /// assuming the local's size is the given `size` argument
    pub fn emit_get_local(
        &mut self,
        identifier: LocalId,
        size: ValueStackSize,
        layout: &LocalsLayout,
    ) {
        let opcode = match size {
            ValueStackSize::Byte => Opcode::GetByte,
            ValueStackSize::QWord => Opcode::GetQWord,
            ValueStackSize::Reference => Opcode::GetRef,
            ValueStackSize::Zero => panic!("get_local for value whose type is zero-sized"),
        };

        self.emit_code(opcode);
        let index = layout.get_index(identifier);
        self.bytecode.emit_u32(index);
    }

    /// emits instructions to push an integer in the operand stack
    pub fn emit_push_int(&mut self, constant: i64) {
        self.emit_code(Opcode::PushInt);
        self.bytecode.emit_int(constant);
    }

    /// emits instructions to push an unsigned byte in the operand stack
    pub fn emit_push_byte(&mut self, constant: u8) {
        self.emit_code(Opcode::PushByte);
        self.bytecode.emit_byte(constant);
    }

    /// emits instructions to push a float in the operand stack
    pub fn emit_push_float(&mut self, constant: f64) {
        self.emit_code(Opcode::PushFloat);
        self.bytecode.emit_float(constant)
    }

    /// emits instructions to push a pool reference in the operand stack
    pub fn emit_push_constant_ref(&mut self, constant_ref: u32) {
        self.emit_code(Opcode::PushString);
        self.bytecode.emit_constant_ref(constant_ref)
    }

    /// Inverts the boolean value on top of the stack.
    pub fn emit_bool_inversion(&mut self) {
        self.emit_push_byte(1);
        self.emit_code(Opcode::BXor);
    }

    /// emits an instruction pointer
    fn emit_instruction_pointer(&mut self, ip: u32) {
        self.bytecode.emit_u32(ip);
    }

    /// Emits a jump instruction.
    ///
    /// It returns the Placeholder of the offset which is to be patched
    #[must_use = "the jump address must be patched later"]
    pub fn emit_jump(&mut self, opcode: Opcode) -> Placeholder {
        self.emit_code(opcode);
        self.bytecode.emit_u32_placeholder()
    }

    /// emits a spawn instruction, with given argument counts
    pub fn emit_spawn(&mut self, arg_count: u8) {
        self.emit_code(Opcode::Spawn);
        self.bytecode.emit_byte(arg_count);
    }

    /// emits a function invocation instruction, with given method signature in constant pool
    pub fn emit_invoke(&mut self, signature_idx: u32) {
        self.emit_code(Opcode::Invoke);
        self.bytecode.emit_constant_ref(signature_idx);
    }

    /// Takes the index of the jump offset to be patched as input, and patches
    /// it to point to the current instruction.
    pub fn patch_jump(&mut self, offset_idx: Placeholder) {
        let ip = self.current_ip();
        self.bytecode.patch_u32_placeholder(offset_idx, ip);
    }

    /// Emits a jump instruction to the given instruction pointer.
    pub fn jump_back_to(&mut self, start_idx: u32) {
        self.emit_code(Opcode::Jump);
        self.emit_instruction_pointer(start_idx);
    }

    /// Returns the current instruction pointer
    pub fn current_ip(&self) -> u32 {
        (self.bytecode.len() - self.ip_offset) as u32
    }
}

/// see vm's `Opcode` enum for more details
#[repr(u8)]
#[derive(Eq, PartialEq)]
pub enum Opcode {
    PushInt,
    PushByte,
    PushFloat,
    PushString,

    GetByte,
    SetByte,
    GetQWord,
    SetQWord,
    GetRef,
    SetRef,

    Spawn,
    Invoke,

    PopByte,
    PopQWord,
    PopRef,

    IfJump,
    IfNotJump,
    Jump,

    Return,

    ConvertByteToInt,
    ConvertIntToStr,
    ConvertFloatToStr,
    ConvertIntToByte,
    Concat,

    BXor,
    IntAdd,
    IntSub,
    IntMul,
    IntDiv,
    IntMod,
    FloatAdd,
    FloatSub,
    FloatMul,
    FloatDiv,

    StringEqual,
    IntEqual,
    IntLessThan,
    IntLessOrEqual,
    IntGreaterThan,
    IntGreaterOrEqual,
    FloatEqual,
    FloatLessThan,
    FloatLessOrEqual,
    FloatGreaterThan,
    FloatGreaterOrEqual,
}
