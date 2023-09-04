# icmp-ping-rust

Concurrent ICMP Echo implementation in Rust.

This project is a Rust implementation of ICMP Echo, commonly known as ping.

It allows for concurrent pinging using `tokio`, `pnet` and `socket2` creates.



# Build Prerequisites

## `cargo-make`

The project incorporates several build subcommands: `install_dependencies`, `build`, `clean`, `format`, `clippy`, and `setcap`.

To facilitate easy building of the binary in a unified manner, the [cargo-make](https://sagiegurari.github.io/cargo-make/) task runner is being used.

Install `cargo-make` using the following command:

> `cargo install --force cargo-make`

Displays all available commands for the project:

> `cargo make --list-all-steps`


# Build Commands

## Install dependencies

A required one-time pre-build step.

To install dependencies run the following command:

> `cargo make install_dependencies`

## Build Binary Only

### Build in Debug Mode
Use the following command to build in debug mode:

> `cargo make build`

or 

> `cargo make --profile=development build`

### Build in Release Mode
Use the following command to build in release mode:

> `cargo make --profile=production build`


## Build Binary and Update Privileges

A binary needs to create a RAW socket in order to access the IP header bytes.

To allow normal (non-root) users to run the binary, the `cap_net_raw` privilege should be granted.

### Step 1: Build Binary
Use the following command to build the binary:

> `cargo make build`

### Step 2: Grant Privilege

Use the following command to grant the necessary privilege:

> `cargo make setcap`

<i>The task will run the `setcap` command with sudo, requiring a user password.</i>


# Testing

Run unit tests:

> `cargo test`


# Usage

## Run

Input will be passed in as CSV format in the first argument to the executable, e.g.

```
./icmp-ping-rs 8.8.8.8,6,500
```

Where:
- The first column is the IPv4 address; 
- The second column is the number of requests to send;
- The third column is the interval in milliseconds between sent requests.

## Run with Logging

The project using `env_logger` crate for different levels of logging.

Possible log levels are:
- `error`
- `warn`
- `info`
- `debug`
- `trace`

Use the following command to specify the log level:

```
RUST_LOG=info ./icmp-ping-rs 8.8.8.8,6,500
```


# Debugging

The project using [tokio-console](https://docs.rs/tokio-console/latest/tokio_console/) for debugging and profiling of asynchronous functions.

---

To debug timeouts, emulation of delays for sending and receiving ICMP packets has been added. The emulation is enabled through the activation of the ENABLE_EMULATE_DELAY [feature](https://doc.rust-lang.org/cargo/reference/features.html).

Use the following command to activate delay emulation:

```
cargo run --features ENABLE_EMULATE_DELAY -- 8.8.8.8,6,500
```
