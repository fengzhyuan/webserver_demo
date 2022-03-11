use std::fs;
use std::io;
use std::io::prelude::*;
use std::net::TcpListener;
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

use webserver::ThreadPool;

struct WebServer {
    listener: TcpListener,
    threadpool: ThreadPool,
    running: Arc<AtomicBool>,
}

impl WebServer {
    pub fn new() -> WebServer {
        WebServer {
            listener: {
                let listener = TcpListener::bind("127.0.0.1:2333").unwrap();
                listener
                    .set_nonblocking(true)
                    .expect("Failed to set non-blocking mode");
                listener
            },
            threadpool: ThreadPool::new(5).unwrap(),
            running: Arc::new(AtomicBool::new(true)),
        }
    }

    pub fn set_exception_handler(&self) {
        let running = self.running.clone();
        ctrlc::set_handler(move || {
            println!("SIGINT detected; Shutting down...");
            running.store(false, Ordering::Relaxed);
        })
        .expect("error setting ctrlc handler");
    }

    pub fn run(&self) {
        loop {
            if !self.running.load(Ordering::Relaxed) {
                break;
            }

            for stream in self.listener.incoming() {
                if !self.threadpool.is_running() {
                    break;
                }

                match stream {
                    Ok(stream) => {
                        self.threadpool.execute(move || {
                            WebServer::handle_connection(stream);
                        });
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                        break;
                    }
                    Err(e) => {
                        panic!("encountered IO error: {}", e);
                    }
                }
            }
        }
    }

    fn handle_connection(mut stream: TcpStream) {
        let mut buffer = [0; 1024];
        Read::read(&mut stream, &mut buffer).unwrap();
    
        let get = b"GET / HTTP/1.1\r\n";
        let sleep = b"GET /sleep HTTP/1.1\r\n";
    
        let (status_line, filename) = if buffer.starts_with(get) {
            ("HTTP/1.1 200 OK", "hello.html")
        } else if buffer.starts_with(sleep) {
            thread::sleep(Duration::from_secs(5));
            ("HTTP/1.1 200 OK", "hello.html")
        } else {
            ("HTTP/1.1 404 NOT FOUND", "404.html")
        };
    
        let contents = fs::read_to_string(filename).unwrap();
        let response = format!(
            "{}\r\nContent-Length: {}\r\n\r\n{}",
            status_line,
            contents.len(),
            contents
        );
    
        Write::write(&mut stream, response.as_bytes()).unwrap();
        Write::flush(&mut stream).unwrap();
    }
}

impl Drop for WebServer {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        drop(&self.threadpool);
    }
}

fn main() {
    let webserver = WebServer::new();
    webserver.set_exception_handler();
    webserver.run();
}
