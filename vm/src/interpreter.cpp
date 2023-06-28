#include "interpreter.h"
#include "conversions.h"
#include "memory/constant_pool.h"
#include "memory/operand_stack.h"
#include "vm.h"

#include <cstdlib>
#include <cstring>
#include <iostream>
#include <memory>
#include <sys/wait.h>
#include <unistd.h>
#include <vector>

enum Opcode {
    OP_PUSH_INT,    // with 8 byte int value, pushes an int onto the operand stack
    OP_PUSH_BYTE,   // with 1 byte value, pushes a byte onto the operand stack
    OP_PUSH_FLOAT,  // with 8 byte float value, pushes a float onto the operand stack
    OP_PUSH_STRING, // with 8 byte string index in constant pool, pushes a string ref onto the operand stack

    OP_GET_BYTE,   // with 1 byte local index, pushes given local value onto the operand stack
    OP_SET_BYTE,   // with 1 byte local index, set given local value from value popped from the operand stack
    OP_GET_Q_WORD, // with 1 byte local index, pushes given local value onto the operand stack
    OP_SET_Q_WORD, // with 1 byte local index, set given local value from value popped from the operand stack
    OP_GET_REF,    // with 1 byte local index, pushes given local value onto the operand stack
    OP_SET_REF,    // with 1 byte local index, set given local value from value popped from the operand stack

    OP_SPAWN, // with 1 byte stack size for process exec(), pushes process exit status onto the operand stack

    OP_POP_BYTE,   // pops one byte from operand stack
    OP_POP_Q_WORD, // pops 8 bytes from operand stack

    OP_IF_JUMP,     // with 1 byte opcode for 'then' branch, jumps only if value popped from operand stack is 0
    OP_IF_NOT_JUMP, // with 1 byte opcode for where to jump, jumps only if value popped from operand stack is not 0
    OP_JUMP,        // with 1 byte opcode for where to jump

    OP_BYTE_TO_INT,  // replaces last value of operand stack from byte to int
    OP_INT_TO_STR,   // replaces last value of operand stack from int to a string reference
    OP_FLOAT_TO_STR, // replaces last value of operand stack from float to a string reference
    OP_INT_TO_BYTE,  // replaces last value of operand stack from int to byte
    OP_CONCAT,       // pops two string references, concatenates them, and pushes the result

    OP_B_XOR,     // pops last two bytes, apply xor operation then push the result
    OP_INT_ADD,   // takes two ints, adds them, and pushes the result
    OP_INT_SUB,   // takes two ints, subtracts them, and pushes the result
    OP_INT_MUL,   // takes two ints, multiplies them, and pushes the result
    OP_INT_DIV,   // takes two ints, divides them, and pushes the result
    OP_INT_MOD,   // takes two ints, mods them, and pushes the result
    OP_FLOAT_ADD, // takes two floats, adds them, and pushes the result
    OP_FLOAT_SUB, // takes two floats, subtracts them, and pushes the result
    OP_FLOAT_MUL, // takes two floats, multiplies them, and pushes the result
    OP_FLOAT_DIV, // takes two floats, divides them, and pushes the result

    OP_STR_EQ, // takes two string references, checks if they are equal, and pushes the result

    OP_INT_EQ, // takes two ints, checks if they are equal, and pushes the result
    OP_INT_LT, // takes two ints, checks if the first is less than the second, and pushes the result
    OP_INT_LE, // takes two ints, checks if the first is less than or equal to the second, and pushes the result
    OP_INT_GT, // takes two ints, checks if the first is greater than the second, and pushes the result
    OP_INT_GE, // takes two ints, checks if the first is greater than or equal to the second, and pushes the result

    OP_FLOAT_EQ, // takes two floats, checks if they are equal, and pushes the result
    OP_FLOAT_LT, // takes two floats, checks if the first is less than the second, and pushes the result
    OP_FLOAT_LE, // takes two floats, checks if the first is less than or equal to the second, and pushes the result
    OP_FLOAT_GT, // takes two floats, checks if the first is greater than the second, and pushes the result
    OP_FLOAT_GE, // takes two floats, checks if the first is greater than or equal to the second, and pushes the result
};

/**
 * Apply an arithmetic operation to two integers
 * @param code The opcode to apply
 * @param a The first integer
 * @param b The second integer
 * @return The result of the arithmetic operation
 */
int64_t apply_op(Opcode code, int64_t a, int64_t b) {
    switch (code) {
    case OP_INT_ADD:
        return a + b;
    case OP_INT_SUB:
        return a - b;
    case OP_INT_MUL:
        return a * b;
    case OP_INT_DIV:
        return a / b;
    case OP_INT_MOD:
        return a % b;
    default:
        throw InvalidBytecodeError("Unknown opcode");
    }
}

/**
 * Apply an arithmetic operation to two floats
 * @param code The opcode to apply
 * @param a The first float
 * @param b The second float
 * @return The result of the arithmetic operation
 */
double apply_op(Opcode code, double a, double b) {
    switch (code) {
    case OP_FLOAT_ADD:
        return a + b;
    case OP_FLOAT_SUB:
        return a - b;
    case OP_FLOAT_MUL:
        return a * b;
    case OP_FLOAT_DIV:
        return a / b;
    default:
        throw InvalidBytecodeError("Unknown opcode");
    }
}

/**
 * Apply a comparison operation to two integers
 * @param code The opcode to apply
 * @param a The first integer
 * @param b The second integer
 * @return The result of the comparison
 */
bool apply_comparison(Opcode code, int64_t a, int64_t b) {
    switch (code) {
    case OP_INT_EQ:
        return a == b;
    case OP_INT_GT:
        return a > b;
    case OP_INT_GE:
        return a >= b;
    case OP_INT_LT:
        return a < b;
    case OP_INT_LE:
        return a <= b;
    default:
        throw InvalidBytecodeError("Unknown opcode");
    }
}

/**
 * Apply a comparison operation to two floats
 * @param code The opcode to apply
 * @param a The first float
 * @param b The second float
 * @return The result of the comparison
 */
bool apply_comparison(Opcode code, double a, double b) {
    switch (code) {
    case OP_FLOAT_EQ:
        return a == b;
    case OP_FLOAT_GT:
        return a > b;
    case OP_FLOAT_GE:
        return a >= b;
    case OP_FLOAT_LT:
        return a < b;
    case OP_FLOAT_LE:
        return a <= b;
    default:
        throw InvalidBytecodeError("Unknown opcode");
    }
}
void run(const ConstantPool &pool, const char *bytes, size_t size, strings_t &strings) {
    std::unique_ptr<char[]> locals_buf = std::make_unique<char[]>(1024);
    OperandStack stack = OperandStack(1024);

    unsigned int ip = 0;
    char *locals = locals_buf.get();
    int local_frame = 0;
    while (ip < size) {
        // Read the opcode
        switch (bytes[ip]) {
        case OP_PUSH_INT: {
            // Read the 8 byte int value
            int64_t value = *(int64_t *)(bytes + ip + 1);
            ip += 9;
            // Push the value onto the stack
            stack.push_int(ntohl(value));
            break;
        }
        case OP_PUSH_BYTE: {
            char value = *(bytes + ip + 1);
            ip += 2;
            stack.push_byte(value);
            break;
        }
        case OP_PUSH_FLOAT: {
            // Read the 8 byte float value
            int64_t value = ntohl(*(int64_t *)(bytes + ip + 1));
            ip += 9;
            // Push the value onto the stack
            stack.push_double(reinterpret_cast<double &>(value));
            break;
        }
        case OP_PUSH_STRING: {
            // Read the string reference
            constant_index index = ntohl(*(constant_index *)(bytes + ip + 1));
            ip += sizeof(constant_index) + 1;

            const std::string *str_ref = &pool.get_string(index);

            // Push the string index onto the stack
            stack.push_reference((uintptr_t)str_ref);
            break;
        }
        case OP_SPAWN: {
            // Read the 1 byte stack size
            char frame_size = *(bytes + ip + 1);
            ip += 2;

            // Create argv of the given frame_size, and create a new string for each arg with a null byte after each string
            std::vector<std::unique_ptr<char[]>> argv(frame_size + 1);
            for (int i = frame_size - 1; i >= 0; i--) {
                // Pop the string reference
                uintptr_t reference = stack.pop_reference();
                // cast the ref to a string pointer
                const std::string &arg = *(std::string *)reference;
                size_t arg_length = arg.length() + 1; // add 1 for the trailing '\0' char
                // Allocate the string
                argv[i] = std::make_unique<char[]>(arg_length);
                // copy the string fata
                memcpy(argv[i].get(), arg.c_str(), arg_length);
            }

            // Fork and exec the process
            pid_t pid = fork();
            if (pid == 0) {
                // Replace the current process with a new process image
                if (execvp(argv[0].get(), reinterpret_cast<char *const *>(argv.data())) == -1) {
                    perror("execvp");
                    _exit(MOSHELL_COMMAND_NOT_RUNNABLE);
                }
            } else if (pid == -1) {
                perror("fork");
            } else {
                int status = 0;
                // Wait for the process to finish
                waitpid(pid, &status, 0);

                // Add the exit status to the stack
                stack.push_byte(WEXITSTATUS(status) & 0xFF);
            }
            break;
        }
        case OP_GET_BYTE: {
            // Read the 1 byte local local_index
            char local_index = *(bytes + ip + 1);
            ip += 2;
            // Push the local onto the stack
            stack.push_byte(*(locals + local_frame + local_index * 8));
            break;
        }
        case OP_SET_BYTE: {
            // Read the 1 byte local index
            char local_index = *(bytes + ip + 1);
            ip += 2;
            // Pop the value from the stack
            char value = stack.pop_byte();
            // Set the local
            *(locals + local_frame + local_index * 8) = value;
            break;
        }
        case OP_GET_Q_WORD: {
            // Read the 1 byte local local_index
            char local_index = *(bytes + ip + 1);
            ip += 2;
            // Push the local onto the stack
            stack.push_reference(*(uint64_t *)(locals + local_frame + local_index * 8));
            break;
        }
        case OP_SET_Q_WORD: {
            // Read the 1 byte local index
            char local_index = *(bytes + ip + 1);
            ip += 2;
            // Pop the value from the stack
            int64_t value = stack.pop_int();
            // Set the local
            *(uintptr_t *)(locals + local_frame + local_index * 8) = value;
            break;
        }
        case OP_GET_REF: {
            // Read the 1 byte local local_index
            char local_index = *(bytes + ip + 1);
            ip += 2;
            // Push the local onto the stack
            stack.push_reference(*(uintptr_t *)(locals + local_frame + local_index * 8));
            break;
        }
        case OP_SET_REF: {
            // Read the 1 byte local index
            char local_index = *(bytes + ip + 1);
            ip += 2;
            // Pop the value from the stack
            uintptr_t value = stack.pop_reference();
            // Set the local
            *(uintptr_t *)(locals + local_frame + local_index * 8) = value;
            break;
        }
        case OP_BYTE_TO_INT: {
            char value = stack.pop_byte();
            stack.push_int(value);
            ip++;
            break;
        }
        case OP_INT_TO_STR: {
            int64_t value = stack.pop_int();

            auto [it, _] = strings.insert(std::make_unique<std::string>(std::to_string(value)));
            stack.push_reference((uintptr_t)it->get());

            // goto next operation
            ip++;
            break;
        }
        case OP_FLOAT_TO_STR: {
            double value = stack.pop_double();

            auto [it, _] = strings.insert(std::make_unique<std::string>(std::to_string(value)));
            stack.push_reference((uintptr_t)it->get());

            // goto next operation
            ip++;
            break;
        }
        case OP_INT_TO_BYTE: {
            int64_t i = stack.pop_int();
            stack.push_byte((char)i);
            ip++;
            break;
        }
        case OP_CONCAT: {
            auto right = (const std::string *)stack.pop_reference();
            auto left = (const std::string *)stack.pop_reference();

            std::string result = *left + *right;

            auto [it, _] = strings.insert(std::make_unique<std::string>(std::move(result)));
            stack.push_reference((uintptr_t)it->get());

            ip++;
            break;
        }
        case OP_IF_NOT_JUMP:
        case OP_IF_JUMP: {
            char value = stack.pop_byte();
            size_t then_branch = ntohl(*(size_t *)(bytes + ip + sizeof(char)));
            // test below means "test is true if value is 1 and we are in a if-jump,
            //                    or if value is not 1 and we are in a if-not-jump operation"
            if (value == (bytes[ip] == OP_IF_JUMP)) {
                ip = then_branch;
            } else {
                // the length of if-jump opcode and its branch destination
                ip += 1 + sizeof(size_t);
            }
            break;
        }
        case OP_JUMP: {
            size_t destination = ntohl(*(size_t *)(bytes + ip + sizeof(char)));
            ip = destination;
            break;
        }
        case OP_POP_BYTE: {
            stack.pop_byte();
            ip++;
            break;
        }
        case OP_POP_Q_WORD: {
            stack.pop_bytes(8);
            ip++;
            break;
        }
        case OP_B_XOR: {
            char a = stack.pop_byte();
            char b = stack.pop_byte();
            stack.push_byte((char)(a ^ b));
            ip++;
            break;
        }
        case OP_INT_ADD:
        case OP_INT_SUB:
        case OP_INT_MUL:
        case OP_INT_DIV:
        case OP_INT_MOD: {
            int64_t b = stack.pop_int();
            int64_t a = stack.pop_int();
            int64_t res = apply_op(static_cast<Opcode>(bytes[ip]), a, b);
            stack.push_int(res);
            ip++;
            break;
        }
        case OP_FLOAT_ADD:
        case OP_FLOAT_SUB:
        case OP_FLOAT_MUL:
        case OP_FLOAT_DIV: {
            double b = stack.pop_double();
            double a = stack.pop_double();
            double res = apply_op(static_cast<Opcode>(bytes[ip]), a, b);
            stack.push_double(res);
            ip++;
            break;
        }
        case OP_STR_EQ: {
            const std::string &b = *(const std::string *)stack.pop_reference();
            const std::string &a = *(const std::string *)stack.pop_reference();
            stack.push_byte(a == b);
            ip++;
            break;
        }
        case OP_INT_EQ:
        case OP_INT_LT:
        case OP_INT_LE:
        case OP_INT_GT:
        case OP_INT_GE: {
            int64_t b = stack.pop_int();
            int64_t a = stack.pop_int();
            char res = apply_comparison(static_cast<Opcode>(bytes[ip]), a, b);
            stack.push_byte(res);
            ip++;
            break;
        }
        case OP_FLOAT_EQ:
        case OP_FLOAT_LT:
        case OP_FLOAT_LE:
        case OP_FLOAT_GT:
        case OP_FLOAT_GE: {
            double b = stack.pop_double();
            double a = stack.pop_double();
            char res = apply_comparison(static_cast<Opcode>(bytes[ip]), a, b);
            stack.push_byte(res);
            ip++;
            break;
        }
        default: {
            throw InvalidBytecodeError("Unknown opcode " + std::to_string(bytes[ip]));
        }
        }
    }
}
