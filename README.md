# minecraft-server-proxy

A simple Minecraft server proxy that allows you to connect to multiple servers using a single IP address.

## Building

```bash
git clone https://github.com/0x7d8/minecraft-server-proxy.git
cd minecraft-server-proxy

# Build the project
cargo build --release

# or use the pre-built binary
# download the latest release from the releases page
# extract the downloaded archive
# add the binary to your PATH
```

## Running

```bash
# Copy the example configuration file
cp reroutes.json.example reroutes.json

# Run the proxy
./target/release/minecraft-server-proxy
```
