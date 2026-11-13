# prettier-ignore

set default-list

# Format the source code.
format:
    cargo fmt

# Check the source code is formatted, without rewriting it. Mirrors CI.
format-check:
    cargo fmt --check

# Build Bank Holidays.app and install it in /Applications.
install-app:
    #!/usr/bin/env bash
    set -euo pipefail
    package=bank-holidays-app
    bundle='Bank Holidays.app'
    sources=crates/app/bundle
    metadata="$(cargo metadata --no-deps --format-version 1)"
    version="$(jq -r --arg package "$package" '.packages[] | select(.name == $package) | .version' <<<"$metadata")"
    if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
      echo "error: $package's version is $version; an app bundle needs a plain X.Y.Z" >&2
      exit 1
    fi
    target="$(jq -r .target_directory <<<"$metadata")"
    deployment_target="$(plutil -extract LSMinimumSystemVersion raw "$sources/Info.plist")"
    executable="$(MACOSX_DEPLOYMENT_TARGET="$deployment_target" \
      cargo build --release -p "$package" --message-format json-render-diagnostics \
      | jq -r --arg package "$package" 'select(.reason == "compiler-artifact" and .target.name == $package) | .executable // empty')"
    if [[ -z "$executable" ]]; then
      echo "error: cargo built no $package executable" >&2
      exit 1
    fi
    app="$target/$bundle"
    iconset="$target/AppIcon.iconset"
    installed="/Applications/$bundle"
    plist="$app/Contents/Info.plist"
    rm -rf "$app" "$iconset"
    mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" "$iconset"
    install -m 755 "$executable" "$app/Contents/MacOS/"
    cp "$sources/Info.plist" "$plist"
    plutil -insert CFBundleShortVersionString -string "$version" "$plist"
    plutil -insert CFBundleVersion -string "$version" "$plist"
    cp "$sources/Credits.rtf" "$app/Contents/Resources/"
    for size in 16 32 128 256 512; do
      sips -z "$size" "$size" "$sources/icon.png" --out "$iconset/icon_${size}x${size}.png" >/dev/null
      sips -z "$((size * 2))" "$((size * 2))" "$sources/icon.png" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
    done
    iconutil -c icns "$iconset" -o "$app/Contents/Resources/AppIcon.icns"
    codesign --force --sign - "$app"
    rm -rf "$installed"
    ditto "$app" "$installed"
    echo "Installed $installed $version."

# Lint the source code.
lint:
    cargo clippy --all-targets -- -D warnings

# Refresh the gov.uk feed snapshot built into the library, and its capture date.
snapshot:
    #!/usr/bin/env bash
    set -euo pipefail
    data=crates/lib/data
    download="$(mktemp "$data/bank-holidays.json.XXXXXX")"
    trap 'rm -f "$download"' EXIT
    curl --fail --silent --show-error --max-time 30 --output "$download" https://www.gov.uk/bank-holidays.json
    cargo run --quiet --package bank-holidays --example check_feed -- "$download"
    chmod 644 "$download"
    mv "$download" "$data/bank-holidays.json"
    date -u +%F > "$data/captured"
    echo "Snapshot captured $(cat "$data/captured")."

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
