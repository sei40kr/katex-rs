#!/usr/bin/env node
// Renders a TeX expression to MathML using the upstream `katex` package
// pinned in the repo-root `package.json`. Used by `cargo xtask snapshot`
// to regenerate expected files; the Rust test driver never invokes this
// script.
//
// Usage:
//   node tools/render-mathml.mjs '<tex source>'
//   echo '<tex source>' | node tools/render-mathml.mjs -
//
// Emits the inner `<math>` element only, stripped of upstream's
// `<span class="katex">` wrapper. No additional normalization is done
// here — that lives in `xtask/src/normalize.rs` so Node and Rust agree
// byte-for-byte.

import { readFileSync } from "node:fs";
import katex from "katex";

const arg = process.argv[2];
if (arg === undefined || arg === "--help" || arg === "-h") {
  console.error("usage: node tools/render-mathml.mjs '<tex source>' | -");
  process.exit(2);
}

const tex = arg === "-" ? readFileSync(0, "utf8") : arg;

let html;
try {
  html = katex.renderToString(tex, {
    output: "mathml",
    throwOnError: true,
    strict: "ignore",
  });
} catch (err) {
  console.error(`render error: ${err.message ?? err}`);
  process.exit(1);
}

const open = html.indexOf("<math");
const close = html.lastIndexOf("</math>");
if (open === -1 || close === -1) {
  console.error("upstream output did not contain a <math> element");
  console.error(html);
  process.exit(1);
}
process.stdout.write(html.slice(open, close + "</math>".length));
process.stdout.write("\n");
