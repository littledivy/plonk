/* Injector dynamic library for Plonk */

#include <stdio.h>
#include <stdlib.h>
#include <dlfcn.h>

#include "deps/frida-gum.h"

__attribute__((constructor))
static void init() 
{
  GumInterceptor * interceptor;
  char *sym, *new_sym, *lib, *verbose;
  void *dl, *original, *new;

  sym = getenv("SYMBOL");
  new_sym = getenv("NEW_SYMBOL");

  /* Library with the new symbols */
  lib = getenv("PLONK_LIBRARY");
  verbose = getenv("VERBOSE");

  if (!sym || !lib)
    return;
  /* Assume same identifier as the original symbol */
  if (!new_sym)
    new_sym = sym;

  gum_init_embedded();

  interceptor = gum_interceptor_obtain();

  original = GSIZE_TO_POINTER(gum_module_find_export_by_name(NULL, sym));
  if (!original) {
    fprintf(stderr, "[*] Could not find symbol %s\n", sym);
    return;
  }

  /* Leak (intentional) */
  dl = dlopen(lib, RTLD_LAZY);
  if (!dl) {
    fprintf(stderr, "[*] Could not open library %s\n", lib);
    return;
  }

  new = GSIZE_TO_POINTER (gum_module_find_export_by_name(lib, new_sym));
  if (!new) {
    fprintf(stderr, "[*] Could not find symbol %s in %s\n", new_sym, lib);
    fprintf(stderr, "    Did you forget #[no_mangle]?\n");
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
