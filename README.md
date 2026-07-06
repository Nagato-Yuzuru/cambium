Under development.

# Cambium

A [Scheme](https://small.r7rs.org/) compiler in Rust, with two backends behind one shared
frontend:

- **VM** _(the MVP)_ — a stack-based bytecode compiler + interpreter that runs in-process.
- **wasm-GC** _(stretch)_ — ahead-of-time compilation to a WebAssembly GC module that runs on
  [wasmtime](https://wasmtime.dev/), with a Rust host supplying the runtime imports.

The name comes from the botanical [cambium](https://en.wikipedia.org/wiki/Cambium): the growth
layer that differentiates outward into phloem and inward into xylem. The shared frontend
(reader → expander → core IR) plays the same role here, differentiating into the two backends.

## Why

[Steel](https://github.com/mattwparas/steel) and [scheme-rs](https://github.com/maplant/scheme-rs)
are solid Rust Scheme VMs. [Guile Hoot](https://spritely.institute/hoot/) compiles Scheme to
wasm-GC but targets browsers, not wasmtime. Cambium combines a VM and a wasm-GC backend from one
frontend, with the wasm-GC output running on wasmtime and a Rust host providing the imports —
plus sandboxing, fuel metering, and debugging that only a server-side runtime needs.

## Getting started

Toolchain is pinned and installed via [`mise`](https://mise.jdx.dev/); tasks run through
[`just`](https://just.systems/).

```sh
mise install
just hooks
```

## License

MIT, see [LICENSE](LICENSE).
