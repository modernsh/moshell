#pragma once

#include <exception>
#include <string>

/**
 * Base class for fatal errors that occurs inside the virtual machine
 */
class VirtualMachineError : public std::exception {
    const std::string msg;

public:
    explicit VirtualMachineError(std::string msg);

    virtual const char *name() const noexcept = 0;

    [[nodiscard]] const char *what() const noexcept override;
};

/**
 * VM Error subdivision for any error implied by bytecode interpretation
 */
class InvalidBytecodeError : public VirtualMachineError {
public:
    explicit InvalidBytecodeError(std::string msg) : VirtualMachineError(std::move(msg)) {}
    const char *name() const noexcept override {
        return "InvalidBytecodeError";
    }
};

/**
 * VM Error subdivision for any error related to VM's memory
 */
class MemoryError : public VirtualMachineError {
public:
    explicit MemoryError(std::string msg) : VirtualMachineError(std::move(msg)) {}
    const char *name() const noexcept override {
        return "MemoryError";
    }
};