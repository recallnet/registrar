# Makefile for faucet Docker container

# Variables
IMAGE_NAME := faucet
CONTAINER_NAME := faucet-container
DOCKER_FILE := Dockerfile
PORT := 8080

# Phony targets
.PHONY: build run stop clean all

# Default target
all: build run

# Build the Docker image
build:
	@echo "Building Docker image..."
	docker build -t $(IMAGE_NAME) -f $(DOCKER_FILE) .

# Run the Docker container
run:
	@echo "Running Docker container..."
	docker run -d --name $(CONTAINER_NAME) -p $(PORT):$(PORT) \
		-e PRIVATE_KEY=$(PRIVATE_KEY) \
		-e TOKEN_ADDRESS=$(TOKEN_ADDRESS) \
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