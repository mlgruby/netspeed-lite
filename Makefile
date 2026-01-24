# NetSpeed-Lite Makefile
# 
# Optional tools (install with cargo install):
#   - cargo-audit: For 'make audit' (security auditing)
#   - cargo-watch: For 'make watch' (auto-rebuild on changes)

.PHONY: help build test check fmt clippy audit clean run docker-build docker-run docker-stop docker-clean all

# Default target
.DEFAULT_GOAL := help

# Variables
BINARY_NAME := netspeed-lite
DOCKER_IMAGE := netspeed-lite
DOCKER_TAG := latest

help: ## Show this help message
	@echo 'Usage: make [target]'
	@echo ''
	@echo 'Available targets:'
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "  %-20s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

build: ## Build release binary
	cargo build --release

build-dev: ## Build debug binary
	cargo build

test: ## Run all tests
	cargo test --all-features

check: ## Check code compiles
	cargo check --all-features

fmt: ## Format code
	cargo fmt --all

fmt-check: ## Check code formatting
	cargo fmt --all -- --check

clippy: ## Run clippy linter
	cargo clippy --all-features -- -D warnings

audit: ## Run security audit
	cargo audit

clean: ## Clean build artifacts
	cargo clean
	rm -rf target/

env-check: ## Check if .env file exists
	@if [ ! -f .env ]; then \
		echo "Error: .env file not found. Copy .env.example to .env and configure it."; \
		echo "  cp .env.example .env"; \
		exit 1; \
	fi

run: env-check ## Run the monitor (requires .env file)
	set -a && . ./.env && set +a && cargo run --release

run-dev: env-check ## Run the monitor in debug mode
	set -a && . ./.env && set +a && cargo run

watch: ## Watch for changes and rebuild
	cargo watch -x run

# Docker targets
docker-build: ## Build Docker image
	docker build -t $(DOCKER_IMAGE):$(DOCKER_TAG) .

docker-run: env-check ## Run Docker container
	docker run -d \
		--name $(BINARY_NAME) \
		-p 9109:9109 \
		--env-file .env \
		$(DOCKER_IMAGE):$(DOCKER_TAG)

docker-stop: ## Stop Docker container
	docker stop $(BINARY_NAME) || true
	docker rm $(BINARY_NAME) || true

docker-logs: ## Show Docker container logs
	docker logs -f $(BINARY_NAME)

docker-clean: docker-stop ## Clean Docker images
	docker rmi $(DOCKER_IMAGE):$(DOCKER_TAG) || true

docker-shell: ## Open shell in running container
	docker exec -it $(BINARY_NAME) /bin/sh

# Docker Compose targets
compose-up: ## Start services with docker-compose
	docker-compose up -d

compose-down: ## Stop services with docker-compose
	docker-compose down

compose-logs: ## Show docker-compose logs
	docker-compose logs -f

compose-restart: ## Restart docker-compose services
	docker-compose restart

# CI/CD simulation
ci: fmt-check clippy test build docker-build ## Run all CI checks locally

# Development workflow
dev: fmt clippy test ## Run development checks (format, lint, test)

all: clean ci ## Clean and run all checks

# Release preparation
release-check: ## Check if ready for release
	@echo "Checking version in Cargo.toml..."
	@grep '^version' Cargo.toml
	@echo ""
	@echo "Running all checks..."
	@make ci
	@echo ""
	@echo "✓ Ready for release!"

# API testing
test-api: ## Test API endpoints (requires running instance)
	@echo "Testing root endpoint..."
	@curl -f http://localhost:9109/ || echo "Root endpoint failed"
	@echo ""
	@echo "Testing metrics endpoint..."
	@curl -f http://localhost:9109/metrics || echo "Metrics endpoint failed"
	@echo ""
	@echo "Testing health check endpoint..."
	@curl -f http://localhost:9109/healthz || echo "Health check failed"

# Documentation
docs: ## Generate and open documentation
	cargo doc --no-deps --open

docs-build: ## Build documentation
	cargo doc --no-deps

# Quick test run with interval mode
test-run: env-check ## Run with 5-minute interval for testing
	set -a && . ./.env && set +a && \
	NETSPEED_SCHEDULE_MODE=interval \
	NETSPEED_INTERVAL_SECONDS=300 \
	cargo run
