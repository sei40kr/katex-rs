# katex-rs &mdash; browser example

Minimal, no-bundler page that loads `katex-wasm` directly via an
ES-module import and renders TeX to MathML in the browser.

## Build

From the repository root, build the wasm module into `examples/web/pkg/`:

```sh
wasm-pack build crates/katex-wasm --target web --out-dir ../../examples/web/pkg
```

`wasm-pack` writes `katex_wasm.js` (the loader) and `katex_wasm_bg.wasm`
(the binary) into that directory. The folder is gitignored.

## Run

Serve `examples/web/` over HTTP &mdash; modern browsers refuse to load
ES modules from `file://`:

```sh
cd examples/web
python -m http.server
```

Then open <http://localhost:8000/>. The page renders a representative
slice of the snapshot corpus
(`crates/katex/tests/snapshots/inputs/`) and offers a textarea for
ad-hoc input.

## API

```js
import init, { renderToString } from "./pkg/katex_wasm.js";

await init();
const mathml = renderToString("\\frac{1}{2}", undefined);
//   <math …><mfrac><mn>1</mn><mn>2</mn></mfrac>…</math>
```

`renderToString(tex, opts)` &mdash; `opts` may be `undefined`, `null`,
or a plain object whose keys mirror upstream KaTeX's
[`Settings`](https://katex.org/docs/options.html) (camelCased: e.g.
`displayMode`, `throwOnError`, `errorColor`, `strict`, `macros`).
The function throws a JS `Error` on parse failure, matching upstream's
behavior when `throwOnError: true`.
