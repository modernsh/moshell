#pragma once

#include "definitions/bytecode_unit.h"
#include "memory/operand_stack.h"
#include "memory/strings.h"
#include "stdlib_natives.h"

/**
 * Will run given bytecode's main method.
 * @throws InvalidBytecodeStructure if the given bytecode does not defines a <main>() function
 * @throws InvalidBytecodeError if an interpreted instruction set contains invalid instructions
 */
void run_unit(const bytecode_unit &module_def, StringsHeap &strings, const natives_functions_t &natives);
