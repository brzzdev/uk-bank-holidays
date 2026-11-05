# prettier-ignore

set default-list

# Build a release binary.
build-release:
  cargo build --release

# Format the source code.
format:
  cargo fmt

# Check the source code is formatted, without rewriting it. Mirrors CI.
format-check:
  cargo fmt --check

# Lint the source code.
lint:
  cargo clippy --all-targets -- -D warnings

# Run the test suite.
test:
  cargo test

# Install a git pre-commit hook that runs `format-check` and `lint`.
tools:
  #!/usr/bin/env sh
  hook="$(git rev-parse --git-dir)/hooks/pre-commit"
  printf '%s\n' '#!/usr/bin/env sh' 'set -e' 'just format-check' 'just lint' > "$hook"
  chmod +x "$hook"
  echo "Installed pre-commit hook."
