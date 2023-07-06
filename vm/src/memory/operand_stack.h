#pragma once

#include "constant_pool.h"
#include "errors.h"
#include <cstddef>
#include <cstdint>
#include <exception>
#include <memory>
#include <stdexcept>

/**
 * thrown when the operand stack does not have enough data to pop requested value
 */
struct OperandStackUnderflowError : public MemoryError {

public:
    explicit OperandStackUnderflowError(std::string message) : MemoryError{std::move(message)} {}
};

class OperandStack {
private:
    const char *bytes;
    size_t &current_pos;
    const size_t stack_capacity;

public:
    explicit OperandStack(char *buff, size_t &initial_pos, size_t stack_capacity);

    /**
     * @return the size in bytes of the operand stack
     */
    size_t size() const;

    /**
     * @return the capacity in bytes of the operand stack
     */
    size_t get_capacity() const;

    /**
     * @throws StackOverflowError if the operand stack would overflow by pushing the quad-word
     */
    void push_int(int64_t i);

    /**
     * @throws StackOverflowError if the operand stack would overflow by pushing the byte
     */
    void push_byte(int8_t b);

    /**
     * @throws StackOverflowError if the operand stack would overflow by pushing the quad-word
     */
    void push_double(double d);

    /**
     * @throws StackOverflowError if the operand stack would overflow by pushing the reference
     */
    void push_reference(uint64_t r);

    /**
     * @return popped quad-word as an integer
     * @throws OperandStackUnderflowError if the operand stack does not have enough bytes to pop a quad-word
     */
    int64_t pop_int();

    /**
     * @return popped byte
     * @throws OperandStackUnderflowError if the operand stack does not have enough bytes to pop a byte
     */
    int8_t pop_byte();

    /**
     * @return popped quad-word as a double
     * @throws OperandStackUnderflowError if the operand stack does not have enough bytes to pop a quad-word
     */
    double pop_double();

    /**
     * @return popped reference
     * @throws OperandStackUnderflowError if the operand stack does not have enough bytes to pop a reference
     */
    uint64_t pop_reference();

    /**
     * pops `n` bytes
     * @throws OperandStackUnderflowError if the operand stack does not have enough bytes to pop
     */
    const char *pop_bytes(size_t n);

    /**
     * advances, without checking for stack overflow, the position of the operand stack.
     * The call of this method must be JUSTIFIED
     */
    void advance_unchecked(size_t size);

private:
    template <typename T>
    void push(T t);

    template <typename T>
    T pop();
};
