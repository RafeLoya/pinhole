# Uncomment to test local container
COMPOSE_DEV := .devcontainer/local.docker-compose.yml
CONTAINER_DEV:= rust-server-dev
BINARY_DEV := /app/target/debug/server

COMPOSE_PROD := .devcontainer/ci.docker-compose.yml
CONTAINER_PROD := rust-server-prod

PROD_IP := 35.226.114.166

help: ## Help function
	@echo "Usage: make [target]"
	@echo ""
	@echo "tl;dr:"
	@echo "	 1. make up"
	@echo "	 2. make start"
	@echo "	 3. make down"
	@echo ""
	@echo "Container Management:"
	@grep -E '^\S+: .*## \[ContMgmt\]' $(MAKEFILE_LIST) | sed 's/: .*## \[ContMgmt\] /\t/' | expand -t 30
	@echo ""
	@echo "Project Management:"
	@grep -E '^\S+: .*## \[ProjMgmt\]' $(MAKEFILE_LIST) | sed 's/: .*## \[ProjMgmt\] /\t/' | expand -t 30
	@echo ""
	@echo "Connectivity Testing:"
	@grep -E '^\S+: .*## \[Test\]' $(MAKEFILE_LIST) | sed 's/: .*## \[Test\] /\t/' | expand -t 30
	@echo ""
	@echo "GCP:"
	@grep -E '^\S+: .*## \[GCP\]' $(MAKEFILE_LIST) | sed 's/: .*## \[GCP\] /\t/' | expand -t 30
.PHONY: help

#
# Container Management
#
.PHONY: up, down, enter, clean

up: ## [ContMgmt] Build server container (does not run binary)
	docker compose -f ${COMPOSE_DEV} up server --build -d

down: ## [ContMgmt] Tear down server container
	docker compose -f ${COMPOSE_DEV} down server -v

enter: ## [ContMgmt] Enter server container
	docker exec -it ${CONTAINER_DEV} bash

clean: ## [ContMgmt] Clean up artifacts
	rm -rf target logs
	docker image prune -af
	@echo ""
	docker system prune -f

#
# Project Management
#
.PHONY: start, rebuild

start: ## [ProjMgmt] Run the binary inside the container
	docker exec ${CONTAINER_DEV} ${BINARY_DEV}

rebuild: ## [ProjMgmt] Rebuild the binary inside the container
	docker exec ${CONTAINER_DEV} cargo build --bin server

#
# Connectivity Testing
#
.PHONY: test-tcp, test-udp, view-logs

test-tcp: ## [Test] Test container TCP connectivity
	nc -v 127.0.0.1 8080

test-udp: ## [Test] Test container UDP connectivity
	echo "Hello UDP Server" | nc -u -v 127.0.0.1 443

view-logs: ## [Test] View docker logs in real time
	docker exec -it ${CONTAINER_DEV} tail -f debug.log

test-tcp-prod: ## [Test] Test GCP container TCP connectivity
	nc -v ${PROD_IP} 8080

test-udp-prod: ## [Test] Test GCP container UDP connectivity
	echo "Hello UDP Server" | nc -u -v ${PROD_IP} 443

#
# CI Container Stuff
#
.PHONY: up-prod, down-prod, enter-prod
up-prod:
	docker compose -f ${COMPOSE_PROD} up server --build -d

down-prod:
	docker compose -f ${COMPOSE_PROD} down server -v

enter-prod:
	docker exec -it ${CONTAINER_PROD} bash

.PHONY: view-stdout-prod, view-logs-prod, test-dev, test-prod
view-stdout-prod: ## [GCP] View server stdout from production VM
	docker logs ${CONTAINER_PROD} -f

view-logs-prod: ## [GCP] View server debug files live from production VM
	docker exec -it ${CONTAINER_PROD} tail -f debug.log

tshark-prod: ## [GCP] View wireshark logs from production VM
tshark-prod: ## [GCP]   Note: run inside the VM but not inside the container.
tshark-prod: ## [GCP]   there is a tshark-command.sh script in the home directory with this command.
	sudo tshark -i any -Y 'ip.addr == 127.0.0.1'
.PHONY: tshark-prod

test-dev:
	CONTAINER_NAME=${CONTAINER_DEV} bash -c scripts/test-suite.sh

test-prod:
	CONTAINER_NAME=${CONTAINER_PROD} bash -c scripts/test-suite.sh
