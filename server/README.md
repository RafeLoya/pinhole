# Server

The server will be containerized and hosted on GCP in a Linux VM. Note that there are linux-specific dependencies so make sure you are testing in a container.

See [`.devcontainer/`](../.devcontainer/) for Dockerfiles and docker-compose yaml files related to ci and local builds.

Github actions are stored at [`.github/workflows/`](../.github/workflows/)

## Local Container For Testing

You should still be able to use RustRover's Remote Development feature.

To build a local container directly in the terminal, run `docker compose -f .devcontainer/local.docker-compose.yml up --build`. Both `server/` and `common/` are loaded as docker volumes, so any local changes will be reflected in your container.

If you make changes locally, you can either nuke the current container and rebuild, or recompile the binary inside of the container.
