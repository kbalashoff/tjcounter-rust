# Use official Rust image: check your with rustc --version
FROM rust:1.58.1

# Copy files to image
COPY ./ ./

# Compile
RUN cargo build --release

RUN cp ./target/release/tjcounter-rust ./tjcounter-rust

RUN cargo clean

EXPOSE 8182

# Run
CMD ["./tjcounter-rust"]