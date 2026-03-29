# syntax=docker/dockerfile:1
# Build: docker build --target relay -t tunnelion-js-relay .
#        docker build --target client -t tunnelion-js-client .
# Runtime: set TUNNELION_SECRET; mount relay.json / tunnels.json or override CMD.
# Relay tunnel listen addresses in JSON should use 0.0.0.0 when publishing ports from the container.

FROM node:22-alpine AS builder
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci
COPY tsconfig.json ./
COPY src ./src
RUN npm run build

FROM node:22-alpine AS runtime-base
RUN apk add --no-cache dumb-init
WORKDIR /app
ENV NODE_ENV=production
COPY package.json package-lock.json ./
RUN npm ci --omit=dev && npm cache clean --force
COPY --from=builder /app/dist ./dist
RUN chown -R node:node /app
USER node
ENTRYPOINT ["/usr/bin/dumb-init", "--"]
LABEL org.opencontainers.image.title="tunnelion-js" \
  org.opencontainers.image.description="JSON config, WebSocket auth, binary tunnel frames"

FROM runtime-base AS relay
EXPOSE 9780
CMD ["node", "dist/bin/relay.js", "relay.json"]

FROM runtime-base AS client
CMD ["node", "dist/bin/client.js", "tunnels.json"]
