# Makefile for registrar Docker container

# Variables
IMAGE_NAME := registrar
CONTAINER_NAME := registrar
DOCKER_FILE := Dockerfile

# Phony targets
.PHONY: all lint check-fmt check-clippy build run run-local stop clean attach

# Default target
all: lint build run

lint: \
	check-fmt \
	check-clippy

check-fmt:
	cargo fmt --all --check

check-clippy:
	cargo clippy --no-deps --tests -- -D clippy::all

# Build the Docker image
build:
	@echo "Building Docker image..."
	docker build -t $(IMAGE_NAME) -f $(DOCKER_FILE) .

# Run the Docker container
run:
	@echo "Running Docker container..."
	docker run -d \
		--name $(CONTAINER_NAME) \
		-p $(LISTEN_HOST):$(LISTEN_PORT):$(LISTEN_PORT) \
		-e PRIVATE_KEY=$(PRIVATE_KEY) \
		-e FAUCET_ADDRESS=$(FAUCET_ADDRESS) \
		-e EVM_RPC_URL=$(EVM_RPC_URL) \
		-e LISTEN_HOST=$(LISTEN_HOST) \
		-e LISTEN_PORT=$(LISTEN_PORT) \
		-e VERBOSITY=$(VERBOSITY) \
		$(IMAGE_NAME)

# Run the Docker container
run-local:
	@echo "Running Docker container on host network..."
	docker run -d \
		--network host \
		--name $(CONTAINER_NAME) \
		-e PRIVATE_KEY=$(PRIVATE_KEY) \
		-e FAUCET_ADDRESS=$(FAUCET_ADDRESS) \
		-e EVM_RPC_URL=$(EVM_RPC_URL) \
		-e LISTEN_HOST=$(LISTEN_HOST) \
		-e LISTEN_PORT=$(LISTEN_PORT) \
		-e VERBOSITY=$(VERBOSITY) \
		$(IMAGE_NAME)

# Stop and remove the Docker container
stop:
	@echo "Stopping Docker container..."
	docker stop $(CONTAINER_NAME)
	docker rm $(CONTAINER_NAME)

# Clean up Docker images
clean: stop
	@echo "Removing Docker image..."
	docker rmi $(IMAGE_NAME)

# Attach to the running container
attach:
	@echo "Attaching to Docker container..."
	docker exec -it $(CONTAINER_NAME) /bin/sh