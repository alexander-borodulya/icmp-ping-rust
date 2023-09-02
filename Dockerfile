FROM rust

# Install helpful utilities
RUN apt-get update && apt-get install -y \
  iputils-ping \
  vim \
  traceroute \
  nmap

ENTRYPOINT ["/bin/bash"]
