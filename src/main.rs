// TJ Counter in RUST
use bytes::{BytesMut, Bytes, BufMut};

use futures::Stream;
use futures::future::{Future, Either, ok, loop_fn, Loop};
use futures::sync::mpsc;
use futures::sink::Sink;

use tokio_core::reactor::Timeout;

use hyper::{Get, StatusCode};
use hyper::server::{Http, Service, Request, Response};
use hyper::mime;
use hyper::header::{ContentType, Connection, AccessControlAllowOrigin};
use hyper::Chunk;

use sprintf::sprintf;
use chrono::{ NaiveDateTime, Utc};

use std::str;

use std::time::Duration;
use std::io::Write;

// this fn replaces closures to avoid boxing in some cases
fn print_err<T:std::fmt::Debug>(t:T) {
    println!("{:?}", t);
}

struct EventService {
    tx_new: mpsc::Sender<mpsc::Sender<Result<Chunk,hyper::Error>>>,
}

impl Service for EventService {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Future = Box<dyn Future<Item=Response, Error=Self::Error>>;

    fn call(&self, req: Request) -> Self::Future {
        match (req.method(), req.path()) {
            (&Get, "/events") => { println!("request events");
                let (tx_msg, rx_msg) = mpsc::channel(10);
                Box::new(
                    self.tx_new.clone().send(tx_msg)
                        .and_then(|_|
                            Ok(Response::new()
                                .with_status(StatusCode::Ok)
                                .with_header(AccessControlAllowOrigin::Any)
                                .with_header(ContentType(mime::TEXT_EVENT_STREAM))
                                .with_header(Connection::keep_alive())   
                                .with_body(rx_msg))
                        )
                        .or_else(|_| Ok(Response::new().with_status(StatusCode::NotAcceptable)))
                )
            },

            (&Get, "/") => { 
                println!("request html");
                Box::new(ok(Response::new().with_status(StatusCode::Ok).with_body(HTML)))
            }

            (method, path) => { 
                println!("invalid request method: {:?}, path: {:?}", method, path);
                Box::new(ok(Response::new().with_status(StatusCode::NotFound)))
            }
        }
    }
}

const  TN: i64 = 1;
const TMC: i64 = 1000 * TN;
const TML: i64 = 1000 * TMC;
const TSC: i64 = 1000 * TML;
const TMN: i64 = 60 * TSC;
const THR: i64 = 60 * TMN;

fn calc_counter(freedom: &NaiveDateTime) -> String {
    let nowtm = Utc::now().naive_utc();
    //let durdiff = nowtm.signed_duration_since(freedom);
    let durdiff = freedom.signed_duration_since(nowtm);

    let diff = durdiff.num_nanoseconds().unwrap();

    let days = diff / (60 * 60 * 24 * 1000000000);
    let diff = diff - days*(60*60*24*1000000000);
    let hours = diff / THR;
    let diff = diff - hours*THR;
    let minutes = diff / TMN;
    let diff = diff - minutes*TMN;
    let seconds = diff / TSC;
    let diff = diff - seconds*TSC;
    let tens = diff / 100000000;
    
    let counter = sprintf!("event: %d days %02d:%02d:%02d,%d", days, hours, minutes, seconds, tens).unwrap();
    counter
}

fn main() {
    let addr = "0.0.0.0:8182".parse().expect("addres parsing failed");

    let (tx_new, rx_new) = mpsc::channel(10);

    let server = Http::new().bind(&addr, move || Ok(EventService{ tx_new: tx_new.clone() })).expect("unable to create server");
    let handle = server.handle();
    let handle2 = handle.clone();    

    let event_delay = Duration::from_millis(100); // 
    let _start_time = std::time::Instant::now();

    let fu_to = Timeout::new(event_delay, &handle).unwrap().map_err(print_err);
    let fu_rx = rx_new.into_future().map_err(print_err);
    let clients:Vec<mpsc::Sender<Result<Chunk, hyper::Error>>> = Vec::new();
    
    let freedom = NaiveDateTime::parse_from_str("2022-02-11 16:00:00", "%Y-%m-%d %H:%M:%S").unwrap();

    let broker = loop_fn((fu_to, fu_rx, clients, 0), move |(fu_to, fu_rx, mut clients, event_counter)|{
        let handle = handle2.clone(); 
        fu_to.select2(fu_rx)
            .map_err(|_| ())
            .and_then(move |done|
                match done {
                    Either::A((_, fu_rx)) => Either::A({//send messages
                        let mut buf = BytesMut::with_capacity(512).writer();
                        let msg = calc_counter(&freedom);
                        //println!("msg {}", msg);
                        write!(buf, "event: uptime\ndata: {{\"time\": \"{}\"}}\n\n", msg).expect("msg write failed");
                        let msg:Bytes = buf.into_inner().freeze();
                        let tx_iter = clients.into_iter()
                            .map(|tx| tx.send(Ok(Chunk::from(msg.clone().to_vec()))));
                        futures::stream::futures_unordered(tx_iter)
                            .map(Some)
                            .or_else(|e| { println!("{:?} client removed", e); Ok::<_,()>(None)})
                            .filter_map(|x| x)
                            .collect()
                            .and_then(move |clients|
                                ok(Loop::Continue((
                                    Timeout::new(event_delay, &handle).unwrap().map_err(print_err),
                                    fu_rx, 
                                    clients,
                                    event_counter + 1
                                )))                            
                            )
                    }),
                        
                    Either::B(((item, rx_new), fu_to)) => Either::B({//register new client
                        match item {
                            Some(item) => {
                                clients.push(item); 
                                println!("client {} registered", clients.len());
                            },
                            None => println!("keeper loop get None"),
                        }       

                        ok(Loop::Continue((
                            fu_to,
                            rx_new.into_future().map_err(print_err), 
                            clients,
                            event_counter
                        )))
                    }),
                }              
            )
    });

    handle.spawn(broker);

    println!("Listening on http://{} with 1 thread.", server.local_addr().expect("unable to get local address"));
    server.run().expect("unable to run server");
}

static HTML:&str = &r#"<!DOCTYPE html>
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
      var evtSource = new EventSource("http://127.0.0.1:8182/events");
      evtSource.addEventListener("uptime", function(e) {
          var sseMsgDiv = document.getElementById('tjcounter');
          const obj = JSON.parse(e.data);
          const tjcnt = obj.time.split("event:");
          sseMsgDiv.innerHTML = tjcnt[1];
      }, false);
    </script>
    <form id="tjcounter" class="counterDiv">
      <div>
      </div>
    </form>
  </body>
</html>
"#;