# prettier-ignore

set default-list

# Format the source code.
format:
  cargo fmt

# Check the source code is formatted, without rewriting it. Mirrors CI.
format-check:
  cargo fmt --check

# Build Bank Holidays.app and install it in ~/Applications.
install-app:
  #!/usr/bin/env bash
  set -euo pipefail
  version="$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name == "bank-holidays-app") | .version')"
  if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "error: bank-holidays-app's version is $version; an app bundle needs a plain X.Y.Z" >&2
    exit 1
  fi
  src=crates/app/bundle
  MACOSX_DEPLOYMENT_TARGET="$(plutil -extract LSMinimumSystemVersion raw "$src/Info.plist")" cargo build --release -p bank-holidays-app
  app='target/Bank Holidays.app'
  iconset=target/AppIcon.iconset
  installed="$HOME/Applications/Bank Holidays.app"
  plist="$app/Contents/Info.plist"
  rm -rf "$app" "$iconset"
  mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" "$iconset"
  install -m 755 target/release/bank-holidays-app "$app/Contents/MacOS/"
  cp "$src/Info.plist" "$plist"
  plutil -insert CFBundleShortVersionString -string "$version" "$plist"
  plutil -insert CFBundleVersion -string "$version" "$plist"
  cp "$src/Credits.rtf" "$app/Contents/Resources/"
  for size in 16 32 128 256 512; do
    sips -z "$size" "$size" "$src/icon.png" --out "$iconset/icon_${size}x${size}.png" >/dev/null
    sips -z "$((size * 2))" "$((size * 2))" "$src/icon.png" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
  done
  iconutil -c icns "$iconset" -o "$app/Contents/Resources/AppIcon.icns"
  codesign --force --sign - "$app"
  rm -rf "$installed"
  ditto "$app" "$installed"
  echo "Installed $installed $version."

# Lint the source code.
lint:
  cargo clippy --all-targets -- -D warnings

# Run the test suite.
test:
  cargo test

# Install a git pre-commit hook that runs `format-check` and `lint`.
tools:
  #!/usr/bin/env sh
  set -e
  hook="$(git rev-parse --git-path hooks/pre-commit)"
  printf '%s\n' '#!/usr/bin/env sh' 'set -e' 'just format-check' 'just lint' > "$hook"
  chmod +x "$hook"
  echo "Installed pre-commit hook."
