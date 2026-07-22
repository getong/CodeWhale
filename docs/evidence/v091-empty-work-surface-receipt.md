# v0.9.1 empty Work-surface receipt

This receipt verifies the empty-surface behavior from the real TUI binary. It
is not based on a product mockup.

## Source

- Version: `codewhale-tui 0.9.1 (4d197626d72b)`
- Commit: `4d197626d72b4bd27e1abf4eed92e86e914414a8`
- Checkout at capture: clean exact-source worktree for
  `codex/v091-final-integration-20260721`
- Terminal: `120x32`
- Route: isolated local Ollama configuration using `qwen3-coder`; no API key
- Workspace: clean public fixture shown as `~/codewhale-demo`
- Theme: Blue Stage dark

## Capture method

The binary was built with:

```bash
cargo build --release --locked -p codewhale-tui --bin codewhale-tui
```

It was launched in a 120-column by 32-row tmux PTY with isolated state. VHS
0.10.0 rasterized the real terminal cells at 1280x720; it did not generate or
reconstruct product UI:

```bash
NO_ANIMATIONS=1 \
CODEWHALE_HOME=/path/to/sealed-home/.codewhale \
CODEWHALE_CONFIG_PATH=/path/to/sealed-home/.codewhale/config.toml \
CODEWHALE_MCP_CONFIG=/path/to/sealed-home/.codewhale/mcp.json \
target/release/codewhale-tui \
  --skip-onboarding \
  --fresh \
  --no-project-config \
  --no-mouse-capture \
  --workspace ~/codewhale-demo
```

The resulting 1280x720 capture has SHA-256
`8ffd0c36699930a9af7bcca3e93d3f9bc8a11df5a691e88335fc8b1f0442a754`.
It contains no username, credential, account identifier, private repository
path, error state, or unsupported product claim. The idle capture process had
no open TCP or UDP socket. `NO_ANIMATIONS=1` makes this one canonical still
stable; the separate real-PTY suite proves full, reduced, and still motion.
The captured header visibly identifies `v0.9.1 (4d197626d72b)`, and the context
line is `~/codewhale-demo · main · mcp 0`.

## Acceptance

The fresh active session contains the header, idle canvas, composer, and
footer. It does not contain `Work · empty`, a Work heading, or reserved Work
rows. Active, error, and disconnected Work projections remain covered by the
TUI unit suite.

```text
cargo test -p codewhale-tui --bins --all-features --locked
test result: ok. 8063 passed; 0 failed; 4 ignored

cargo test -p codewhale-tui --test qa_pty --locked
test result: ok. 25 passed; 0 failed; 1 ignored
```
