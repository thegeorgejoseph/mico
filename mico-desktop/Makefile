.PHONY: install dev test build prod-local-build prod-local-run release-stage clean

install:
	npm install --package-lock-only
	cd app && npm install

dev:
	cd app && npm run dev

test:
	cd backend && go test ./...
	cd app && npm run build

build:
	cd backend && go build -o ./bin/mico-desktop ./cmd/mico-desktop
	cd app && npm run build

prod-local-build:
	./scripts/build-local-prod.sh

prod-local-run:
	./scripts/run-local-prod.sh

release-stage:
	@if [ -z "$(VERSION)" ]; then \
		echo "usage: make release-stage VERSION=x.y.z" >&2; \
		exit 1; \
	fi
	./scripts/stage-release.sh "$(VERSION)"

clean:
	rm -rf app/dist app/node_modules backend/bin dist
