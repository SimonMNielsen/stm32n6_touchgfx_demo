// Minimal C++ runtime for bare metal — avoids linking libstdc++/libsupc++.
//
// TouchGFX is compiled -fno-exceptions -fno-rtti and allocates everything
// statically (FrontendHeap is a placement-new arena), so the only ABI pieces
// it needs are pure-virtual traps, static-destructor registration (which we
// discard: the firmware never exits) and a trapping global operator new to
// catch accidental heap use.

#include <cstddef>
#include <cstdint>

extern "C" void __cxa_pure_virtual()
{
    while (true) {}
}

// Static objects with destructors register them via __aeabi_atexit / __cxa_atexit.
// Firmware never exits — accept and ignore.
extern "C" int __aeabi_atexit(void*, void (*)(void*), void*)
{
    return 0;
}

extern "C" int __cxa_atexit(void (*)(void*), void*, void*)
{
    return 0;
}

void* __dso_handle = nullptr;

// TouchGFX must not heap-allocate; trap loudly if something tries.
void* operator new(size_t)
{
    while (true) {}
}

void* operator new[](size_t)
{
    while (true) {}
}

void operator delete(void*) noexcept {}
void operator delete[](void*) noexcept {}
void operator delete(void*, size_t) noexcept {}
void operator delete[](void*, size_t) noexcept {}
