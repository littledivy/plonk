/*
 * Copyright (c) 2023 Divy Srivastava
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in
 * all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL
 * THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
 * THE SOFTWARE.
 */

/* Injector dynamic library for Plonk */

#include <stdio.h>
#include <stdlib.h>

#if defined(__APPLE__) || defined(__linux__)
#include <dlfcn.h>
#define plonk_dlopen(name) dlopen(name, RTLD_LAZY)
#define plonk_dlerror() dlerror()
#define plonk_getenv(name) getenv(name)
#endif

#if defined(_WIN32)
#include <windows.h>
const char *dlerror()
{
  static char buf[256];
  DWORD err = GetLastError();
  if (!err)
    return NULL;
  FormatMessage(FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS,
                NULL, err, MAKELANGID(LANG_NEUTRAL, SUBLANG_DEFAULT), buf,
                sizeof(buf), NULL);
  return buf;
}
char *plonk_getenv(const char *name)
{
  // TODO(littledivy): this leaks
  char *buf = malloc(256);
  DWORD len = GetEnvironmentVariable(name, buf, 256);
  if (!len)
    return NULL;
  return buf;
}
#define plonk_dlopen(name) LoadLibrary(name)
#define plonk_dlerror() dlerror()

#endif

#include "frida-gum.h"
  
__attribute__((constructor))
static void init() 
{
  GumInterceptor * interceptor;
  char *sym, *new_sym, *lib, *bin, *verbose;
  void *dl, *original, *new;

  sym = plonk_getenv("SYMBOL");
  new_sym = plonk_getenv("NEW_SYMBOL");

  /* Library with the new symbols */
  lib = plonk_getenv("PLONK_LIBRARY");
  /* Binary with the original symbols */
  bin = plonk_getenv("PLONK_BINARY");
  verbose = plonk_getenv("VERBOSE");

  if (!sym || !lib)
    return;
  /* Assume same identifier as the original symbol */
  if (!new_sym)
    new_sym = sym;

  gum_init_embedded();

  interceptor = gum_interceptor_obtain();

  original = GSIZE_TO_POINTER(gum_module_find_export_by_name(NULL, sym));
  if (!original)
    original = GSIZE_TO_POINTER(gum_module_find_symbol_by_name(bin, sym));
  if (!original) {
    fprintf(stderr, "[*] Could not find symbol %s in bin\n", sym);
    return;
  }

  /* Leak (intentional) */
  dl = plonk_dlopen(lib);
  if (!dl) {
    fprintf(stderr, "[*] Could not open library %s\n", lib);
    fprintf(stderr, "[*] Error: %s\n", plonk_dlerror());
    return;
  }

  new = GSIZE_TO_POINTER (gum_module_find_export_by_name(lib, new_sym));
  if (!new) {
    fprintf(stderr, "[*] Could not find symbol %s in %s\n", new_sym, lib);
    return;
  }

  if (new == original) {
    fprintf(stderr, "[*] New symbol %s is the same as the original\n", new_sym);
    return;
  }

  if (verbose) {
    printf("[*] Plonking %s in %s\n", sym, lib);
    printf("[*] Old address: %p\n", original);
    printf("[*] New address: %p\n", new);
  }

  gum_interceptor_replace_fast(interceptor, original, new, NULL);
  if (verbose)
    printf("===\n");
}
