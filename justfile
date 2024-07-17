# The default task is to build moss
default: moss

root-dir := justfile_directory()
build-mode := env_var_or_default("MODE", "onboarding")
# Keep it simple for now and make installs user-local
xdg-data-home := "$HOME/.local/share"
xdg-bin-home := "$HOME/.local/bin"

[private]
help:
  @just --list -u

[private]
build package:
  cargo build --verbose --profile {{build-mode}} -p {{package}}

# Compile boulder
boulder: (build "boulder")

# Compile moss
moss: (build "moss")

# Onboarding replacement
get-started: (build "boulder") (build "moss")
  @echo ""
  @echo "Installing boulder and moss to {{xdg-bin-home}}/ ..."
  mkdir -pv "{{xdg-bin-home}}/"
  cp "{{root-dir}}/target/{{build-mode}}"/{boulder,moss} "{{xdg-bin-home}}/"
  rm -rf "{{xdg-data-home}}/boulder"
  mkdir -pv "{{xdg-data-home}}/boulder/"
  cp -R "{{root-dir}}/boulder/data"/* "{{xdg-data-home}}/boulder/"
  @echo ""
  @echo "Listing installed files..."
  ls -hlF "{{xdg-bin-home}}"/{boulder,moss} "{{xdg-data-home}}/boulder"
  @echo ""
  @echo "Checking the system path to boulder and moss executables:"
  command -v boulder
  command -v moss
  @echo ""
  @echo "Done."
  @echo ""
  @echo "The Serpent OS documentation lives at https://docs.serpentos.com"
  @echo ""

# Fix code issues
fix:
  @echo "Applying clippy fixes..."
  cargo clippy --fix --allow-dirty --allow-staged --workspace -- --no-deps
  @echo "Applying cargo fmt"
  cargo fmt --all
  @echo "Fixing typos"
  typos -w

# Run lints
lint:
  @echo "Running clippy..."
  cargo clippy --workspace -- --no-deps
  @echo "Running cargo fmt.."
  cargo fmt --all -- --check
  @echo "Checking for typos..."
  typos

# Run tests
test: lint
  @echo "Running tests in all packages"
  cargo test --all

# Run all DB migrations
migrate: (diesel "meta" "migration run") (diesel "layout" "migration run") (diesel "state" "migration run")  
# Rerun all DB migrations
migrate-redo: (diesel "meta" "migration redo") (diesel "layout" "migration redo") (diesel "state" "migration redo")  

[private]
diesel db +ARGS:
  diesel \
    --config-file {{root-dir}}/moss/src/db/{{db}}/diesel.toml \
    --database-url sqlite://{{root-dir}}/moss/src/db/{{db}}/test.db \
    {{ARGS}}
