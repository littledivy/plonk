#include <stdio.h>
#include <stdlib.h>

#include "deps/frida-gum.h"

__attribute__((constructor))
static void init() 
{
  char *sym = getenv("SYMBOL");
  char *new_sym = getenv("NEW_SYMBOL");
  if (!sym || !new_sym)
    return;

  GumInterceptor * interceptor;
  gum_init_embedded ();

  interceptor = gum_interceptor_obtain ();

  void *original = GSIZE_TO_POINTER (gum_module_find_export_by_name (NULL, sym));
  if (!original) {
    fprintf(stderr, "Could not find symbol %s\n", sym);
    return;
  }

  void *new = GSIZE_TO_POINTER (gum_module_find_export_by_name (NULL, new_sym));
  if (!new) {
    fprintf(stderr, "Could not find symbol new\n");
    return;
  }

  gum_interceptor_replace_fast(interceptor, original, new, NULL);

  fprintf(stderr, "Replaced %s with new\n", sym);
}
