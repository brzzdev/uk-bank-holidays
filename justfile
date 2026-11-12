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
  package=bank-holidays-app
  bundle='Bank Holidays.app'
  resources=crates/app/bundle
  metadata="$(cargo metadata --no-deps --format-version 1)"
  version="$(jq -r --arg package "$package" '.packages[] | select(.name == $package) | .version' <<<"$metadata")"
  if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "error: $package's version is $version; an app bundle needs a plain X.Y.Z" >&2
    exit 1
  fi
  target="$(jq -r .target_directory <<<"$metadata")"
  executable="$(MACOSX_DEPLOYMENT_TARGET="$(plutil -extract LSMinimumSystemVersion raw "$resources/Info.plist")" \
    cargo build --release -p "$package" --message-format json-render-diagnostics \
    | jq -r --arg package "$package" 'select(.reason == "compiler-artifact" and .target.name == $package) | .executable // empty')"
  app="$target/$bundle"
  iconset="$target/AppIcon.iconset"
  installed="$HOME/Applications/$bundle"
  plist="$app/Contents/Info.plist"
  rm -rf "$app" "$iconset"
  mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" "$iconset"
  install -m 755 "$executable" "$app/Contents/MacOS/"
  cp "$resources/Info.plist" "$plist"
  plutil -insert CFBundleShortVersionString -string "$version" "$plist"
  plutil -insert CFBundleVersion -string "$version" "$plist"
  cp "$resources/Credits.rtf" "$app/Contents/Resources/"
  for size in 16 32 128 256 512; do
    sips -z "$size" "$size" "$resources/icon.png" --out "$iconset/icon_${size}x${size}.png" >/dev/null
    sips -z "$((size * 2))" "$((size * 2))" "$resources/icon.png" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
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
