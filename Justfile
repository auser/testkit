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
    mysql -h mysql -u root -p'root' -e "SHOW DATABASES" | grep testkit_ || echo "No testkit databases found"

mysql-clean:
    mysql -h mysql -u root -p'root' -e "SHOW DATABASES" | grep testkit_ > /tmp/dbs.txt || echo "No testkit databases to clean"
    [ -s /tmp/dbs.txt ] && cat /tmp/dbs.txt | awk '{print "DROP DATABASE `" $$1 "`;"}' | mysql -h mysql -u root && echo "Cleaned testkit databases"
    rm -f /tmp/dbs.txt
