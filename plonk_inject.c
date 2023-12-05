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
#include <dlfcn.h>

#include "frida-gum.h"

__attribute__((constructor))
static void init() 
{
  GumInterceptor * interceptor;
  char *sym, *new_sym, *lib, *bin, *verbose;
  void *dl, *original, *new;

  sym = getenv("SYMBOL");
  new_sym = getenv("NEW_SYMBOL");

  /* Library with the new symbols */
  lib = getenv("PLONK_LIBRARY");
  /* Binary with the original symbols */
  bin = getenv("PLONK_BINARY");
  verbose = getenv("VERBOSE");

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
  dl = dlopen(lib, RTLD_LAZY);
  if (!dl) {
    fprintf(stderr, "[*] Could not open library %s\n", lib);
    fprintf(stderr, "[*] Error: %s\n", dlerror());
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
