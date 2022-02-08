TJ in Rust
==========

TJ Counter (Server RUST+Hyper/Tokio+SSE + Client HTML+JS)

Run this code like:
 > cargo run

 Then open up your browser to http://localhost:8182

Create Docker image:
 > docker build --no-cache -t kbalashoff/tjcounter-rust .

Get image from Docker repo:
 > docker pull kbalashoff/tjcounter-rust

Run Docker container:
 > docker run -d -p 8182:8182 kbalashoff/tjcounter-rust

