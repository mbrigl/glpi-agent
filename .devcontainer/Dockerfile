FROM debian:12-slim

ENV DEBIAN_FRONTEND=noninteractive
ENV GLPI_SERVER="http://glpi/front/inventory.php"
ENV GLPI_TAG="docker-agent"

RUN apt-get update && apt-get install -y \
    curl \
    ca-certificates \
    perl \
    lsb-release \
    dmidecode \
    pciutils \
    usbutils \
    net-tools \
    iproute2 \
    procps \
    && rm -rf /var/lib/apt/lists/*

# Install the GLPI Agent
# Example: provide the package or installer in the build context
RUN curl -L https://github.com/glpi-project/glpi-agent/releases/download/1.17/glpi-agent-1.17-linux-installer.pl -o /tmp/glpi-agent-installer.pl \
    && apt-get update \
    && perl /tmp/glpi-agent-installer.pl --install \
    && rm -f /tmp/glpi-agent-installer.pl \
    && rm -rf /var/lib/apt/lists/*

# Example configuration
RUN <<EOF cat > /etc/glpi-agent/agent.cfg
server = ${GLPI_SERVER}
tag = ${GLPI_TAG}
logger = stderr
EOF


# CMD ["glpi-agent", "--no-fork", "--debug"]
CMD ["sh", "-c", "while true; do glpi-agent --no-fork --debug; sleep 3600; done"]
