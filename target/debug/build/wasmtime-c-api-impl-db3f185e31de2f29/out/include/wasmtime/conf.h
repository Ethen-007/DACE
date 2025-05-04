/**
 * \file wasmtime/conf.h
 *
 * \brief Build-time defines for how the C API was built.
 */

#ifndef WASMTIME_CONF_H
#define WASMTIME_CONF_H

// WASMTIME_FEATURE_LIST
/* #undef WASMTIME_FEATURE_PROFILING */
/* #undef WASMTIME_FEATURE_WAT */
/* #undef WASMTIME_FEATURE_CACHE */
/* #undef WASMTIME_FEATURE_PARALLEL_COMPILATION */
/* #undef WASMTIME_FEATURE_WASI */
/* #undef WASMTIME_FEATURE_LOGGING */
/* #undef WASMTIME_FEATURE_DISABLE_LOGGING */
/* #undef WASMTIME_FEATURE_COREDUMP */
/* #undef WASMTIME_FEATURE_ADDR2LINE */
/* #undef WASMTIME_FEATURE_DEMANGLE */
/* #undef WASMTIME_FEATURE_THREADS */
/* #undef WASMTIME_FEATURE_GC */
/* #undef WASMTIME_FEATURE_ASYNC */
#define WASMTIME_FEATURE_CRANELIFT
/* #undef WASMTIME_FEATURE_WINCH */
// ... if you add a line above this be sure to change the other locations
// marked WASMTIME_FEATURE_LIST

#if defined(WASMTIME_FEATURE_CRANELIFT) || defined(WASMTIME_FEATURE_WINCH)
#define WASMTIME_FEATURE_COMPILER
#endif

#endif // WASMTIME_CONF_H
