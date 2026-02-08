// TJ Counter in RUST
use bytes::{BufMut, Bytes, BytesMut};

use futures::future::{loop_fn, ok, Either, Future, Loop};
use futures::sink::Sink;
use futures::sync::mpsc;
use futures::Stream;

use tokio_core::reactor::Timeout;

use hyper::header::{AccessControlAllowOrigin, Connection, ContentType};
use hyper::mime;
use hyper::server::{Http, Request, Response, Service};
use hyper::Chunk;
use hyper::{Get, StatusCode};

use chrono::{NaiveDateTime, Utc};
use sprintf::sprintf;

use std::str;

use std::io::Write;
use std::time::Duration;

// this fn replaces closures to avoid boxing in some cases
fn print_err<T: std::fmt::Debug>(t: T) {
    println!("{:?}", t);
}

struct EventService {
    tx_new: mpsc::Sender<mpsc::Sender<Result<Chunk, hyper::Error>>>,
}

impl Service for EventService {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Future = Box<dyn Future<Item = Response, Error = Self::Error>>;

    fn call(&self, req: Request) -> Self::Future {
        match (req.method(), req.path()) {
            (&Get, "/events") => {
                println!("request events");
                let (tx_msg, rx_msg) = mpsc::channel(10);
                Box::new(
                    self.tx_new
                        .clone()
                        .send(tx_msg)
                        .and_then(|_| {
                            Ok(Response::new()
                                .with_status(StatusCode::Ok)
                                .with_header(AccessControlAllowOrigin::Any)
                                .with_header(ContentType(mime::TEXT_EVENT_STREAM))
                                .with_header(Connection::keep_alive())
                                .with_body(rx_msg))
                        })
                        .or_else(|_| Ok(Response::new().with_status(StatusCode::NotAcceptable))),
                )
            }

            (&Get, "/") => {
                println!("request html");
                Box::new(ok(Response::new()
                    .with_status(StatusCode::Ok)
                    .with_body(HTML)))
            }

            (method, path) => {
                println!("invalid request method: {:?}, path: {:?}", method, path);
                Box::new(ok(Response::new().with_status(StatusCode::NotFound)))
            }
        }
    }
}

const MS_PER_SECOND: i64 = 1_000;
const MS_PER_MINUTE: i64 = 60 * MS_PER_SECOND;
const MS_PER_HOUR: i64 = 60 * MS_PER_MINUTE;
const MS_PER_DAY: i64 = 24 * MS_PER_HOUR;

fn calc_counter(freedom: &NaiveDateTime) -> String {
    format_counter(Utc::now().naive_utc(), *freedom)
}

fn format_counter(now: NaiveDateTime, target: NaiveDateTime) -> String {
    let diff_ms = target.signed_duration_since(now).num_milliseconds();
    let (direction, mut remaining_ms) = if diff_ms >= 0 {
        ("until", diff_ms)
    } else {
        ("since", -diff_ms)
    };

    let days = remaining_ms / MS_PER_DAY;
    remaining_ms -= days * MS_PER_DAY;
    let hours = remaining_ms / MS_PER_HOUR;
    remaining_ms -= hours * MS_PER_HOUR;
    let minutes = remaining_ms / MS_PER_MINUTE;
    remaining_ms -= minutes * MS_PER_MINUTE;
    let seconds = remaining_ms / MS_PER_SECOND;
    remaining_ms -= seconds * MS_PER_SECOND;
    let tenths = remaining_ms / 100;

    let counter = sprintf!(
        "%s %d days %02d:%02d:%02d,%d",
        direction,
        days,
        hours,
        minutes,
        seconds,
        tenths
    )
    .unwrap();
    counter
}

fn main() {
    let addr = "0.0.0.0:8182".parse().expect("addres parsing failed");

    let (tx_new, rx_new) = mpsc::channel(100);

    let server = Http::new()
        .bind(&addr, move || {
            Ok(EventService {
                tx_new: tx_new.clone(),
            })
        })
        .expect("unable to create server");
    let handle = server.handle();
    let handle2 = handle.clone();

    let event_delay = Duration::from_millis(100); //
    let _start_time = std::time::Instant::now();

    let fu_to = Timeout::new(event_delay, &handle)
        .unwrap()
        .map_err(print_err);
    let fu_rx = rx_new.into_future().map_err(print_err);
    let clients: Vec<mpsc::Sender<Result<Chunk, hyper::Error>>> = Vec::new();

    let freedom =
        NaiveDateTime::parse_from_str("2022-02-11 16:00:00", "%Y-%m-%d %H:%M:%S").unwrap();

    let broker = loop_fn(
        (fu_to, fu_rx, clients, 0),
        move |(fu_to, fu_rx, mut clients, event_counter)| {
            let handle = handle2.clone();
            fu_to
                .select2(fu_rx)
                .map_err(|_| ())
                .and_then(move |done| match done {
                    Either::A((_, fu_rx)) => Either::A({
                        //send messages
                        let mut buf = BytesMut::with_capacity(1024).writer();
                        let msg = calc_counter(&freedom);
                        //println!("msg {}", msg);
                        write!(buf, "event: uptime\ndata: {{\"time\": \"{}\"}}\n\n", msg)
                            .expect("msg write failed");
                        let msg: Bytes = buf.into_inner().freeze();
                        let tx_iter = clients
                            .into_iter()
                            .map(|tx| tx.send(Ok(Chunk::from(msg.clone().to_vec()))));
                        futures::stream::futures_unordered(tx_iter)
                            .map(Some)
                            .or_else(|e| {
                                println!("{:?} client removed", e);
                                Ok::<_, ()>(None)
                            })
                            .filter_map(|x| x)
                            .collect()
                            .and_then(move |clients| {
                                ok(Loop::Continue((
                                    Timeout::new(event_delay, &handle)
                                        .unwrap()
                                        .map_err(print_err),
                                    fu_rx,
                                    clients,
                                    event_counter + 1,
                                )))
                            })
                    }),

                    Either::B(((item, rx_new), fu_to)) => Either::B({
                        //register new client
                        match item {
                            Some(item) => {
                                clients.push(item);
                                println!("client {} registered", clients.len());
                            }
                            None => println!("keeper loop get None"),
                        }

                        ok(Loop::Continue((
                            fu_to,
                            rx_new.into_future().map_err(print_err),
                            clients,
                            event_counter,
                        )))
                    }),
                })
        },
    );

    handle.spawn(broker);

    println!(
        "Listening on http://{} with 1 thread.",
        server.local_addr().expect("unable to get local address")
    );
    server.run().expect("unable to run server");
}

static HTML: &str = &r#"<!DOCTYPE html>
<html>
  <head>
    <meta charset="UTF-8"> 
    <title>TJ in Rust</title>
	<style>
		.counterDiv {
		  text-align: left;
		  font-size: 50px;
          color: SlateBlue;
		}
	</style>
  </head>
  <body>
    <h1>TJ in Rust</h1>
    <div id="sse-msg">
    <img class="v-mid ml0-l" alt="Rust Logo" src="https://www.rust-lang.org/static/images/rust-logo-blk.svg">
    </div>
    <script type="text/javascript">
      var evtSource = new EventSource("/events");
      evtSource.addEventListener("uptime", function(e) {
          var sseMsgDiv = document.getElementById('tjcounter');
          const obj = JSON.parse(e.data);
          sseMsgDiv.innerHTML = obj.time;
      }, false);
    </script>
    <form id="tjcounter" class="counterDiv">
      <div>
      </div>
    </form>
  </body>
</html>
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_future_countdown() {
        let now =
            NaiveDateTime::parse_from_str("2022-02-10 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let target =
            NaiveDateTime::parse_from_str("2022-02-11 16:00:00", "%Y-%m-%d %H:%M:%S").unwrap();

        let counter = format_counter(now, target);

        assert_eq!(counter, "until 1 days 06:00:00,0");
    }

    #[test]
    fn formats_elapsed_time_for_past_target() {
        let now =
            NaiveDateTime::parse_from_str("2022-02-12 16:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let target =
            NaiveDateTime::parse_from_str("2022-02-11 16:00:00", "%Y-%m-%d %H:%M:%S").unwrap();

        let counter = format_counter(now, target);

        assert_eq!(counter, "since 1 days 00:00:00,0");
    }
}
