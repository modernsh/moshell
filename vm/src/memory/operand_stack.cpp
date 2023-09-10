#include "operand_stack.h"
#include <cstring>

OperandStack::OperandStack(char *const buff, size_t &position, size_t stack_capacity)
    : bytes{buff},
      current_pos{position},
      stack_capacity{stack_capacity} {}

size_t OperandStack::size() const {
    return current_pos;
}

size_t OperandStack::get_capacity() const {
    return stack_capacity;
}

void OperandStack::push_int(int64_t i) {
    push(i);
}

void OperandStack::push_reference(msh::obj &r) {
    push_unchecked_reference(&r);
}

void OperandStack::push_unchecked_reference(void *r) {
    push((uint64_t)r);
}

void OperandStack::push_byte(int8_t b) {
    push(b);
}

void OperandStack::push_double(double d) {
    push(d);
}

int64_t OperandStack::pop_int() {
    return pop<int64_t>();
}

int8_t OperandStack::pop_byte() {
    return pop<int8_t>();
}

msh::obj &OperandStack::pop_reference() {
    return *(msh::obj *)(pop<uint64_t>());
}

double OperandStack::pop_double() {
    return pop<double>();
}

const char *OperandStack::pop_bytes(size_t n) {
    if (current_pos < n) {
        throw OperandStackUnderflowError("operand stack is empty");
    }
    current_pos -= n;
    return bytes + current_pos;
}

void OperandStack::push(const char *bytes, size_t size) {
    if (current_pos + size > stack_capacity) {
        throw StackOverflowError("exceeded stack capacity via operand stack");
    }
    memcpy(static_cast<void *>(this->bytes + current_pos), bytes, size);
    current_pos += size;
}
