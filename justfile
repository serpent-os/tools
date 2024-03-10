root-dir := justfile_directory()

[private]
help:
  @just --list -u

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
