# AIL IDE

Desktop IDE for the AIL project, built with Tauri v2 + SvelteKit.

Run `pnpm install` once to install dependencies. Use `pnpm run dev` to start the
SvelteKit dev server in the browser (port 1420, strict). Use `pnpm tauri dev` to
launch the full desktop window (requires Rust toolchain). Use `pnpm run build` to
produce the static frontend in `ide/build/`. Windows users need WebView2 installed
for `pnpm tauri build`; `cargo check` works without it. Note: `ide/src-tauri/` is
a standalone Cargo project — it is NOT a member of the root workspace, so
`cargo build --workspace` from the repo root stays Tauri-free.
