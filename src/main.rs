pub mod fs_html;
mod http;
mod server;

use hello_server::ThreadPool;
use http::{Request, Response};
use server::ServerConfig;
use std::{
    env::{self},
    io::Write,
    net::{TcpListener, TcpStream},
    time::Duration, collections::HashMap, sync::{Mutex, OnceLock},
};

static S_CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

static S_CONF: OnceLock<ServerConfig> = OnceLock::new();

fn main() {
    init_folders();
    let mut args = env::args();
    args.next(); //command line
    if let Some(arg) = args.next() {
        if arg.contains("help") || arg.contains("-h") {
            print_help(&arg);
            return;
        }
    };
    // let s_conf = Arc::new(ServerConfig::from_args(env::args()));
    let mut s_conf = ServerConfig::from_config_file();
    s_conf.update_from_args(env::args());

    // println!("{:#?}", s_conf);

    let address = format!("0.0.0.0:{}", s_conf.port);
    let listener = TcpListener::bind(address).unwrap();

    let pool = match ThreadPool::build(s_conf.thread_count) {
        Ok(v) => v,
        Err(e) => {
            println!("error creating pool thread: {}", e);
            return;
        }
    };

    S_CONF.set(s_conf).unwrap();
    S_CACHE.set(Mutex::new(HashMap::new())).unwrap();

    for stream in listener.incoming() {
        let stream = stream.unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        // let s_conf_ref = s_conf.clone();
        pool.execute(|| handle_connection(stream));
    }

    println!("main end");

}

fn handle_connection(mut stream: TcpStream) {
    let auth_scheme = crate::S_CONF.get().unwrap().auth_scheme();
    
    // println!("\nhandle_connection {}", stream.local_addr().unwrap());
    let mut request = match http::Request::try_from(&stream) {
        Ok(val) => val,
        Err(e) => {
            let error_str = format!("Request::try_from\n{}", e);
            println!("ERROR: {}", error_str);
            let response = Response::from(&error_str[..]);
            match stream.write_all(&response.as_bytes()) {
                Ok(_) => {},
                Err(e) => println!("stream.write_all\n{}", e)
            };
            return;
        }
    };
    if let Err(response) = request.authorize(auth_scheme) {
        match stream.write_all(&response.as_bytes()) {
            Ok(_) => {},
            Err(e) => println!("stream.write_all\n{}", e)
        };        
        return;
    }
    // println!("{:#?}", request);
    let response = match request.method {
        http::RequestMethod::Get => response_get(&mut request),
        http::RequestMethod::Post => Response::from(&request.body[..]),
        _ => http::Response::build_request_echo(&request),
    };
    // println!("{:#?}", response);

    match stream.write_all(&response.as_bytes()) {
        Ok(_) => {}
        Err(e) => {
            println!("stream.write_all\n{}", e)
        }
    };
    match stream.flush() {
        Ok(_) => {}
        Err(e) => {
            println!("stream.flush\n{}", e)
        }
    };
}

fn response_get(request: &mut Request) -> http::Response {
    match request.url.as_str() {
        "/" => Response::from_file("./public/static/hello.html"),
        "/upload" => Response::from_file("./public/static/upload.html"),
        "/favicon.ico" => Response::from_file("./public/static/favicon.ico"),
        "/echo" => http::Response::build_request_echo(request),
        _ => {
            let mut files_html = fs_html::FilesHtml::new(&request.url);
            Response::from(files_html.response_body().as_slice())
        } 
    }
}

fn print_help(arg: &str) {
    match arg {
        "help" | "-help" | "--help" | "-h" | "--h" => {
            println!("hello server [OPTIONS]");
            println!("OPTIONS:");
            println!(" -a, --auth <basic|none>  default is none");
            println!(" -t, --threads <NUMBER>   default is 2");
            println!(" -p, --port <NUMBER>   default is 8080");
            // println!(" -e, --encryption <ecb|cbc>");
        }
        _ => {}
    }
}

fn init_folders() {
    let path = std::path::Path::new("./public/content/");
    if !path.exists() {
        std::fs::create_dir(path).unwrap();
    }
    let path = std::path::Path::new("./public/content/upload");
    if !path.exists() {
        std::fs::create_dir(path).unwrap();
    }    
}

fn get_from_cache(name: &str) -> Option<String> {
    let mutex = match S_CACHE.get() {
        Some(val) => {
            val
        },
        None => {
            panic!("S_CACHE uninitialized");
        }
    };
    let hash_map = match mutex.lock() {
        Ok(val) => val,
        Err(e) => {
            println!("get_from_cache/mutex.lock()\n{}", e);
            return None
        }
    };
    // println!("cache get\n{:#?}", hash_map);
    match hash_map.get(name) {
        Some(val) => Some(val.to_owned()),
        None => None
    }
}

fn _update_in_cache(name: &str, value: &str) {
    let mutex = match S_CACHE.get() {
        Some(val) => {
            val
        },
        None => {
            panic!("S_CACHE uninitialized");
        }
    };
    let mut hash_map = match mutex.lock() {
        Ok(val) => val,
        Err(e) => {
            println!("get_from_cache/mutex.lock()\n{}", e);
            return
        }
    };
    hash_map.insert(name.to_owned(), value.to_owned());
    // println!("cache update\n{:#?}", hash_map);
}

fn append_to_cache(name: &str, value: &str) {
    let mutex = match S_CACHE.get() {
        Some(val) => {
            val
        },
        None => {
            panic!("S_CACHE uninitialized");
        }
    };
    let mut hash_map = match mutex.lock() {
        Ok(val) => val,
        Err(e) => {
            println!("get_from_cache/mutex.lock()\n{}", e);
            return
        }
    };
    let mut init_value = match hash_map.get(name) {
        Some(val) => val.to_owned(),
        None => String::new()
    };
    init_value.push_str(value);
    hash_map.insert(name.to_owned(), init_value);
    // println!("cache append\n{:#?}", hash_map);
}
