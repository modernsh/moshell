#pragma once

#include <cstdint>
#include <forward_list>
#include <string>
#include <variant>
#include <vector>

namespace msh {
    // Create a recursive variant type by forward declaring the vector type.
    // Since C++17, `std::vector` doesn't require the type to be complete with
    // an appropriate allocator, and only a pointer is used here.

    class obj;

    /**
     * A vector of heap allocated objects.
     */
    struct obj_vector : public std::vector<obj *> {
        using std::vector<obj *>::vector;
    };

    using obj_data = std::variant<int64_t, double, const std::string, obj_vector>;

    class gc;

    /**
     * A vm object that can be stored in the heap.
     */
    class obj {
        mutable uint8_t gc_cycle;
        obj_data data;

        friend gc;

    public:
        template <typename T>
        obj(T val) : gc_cycle{0}, data{std::move(val)} {}

        obj_data &get_data();

        template <typename T>
        T &get() {
            return std::get<T>(data);
        }
        template <typename T>
        const T &get() const {
            return std::get<T>(data);
        }
    };

    /**
     * A collection of objects that can be referenced by other objects.
     *
     * The VM keep track of all objects allocated in the heap.
     */
    class heap {
        /**
         * The allocated objects.
         *
         * A linked list is used to avoid invalidating references to objects when
         * inserting or removing new objects.
         */
        std::forward_list<obj> objects;

        /**
         * heap size
         * */
        size_t len;

        friend gc;

    public:
        /**
         * Inserts a new object in the heap.
         *
         * @param obj The object to insert.
         * @return A reference to this object, valid as long as the object is not deleted.
         */
        msh::obj &insert(msh::obj &&obj);

        size_t size() const;
    };
}