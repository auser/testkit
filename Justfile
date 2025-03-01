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

mysql-list:
    mysql -h mysql -u root -e "SHOW DATABASES" | grep testkit_ || echo "No testkit databases found"

mysql-clean:
    mysql -h mysql -u root -e "SHOW DATABASES" | grep testkit_ > /tmp/dbs.txt || echo "No testkit databases to clean"
    [ -s /tmp/dbs.txt ] && cat /tmp/dbs.txt | awk '{print "DROP DATABASE `" $$1 "`;"}' | mysql -h mysql -u root && echo "Cleaned testkit databases"
    rm -f /tmp/dbs.txt

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
    cargo test --features "sqlite"

# Test SQLx SQLite backend
test-sqlx-sqlite:
    @echo "Running tests with sqlx-sqlite and sqlx-backend features"
    cargo test --features "sqlx-sqlite sqlx-backend"

# Test MySQL backend
test-mysql:
    @echo "Running tests with mysql feature"
    cargo test --features "mysql"

# Test SQLx MySQL backend
test-sqlx-mysql:
    @echo "Running tests with sqlx-mysql and sqlx-backend features"
    cargo test --features "sqlx-mysql sqlx-backend"

# Test all SQLx backends
test-all-sqlx:
    @echo "Running tests with all SQLx backends"
    cargo test --features "sqlx-mysql sqlx-postgres sqlx-sqlite sqlx-backend"

# Test all PostgreSQL backends (native and SQLx)
test-all-postgres:
    @echo "Running tests with all PostgreSQL backends"
    cargo test --features "postgres sqlx-postgres sqlx-backend"

# Test all MySQL backends (native and SQLx)
test-all-mysql:
    @echo "Running tests with all MySQL backends"
    cargo test --features "mysql sqlx-mysql sqlx-backend" 

# Test all SQLite backends (native and SQLx)
test-all-sqlite:
    @echo "Running tests with all SQLite backends"
    cargo test --features "sqlite sqlx-sqlite sqlx-backend"

# Test all backends individually and together
test-backends: test-postgres test-sqlx-postgres test-sqlite test-sqlx-sqlite test-mysql test-sqlx-mysql test-all
    @echo "All backend tests completed"



# Run PostgreSQL feature tests
test-postgres-features:
    @echo "Running PostgreSQL feature tests"
    cargo test --features "postgres" --test postgres_features

# Run MySQL feature tests
test-mysql-features:
    @echo "Running MySQL feature tests"
    cargo test --features "mysql" --test mysql_features

# Run SQLite feature tests
test-sqlite-features:
    @echo "Running SQLite feature tests"
    cargo test --features "sqlite" --test sqlite_features

# Run SQLx PostgreSQL feature tests
test-sqlx-postgres-features:
    @echo "Running SQLx PostgreSQL feature tests"
    cargo test --features "sqlx-postgres sqlx-backend" --test sqlx_postgres_features

# Run SQLx MySQL feature tests
test-sqlx-mysql-features:
    @echo "Running SQLx MySQL feature tests"
    cargo test --features "sqlx-mysql sqlx-backend" --test sqlx_mysql_features

# Run SQLx SQLite feature tests
test-sqlx-sqlite-features:
    @echo "Running SQLx SQLite feature tests"
    cargo test --features "sqlx-sqlite sqlx-backend" --test sqlx_sqlite_features

# Run all feature-specific tests
test-all-features: test-postgres-features test-mysql-features test-sqlite-features test-sqlx-postgres-features test-sqlx-mysql-features test-sqlx-sqlite-features
    @echo "All feature-specific tests completed"

# Run concurrent operations tests
test-concurrent:
    @echo "Running concurrent database operations tests"
    cargo test --features "postgres" --test concurrent_db_operations

