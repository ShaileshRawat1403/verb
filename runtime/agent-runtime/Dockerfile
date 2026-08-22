FROM --platform=linux/arm64 debian:bookworm-slim

ARG NODE_VERSION=24.18.0
ARG CLAUDE_VERSION=2.1.233
ARG OPENCODE_VERSION=1.18.18
ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update \
    && apt-get install -y --no-install-recommends bash ca-certificates curl git gzip libc6 libstdc++6 openssh-client tar tzdata xz-utils \
    && rm -rf /var/lib/apt/lists/* \
    && arch="$(dpkg --print-architecture)" \
    && test "$arch" = arm64 \
    && curl -fsSL "https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-linux-arm64.tar.xz" -o /tmp/node.tar.xz \
    && tar -xJf /tmp/node.tar.xz -C /opt \
    && ln -s "/opt/node-v${NODE_VERSION}-linux-arm64/bin/node" /usr/local/bin/node \
    && ln -s "/opt/node-v${NODE_VERSION}-linux-arm64/bin/npm" /usr/local/bin/npm \
    && ln -s "/opt/node-v${NODE_VERSION}-linux-arm64/bin/npx" /usr/local/bin/npx \
    && npm config set prefix /usr/local \
    && npm install --global --foreground-scripts --ignore-scripts=false --allow-scripts=@anthropic-ai/claude-code,opencode-ai "@anthropic-ai/claude-code@${CLAUDE_VERSION}" "opencode-ai@${OPENCODE_VERSION}" \
    && node --version && npm --version && claude --version && opencode --version \
    && rm -rf /tmp/node.tar.xz /root/.npm

RUN useradd --create-home --uid 10000 verb \
    && mkdir -p /workspace /home/verb/.claude /home/verb/.local/share/opencode \
    && chown -R verb:verb /home/verb /workspace
USER verb
WORKDIR /workspace
ENV HOME=/home/verb PATH=/usr/local/bin:/usr/bin:/bin TERM=xterm-256color LANG=C.UTF-8
