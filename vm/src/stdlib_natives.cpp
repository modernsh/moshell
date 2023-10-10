#include "stdlib_natives.h"
#include "interpreter.h"
#include "memory/heap.h"
#include <charconv>
#include <cmath>
#include <cstring>
#include <fstream>
#include <iostream>

static void int_to_string(OperandStack &caller_stack, runtime_memory &mem) {
    int64_t value = caller_stack.pop_int();
    msh::obj &str = mem.emplace(std::to_string(value));
    caller_stack.push_reference(str);
}

static void float_to_string(OperandStack &caller_stack, runtime_memory &mem) {
    double value = caller_stack.pop_double();
    msh::obj &str = mem.emplace(std::to_string(value));
    caller_stack.push_reference(str);
}

static void str_concat(OperandStack &caller_stack, runtime_memory &mem) {
    const std::string &right = caller_stack.pop_reference().get<const std::string>();
    const std::string &left = caller_stack.pop_reference().get<const std::string>();

    std::string result = left + right;

    msh::obj &str = mem.emplace(std::move(result));
    caller_stack.push_reference(str);
}

static void str_eq(OperandStack &caller_stack, runtime_memory &) {
    const std::string &right = caller_stack.pop_reference().get<const std::string>();
    const std::string &left = caller_stack.pop_reference().get<const std::string>();
    int8_t test = static_cast<int8_t>(right == left);
    caller_stack.push_byte(test);
}

static void get_env(OperandStack &caller_stack, runtime_memory &mem) {
    const std::string &var_name = caller_stack.pop_reference().get<const std::string>();
    const char *value = getenv(var_name.c_str());
    if (value == nullptr) {
        caller_stack.push(nullptr);
    } else {
        caller_stack.push_reference(mem.emplace(value));
    }
}

static void set_env(OperandStack &caller_stack, runtime_memory &) {
    const std::string &value = caller_stack.pop_reference().get<const std::string>();
    const std::string &var_name = caller_stack.pop_reference().get<const std::string>();
    setenv(var_name.c_str(), value.c_str(), true);
}

static void panic(OperandStack &caller_stack, runtime_memory &) {
    const std::string &message = caller_stack.pop_reference().get<const std::string>();
    throw RuntimeException(message);
}

static void exit(OperandStack &caller_stack, runtime_memory &) {
    uint8_t code = caller_stack.pop_byte();
    exit(code);
}

static void read_line(OperandStack &caller_stack, runtime_memory &mem) {
    std::string line;
    std::getline(std::cin, line);

    msh::obj &obj = mem.emplace(line);
    caller_stack.push_reference(obj);
}

static void new_vec(OperandStack &caller_stack, runtime_memory &mem) {
    msh::obj &obj = mem.emplace(msh::obj_vector());
    caller_stack.push_reference(obj);
}

static void some(OperandStack &, runtime_memory &) {
    // the argument is the returned value
}

static void none(OperandStack &caller_stack, runtime_memory &) {
    caller_stack.push(nullptr);
}

static void floor(OperandStack &caller_stack, runtime_memory &) {
    double d = caller_stack.pop_double();

    caller_stack.push_int(static_cast<int64_t>(std::floor(d)));
}

static void ceil(OperandStack &caller_stack, runtime_memory &) {
    double d = caller_stack.pop_double();

    caller_stack.push_int(static_cast<int64_t>(std::ceil(d)));
}

static void round(OperandStack &caller_stack, runtime_memory &) {
    double d = caller_stack.pop_double();

    caller_stack.push_int(static_cast<int64_t>(std::round(d)));
}

static void parse_int_radix(OperandStack &caller_stack, runtime_memory &mem) {
    int base = static_cast<int>(caller_stack.pop_int());
    const std::string &str = caller_stack.pop_reference().get<const std::string>();

    if (base < 2 || base > 36) {
        throw RuntimeException("Invalid base: " + std::to_string(base) + ".");
    }
    const char *first = str.data();
    if (!str.empty() && str.front() == '+') { // Allow leading '+'
        first += 1;
    }

    int64_t value = 0;
    const auto result = std::from_chars(first,
                                        str.data() + str.size(),
                                        value, base);

    // Ensure that the entire string was consumed and that the result is valid
    if (result.ec == std::errc() && result.ptr == &*str.end()) {
        caller_stack.push_reference(mem.emplace(value));
    } else {
        caller_stack.push(nullptr);
    }
}

static void str_split(OperandStack &caller_stack, runtime_memory &mem) {
    const std::string &delim = caller_stack.pop_reference().get<const std::string>();
    const std::string &str = caller_stack.pop_reference().get<const std::string>();

    msh::obj &res_obj = mem.emplace(msh::obj_vector());
    caller_stack.push_reference(res_obj);
    msh::obj_vector &res = res_obj.get<msh::obj_vector>();

    if (delim.empty()) {
        throw RuntimeException("The delimiter is empty.");
    }

    std::string word;
    size_t start = 0, end, delim_len = delim.length();
    while ((end = str.find(delim, start)) != std::string::npos) {
        word = str.substr(start, end - start);
        start = end + delim_len;
        res.push_back(&mem.emplace(word));
    }
    res.push_back(&mem.emplace(str.substr(start)));
}

static void str_bytes(OperandStack &caller_stack, runtime_memory &mem) {
    const std::string &str = caller_stack.pop_reference().get<const std::string>();
    msh::obj_vector res;
    res.reserve(str.length());

    msh::obj &heap_obj = mem.emplace(std::move(res));
    caller_stack.push_reference(heap_obj);
    msh::obj_vector &heap_res = heap_obj.get<msh::obj_vector>();

    for (char c : str) {
        heap_res.push_back(&mem.emplace(static_cast<int64_t>(c)));
    }
}

static void vec_len(OperandStack &caller_stack, runtime_memory &) {
    const msh::obj_vector &vec = caller_stack.pop_reference().get<msh::obj_vector>();
    caller_stack.push_int(static_cast<int64_t>(vec.size()));
}

static void vec_pop(OperandStack &caller_stack, runtime_memory &) {
    msh::obj_vector &vec = caller_stack.pop_reference().get<msh::obj_vector>();
    if (vec.empty()) {
        caller_stack.push(nullptr);
        return;
    }
    msh::obj &last_element = *vec.back();
    vec.pop_back();
    caller_stack.push_reference(last_element);
}

static void vec_pop_head(OperandStack &caller_stack, runtime_memory &) {
    msh::obj_vector &vec = caller_stack.pop_reference().get<msh::obj_vector>();
    msh::obj *first_element = *vec.begin();
    vec.erase(vec.begin());
    caller_stack.push_reference(*first_element);
}

static void vec_push(OperandStack &caller_stack, runtime_memory &) {
    msh::obj &ref = caller_stack.pop_reference();
    msh::obj_vector &vec = caller_stack.pop_reference().get<msh::obj_vector>();
    vec.push_back(&ref);
}

static void vec_index(OperandStack &caller_stack, runtime_memory &) {
    int64_t n = caller_stack.pop_int();
    size_t index = static_cast<size_t>(n);
    msh::obj_vector &vec = caller_stack.pop_reference().get<msh::obj_vector>();
    if (index >= vec.size()) {
        throw RuntimeException("Index " + std::to_string(n) + " is out of range, the length is " + std::to_string(vec.size()) + ".");
    }
    caller_stack.push_reference(*vec[index]);
}

static void vec_index_set(OperandStack &caller_stack, runtime_memory &) {
    msh::obj &ref = caller_stack.pop_reference();
    int64_t n = caller_stack.pop_int();
    size_t index = static_cast<size_t>(n);
    msh::obj_vector &vec = caller_stack.pop_reference().get<msh::obj_vector>();
    if (index >= vec.size()) {
        throw RuntimeException("Index " + std::to_string(n) + " is out of range, the length is " + std::to_string(vec.size()) + ".");
    }
    vec[index] = &ref;
}

static void gc(OperandStack &, runtime_memory &mem) {
    mem.run_gc();
}

static void is_operands_empty(OperandStack &os, runtime_memory &) {
    os.push(os.size() == 0);
}

static void program_arguments(OperandStack &os, runtime_memory &mem) {
    std::ifstream process_cmdline;
    process_cmdline.open("/proc/" + std::to_string(getpid()) + "/cmdline");
    if (!process_cmdline.is_open()) {
        throw RuntimeException("Could not get process's arguments");
    }
    int cmdline_cap = 512;
    char *cmdline = (char *)malloc(cmdline_cap);

    while (true) {
        process_cmdline.read(cmdline, cmdline_cap);
        if (process_cmdline.eof()) {
            break;
        }
        cmdline_cap += 512;
        cmdline = (char *)realloc(cmdline, cmdline_cap);
    }

    msh::obj &vec_obj = mem.emplace(msh::obj_vector());
    os.push_reference(vec_obj);
    msh::obj_vector &vec = vec_obj.get<msh::obj_vector>();

    int current_pos = strlen(cmdline) + 1; // skip process's name
    while (current_pos < process_cmdline.gcount()) {
        int arg_len = strlen(cmdline + current_pos);
        vec.push_back(&mem.emplace(std::string(cmdline + current_pos, arg_len)));
        current_pos += arg_len + 1;
    }
    free(cmdline);
}

natives_functions_t load_natives() {
    return natives_functions_t{
        {"lang::Int::to_string", int_to_string},
        {"lang::Float::to_string", float_to_string},

        {"lang::String::concat", str_concat},
        {"lang::String::eq", str_eq},
        {"lang::String::split", str_split},
        {"lang::String::bytes", str_bytes},

        {"lang::Vec::pop", vec_pop},
        {"lang::Vec::pop_head", vec_pop_head},
        {"lang::Vec::len", vec_len},
        {"lang::Vec::push", vec_push},
        {"lang::Vec::[]", vec_index},
        {"lang::Vec::[]=", vec_index_set},

        {"std::panic", panic},
        {"std::exit", exit},
        {"std::env", get_env},
        {"std::set_env", set_env},
        {"std::read_line", read_line},
        {"std::new_vec", new_vec},
        {"std::some", some},
        {"std::none", none},

        {"std::memory::gc", gc},
        {"std::memory::empty_operands", is_operands_empty},
        {"std::memory::program_arguments", program_arguments},

        {"std::convert::ceil", ceil},
        {"std::convert::floor", floor},
        {"std::convert::round", round},
        {"std::convert::parse_int_radix", parse_int_radix},
    };
}
