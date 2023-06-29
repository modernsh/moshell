#pragma once

#include <cstddef>
#include <cstdint>
#include <vector>

/**
 * Contains the information needed for the execution of a function
 */
struct function_definition {
    /**
     * Amount, in bytes of the space in the stack frame allocated for local values area
     */
    size_t locals_size;
    /**
     * Length, in bytes of the heading space in locals area where the arguments of this
     * function call are to be moved.
     * parameters byte count cannot be superior to allocated space for locals, as it is part of this area
     */
    size_t parameters_byte_count;
    /**
     * Length, in bytes of the heading space in locals area where the return value of
     * this function call are contained once the frame returns.
     * return byte count cannot be superior to allocated space for locals, as it is part of this area
     */
    uint8_t return_byte_count;
    /**
     * Number of instruction in bytes
     */
    size_t instruction_count;
    /**
     * pointer to the first instruction to execute.
     */
    const char *instructions;
};
