set dotenv-load := true

default:
    @just --list --justfile {{justfile()}}

# Build the base devcontainer
devcontainer-build:
    docker build -t auser/routerfe -f .devcontainer/docker/Dockerfile.base .

# Install system packages
install-system:
    @echo "Installing system packages"
    @echo "Installing pipx and pre-commit"
    sudo apt update && sudo apt install -y pipx pre-commit

    # Uninstall git hooks
    @echo "Uninstalling git hooks"
    git config --unset-all core.hooksPath

# Install required tools
install-required: install-system
    @echo "Installing tools"
    @echo "Installing Rust nightly toolchain"
    rustup toolchain install nightly

    @echo "Installing nextest"
    cargo install cargo-nextest

    @echo "Install components"
    rustup component add rustfmt rust-analyzer

    @echo "Installing mdbook"
    cargo install mdbook

    @echo "Install sqlx"
    cargo install sqlx-cli

# Install required and recommended tools
install-recommended: install-required
    @echo "Installing recommended tools..."
    @echo "Installing git hooks"
    pre-commit --version || pipx install pre-commit
    pre-commit install || echo "failed to install git hooks!" 1>&2

# Run migrations
run-migrations:
    sqlx migrate run

# Reset database
reset-db:
    sqlx database reset

# List the test postgres databases
postgres-list:
    psql postgresql://postgres:postgres@postgres:5432/ -c "\l" | grep 'testkit_' | awk '{print $1}'

# Cleanup the test postgres databases
postgres-clean:
    psql postgresql://postgres:postgres@postgres:5432/ -c "\l" | grep 'testkit_' | awk '{print $1}' | xargs -I '{}' psql postgresql://postgres:postgres@postgres:5432/ -c "DROP DATABASE \"{}\""

# Test all features
test-all:
    @echo "Running tests with all features"
    cargo test --all-features

# Test default features only
test-default:
    @echo "Running tests with default features"
    cargo test

# Test PostgreSQL backend
test-postgres:
    @echo "Running tests with postgres feature"
    cargo test --features "postgres"

# Test SQLx PostgreSQL backend
test-sqlx-postgres:
    @echo "Running tests with sqlx-postgres and sqlx-backend features"
    cargo test --features "sqlx-postgres sqlx-backend"

# Test SQLite backend 
test-sqlite:
    @echo "Running tests with sqlite feature"
    cargo test --features "sqlx-sqlite sqlx-backend"

# Test MySQL backend
test-mysql:
    @echo "Running tests with mysql feature"
    cargo test --features "mysql"

# Test all backends individually and together
test-backends: test-postgres test-sqlx-postgres test-sqlite test-mysql test-all
    @echo "All backend tests completed"

# Run SQLite examples
run-sqlite-examples:
    @echo "Running SQLite examples"
    cargo run --features "sqlx-sqlite sqlx-backend" --example simple_sqlite_test
    cargo run --features "sqlx-sqlite sqlx-backend" --example macro_sqlite_test

# Run PostgreSQL examples
run-postgres-examples:
    @echo "Running PostgreSQL examples"
    cargo run --features "postgres" --example simple_postgres_test
    cargo run --features "postgres" --example macro_postgres_test
    cargo run --features "sqlx-postgres sqlx-backend" --example sqlx_postgres_usage

