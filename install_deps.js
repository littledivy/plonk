import { $ } from "https://deno.land/x/dax/mod.ts";

const arch = "arm64";
const os = "macos";
const version = "16.0.19";

const devkit_name = `frida-gum-devkit-${version}-${os}-${arch}`;
let fridaUrl = `https://github.com/frida/frida/releases/download/${version}/${devkit_name}.tar.xz`;

const res = await fetch(fridaUrl);
const file = Deno.openSync(`deps/${devkit_name}.tar.xz`, { write: true, create: true });
await res.body.pipeTo(file.writable);

await $`tar -xf ${devkit_name}.tar.xz`.cwd("deps");
