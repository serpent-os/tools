# The default task is to build moss
default: moss

root-dir := justfile_directory()
build-mode := env_var_or_default("MODE", "packaging")

[private]
help:
  @just --list -u

[private]
build package:
  cargo build --profile {{build-mode}} -p {{package}}

# Compile boulder
boulder: (build "boulder")

# Compile moss
moss: (build "moss")

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
