# Docker Compose Workflow: Development vs Production

Let me walk you through complete workflows for both development and production environments using Docker Compose.

## Development Workflow

### Commands Summary
```bash
# Initial setup
docker compose -f docker/dev.docker-compose.yml build

# Start services with latest changes
docker compose -f docker/dev.docker-compose.yml up --build

# Run in background
docker compose -f docker/dev.docker-compose.yml up -d --build

# View logs when running in background
docker compose -f docker/dev.docker-compose.yml logs -f

# Execute commands in a running container
docker compose -f docker/dev.docker-compose.yml exec server bash

# Run a temporary container
docker compose -f docker/dev.docker-compose.yml run --rm server bash

# Stop all services
docker compose -f docker/dev.docker-compose.yml down

# Stop and remove volumes (careful!)
docker compose -f docker/dev.docker-compose.yml down -v
```

### Step-by-Step Development Process

1. **Initial Build**:
   ```bash
   docker compose -f docker/dev.docker-compose.yml build
   ```
   This creates images for all services defined in your compose file.

2. **Start Services**:
   ```bash
   docker compose -f docker/dev.docker-compose.yml up --build
   ```
   This starts all services and rebuilds if necessary. The terminal will show logs from all containers.

3. **Development Work**:
   As you make code changes, depending on your setup:
   - Some changes may auto-reload (if volume mounts are configured)
   - For changes requiring rebuilds, press Ctrl+C and run `up --build` again

4. **Run Commands in Services**:
   ```bash
   docker compose -f docker/dev.docker-compose.yml exec server bash
   ```
   This opens a shell in your running server container for debugging or testing.

5. **Teardown**:
   ```bash
   docker compose -f docker/dev.docker-compose.yml down
   ```
   This stops and removes containers and networks, but preserves volumes for data persistence.

## Production Workflow

### Commands Summary
```bash
# Build with production optimizations
docker compose -f docker/prod.docker-compose.yml build

# Tag images for registry
docker tag myproject_server:latest registry.example.com/myproject_server:v1.2.3

# Push to registry
docker push registry.example.com/myproject_server:v1.2.3

# Deploy with specific image versions
IMAGE_TAG=v1.2.3 docker compose -f docker/prod.docker-compose.yml up -d

# Check service health
docker compose -f docker/prod.docker-compose.yml ps

# View logs
docker compose -f docker/prod.docker-compose.yml logs -f

# Scale services
docker compose -f docker/prod.docker-compose.yml up -d --scale worker=3

# Rolling updates
IMAGE_TAG=v1.2.4 docker compose -f docker/prod.docker-compose.yml up -d

# Teardown
docker compose -f docker/prod.docker-compose.yml down
```

### Step-by-Step Production Process

1. **Build Production Images**:
   ```bash
   docker compose -f docker/prod.docker-compose.yml build
   ```
   Creates optimized production images with minimal dependencies.

2. **Tag Images**:
   ```bash
   docker tag myproject_server:latest registry.example.com/myproject_server:v1.2.3
   ```
   Version your images with semantic versioning for tracking deployments.

3. **Push to Registry**:
   ```bash
   docker push registry.example.com/myproject_server:v1.2.3
   ```
   Upload images to a registry accessible by production servers.

4. **Deploy Services**:
   ```bash
   IMAGE_TAG=v1.2.3 docker compose -f docker/prod.docker-compose.yml up -d
   ```
   Start services in detached mode using environment variables to specify versions.

5. **Verify Deployment**:
   ```bash
   docker compose -f docker/prod.docker-compose.yml ps
   ```
   Check that all services are up and healthy.

6. **Monitoring**:
   ```bash
   docker compose -f docker/prod.docker-compose.yml logs -f
   ```
   Watch logs for issues during startup.

7. **Updates/Rollouts**:
   ```bash
   IMAGE_TAG=v1.2.4 docker compose -f docker/prod.docker-compose.yml up -d
   ```
   Deploy new versions with zero downtime if configured properly.

8. **Teardown (if needed)**:
   ```bash
   docker compose -f docker/prod.docker-compose.yml down
   ```
   Stop services if maintenance is required.

The key differences in production are:
- Using versioned images rather than building on-the-fly
- Running in detached mode always
- Using environment variables to control deployment versions
- More careful handling of volumes and state
- Often integrated with orchestration systems for true production deployments
