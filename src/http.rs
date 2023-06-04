use std::{
    collections::HashMap,
    error::Error,
    fs,
    io::{self, BufRead, BufReader},
    net::TcpStream,
};

use crate::server::{Auth, AuthScheme, BasicAuth};

const VERSION: &str = "HTTP/1.1"; // doesn't metter
// const BUF_STRING_LIMIT: usize = 8192 * 100;
// const FILE_BUF_SIZE_LIMIT: usize = 10 * 1024 * 1024; // 10 Mb
// const FILE_SIZE_LIMIT: usize = 1024 * 1024 * 1024; // 1Gb

#[derive(Debug)]
pub enum RequestMethod {
    Get,
    Post,
    Other(String),
}

impl RequestMethod {
    pub fn from(str_raw: &str) -> Self {
        let str = str_raw.trim().to_lowercase();
        match str.as_str() {
            "get" => RequestMethod::Get,
            "post" => RequestMethod::Post,
            other => RequestMethod::Other(other.to_owned()),
        }
    }
}

impl ToString for RequestMethod {
    fn to_string(&self) -> String {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Other(other) => other,
        }
        .to_owned()
    }
}

#[derive(Debug)]
pub struct Request {
    pub method: RequestMethod,
    pub headers: HashMap<String, String>,
    pub url: String,
    pub body: Vec<u8>,
}

impl Request {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Request {
            method: RequestMethod::Get,
            headers: HashMap::new(),
            url: String::from(""),
            body: Vec::new(),
        }
    }
    #[allow(dead_code)]
    pub fn build(method: RequestMethod, url: &str, body: &[u8]) -> Self {
        Request {
            method,
            headers: HashMap::new(),
            url: url.to_owned(),
            body: body.to_owned(),
        }
    }

    fn parse_first_line(&mut self, request_line: &str) {
        let request_words: Vec<&str> = request_line.split(' ').collect();
        let method_str = request_words.first().map_or("", |str| str);
        let version_str = request_words.last().map_or("", |str| str);

        self.method = RequestMethod::from(method_str);
        let request_line_len = request_line.len();
        let word_start = (method_str.len() + 1).min(request_line_len);
        let word_end = if request_line_len > version_str.len() {
            request_line_len - version_str.len()
        } else {
            0
        }
        .max(word_start);
        self.url = request_line[word_start..word_end]
            .trim()
            .replace("%20", " ");
    }

    fn parse_header(&mut self, buf: &str) {
        if buf.is_empty() {
            return;
        }
        let mut header_split = buf.split(": ");
        let name = header_split.next().map_or("", |str| str);
        let value = header_split.next().map_or("", |str| str.trim());
        self.headers.insert(name.to_owned(), value.to_owned());
    }

    fn read_body_from_buf(
        &mut self,
        buf: &mut SafeBuf,
    ) -> Result<usize, Box<dyn Error>> {
        let ctype_str = match self.headers.get("Content-Type") {
            Some(str) => str,
            None => {
                return Err(Box::new(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "no Content-Type header found",
                )))
            }
        };
        // multipart/form-data; boundary=---------------------------133311203534746783952650403015
        let _mime;
        let mut ctype_split = ctype_str.split("; ");
        if let Some(str) = ctype_split.next() {
            _mime = str;
            if _mime != "multipart/form-data" {
                return Err(Box::new(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Content-Type must be multipart/form-data",
                )));
            }
        } else {
            return Err(Box::new(io::Error::new(
                io::ErrorKind::InvalidInput,
                "no mime type found",
            )));
        }
        let boundary;
        if let Some(str) = ctype_split.next() {
            let eqs = str.find('=');
            let boundary_pos = str.find("boundary");
            if boundary_pos.is_none() {
                return Err(Box::new(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "no boundry found",
                )));
            }
            if let Some(eqs_pos) = eqs {
                boundary = str[eqs_pos + 1..].to_string();
            } else {
                return Err(Box::new(io::Error::new(io::ErrorKind::InvalidInput, "no equal sign in boundry")));
            }
        } else {
            return Err(Box::new(io::Error::new(io::ErrorKind::InvalidInput, "no boundry found",)));
        }
        let boundary_start = format!("\r\n--{}", &boundary);
        let boundary_start = boundary_start.as_bytes();

        let boundary_end = "--".as_bytes();


        let empty_data = buf.read_until(boundary.as_bytes())?;// should be empty
        // println!("empty_data: '{}'", String::from_utf8_lossy(&empty_data));
        drop(empty_data);
        loop {
            // println!("loop");
            // find start of part
            let data_remainder = buf.read_until("\r\n".as_bytes())?;
            // println!("data_remainder: '{}'", String::from_utf8_lossy(&data_remainder));
            if data_remainder == boundary_end {
                break;
            }
            drop(data_remainder);
            let line = buf.read_line()?;
            // println!("#1 line: '{}'", line);
            let mut file_name = "last_upload";
            for str in line.split(';') {
                let file_name_pos = str.find("filename=\"");
                if let Some(pos) = file_name_pos {
                    file_name = &str[pos+10..str.len()-1];
                }
            }
            let _line = buf.read_line()?;// ignore Content-Type
            // println!("#2 line: '{}'", _line);
            let line = buf.read_line()?;// must be empty - delimit beginning of file data
            // println!("#3 line: '{}'", line);
            if !line.is_empty() {
                return Err(Box::new(io::Error::new(io::ErrorKind::InvalidInput, "no data start delimiter found (\\r\\n\\r\\n)")));
            }
            // println!("url: {}", self.url);
            let file_path = match self.url.as_str() {
                "/upload" => format!("./public/content/upload/{}", file_name),
                url => {
                    format!("./public/{}/{}", url, file_name)
                }
            };
            // println!("file_path: '{}'", file_path);
            buf.set_file_path(&file_path);
            let _file_data = buf.read_until(boundary_start)?;// should be emty if file_path is set
            // println!("'{}'", String::from_utf8_lossy(&_file_data));
        }
        buf.clear_file_name();

        self.body = Self::get_msg_str("saved", "").as_bytes().to_owned();
        Ok(1)
    }

    fn get_msg_str(header: &str, msg: &str) -> String {
        let response_pattern = match fs::read("./public/static/msg.html") {
            Ok(val) => val,
            Err(_e) => header.as_bytes().to_owned(),
        };
        let response_str = String::from_utf8_lossy(&response_pattern);
        response_str.replace("{header}", header).replace("{msg}", msg)
    }

    pub fn authorize(&self, auth_scheme: &AuthScheme) -> Result<(), Response> {
        if let AuthScheme::None = auth_scheme {
            return Ok(());
        };

        let credentials_str = self.headers.get("Authorization").map_or("", |str| str);
        let mut headers = HashMap::new();
        let auth_result = match auth_scheme {
            AuthScheme::None => return Ok(()),
            AuthScheme::Basic => {
                headers.insert("WWW-Authenticate".to_owned(), "Basic".to_owned());
                let mut auth = BasicAuth::new(credentials_str);
                auth.authorize()
            }
        };
        match auth_result {
            Ok(_) => Ok(()),
            Err(e) => {
                let mut response = Response::new();
                response.str_fill(&e);
                response.status = 401;
                response.headers.extend(headers.into_iter());
                Err(response)
            }
        }
    }
}

impl TryFrom<&TcpStream> for Request {
    type Error = Box<dyn std::error::Error>;
    fn try_from(stream: &TcpStream) -> Result<Self, Self::Error> {
        let mut buf = SafeBuf::try_from(stream)?;
        // println!("'{}'", String::from_utf8_lossy(&buf._buf()));
        let line = buf.read_line()?;
        // println!("'{}'", line);
        let mut request = Self::new();
        if line.is_empty() {
            return Ok(request);
        }        
        request.parse_first_line(&line);
        loop {
            let line = buf.read_line()?;
            // println!("'{}'", line);
            if line.is_empty() {
                break;
            }
            request.parse_header(&line);
        }
        if let RequestMethod::Post = request.method {
            request.read_body_from_buf(&mut buf)?;
        }
        // println!("{:#?}", request);
        Ok(request)
    }
}


struct SafeBuf<'a> {
    buf_reader: BufReader<&'a TcpStream>,
    buf: Vec<u8>,
    index: usize,
    buf_len: usize,
    buf_tail: Vec<u8>,
    file_path: Option<String>,
    file_size: usize,
}

impl<'a> TryFrom<&'a TcpStream> for SafeBuf<'a> {
    type Error = Box<dyn std::error::Error>;
    fn try_from(stream: &'a TcpStream) -> Result<Self, Self::Error> {
        let mut safe_buf = SafeBuf {
            buf_reader: BufReader::new(stream),
            buf: Vec::with_capacity(8192),
            index: 0,
            buf_len: 0,
            buf_tail: Vec::with_capacity(3000),
            file_path: None,
            file_size: 0,
        };
        safe_buf.update_buf()?;
        Ok(safe_buf)
    }
}

impl<'a> SafeBuf<'a> {

    fn update_buf(&mut self) -> Result<(), Box<dyn Error>> {
        self.buf_len = self.buf.len();
        if self.buf_len == 0 {
            // println!("update_buf: start");
            self.buf = self.buf_reader.fill_buf()?.to_owned();
            self.index = 0;
            self.buf_len = self.buf.len();
            // println!("update_buf: buf_len = {}; buf_tail_len = {}", self.buf_len, self.buf_tail.len());
            // if self.buf_len > 0 {
            //     println!("buf: '{}'", String::from_utf8_lossy(&self.buf));
            // }
        };
        Ok(())
    }

    fn consume_buf(&mut self) {
        self.buf_tail.extend_from_slice(&self.buf[self.index..]);
        self.buf.clear();
        self.index = 0;
        self.buf_reader.consume(self.buf_len);
        self.buf_len = 0;
    }

    fn check_limits(&mut self) -> Result<(), Box<dyn Error>> {
        let limits = crate::S_CONF.get().unwrap().limits();
        // redirect output to file if file_path is set
        if let Some(file_path) = &self.file_path {
            if limits.file_buf_size_limit > 0 && self.buf_tail.len() > limits.file_buf_size_limit {
                if limits.file_size_limit > 0 && self.file_size > limits.file_size_limit {
                    // todo: remove file 
                    return Err(Box::new(io::Error::new(
                        io::ErrorKind::OutOfMemory,
                        format!("FILE_SIZE_LIMIT({}) was reached ", limits.file_size_limit)
                    )))
                } else {
                    self.file_size += self.buf_tail.len();
                    fs::write(file_path, std::mem::take(&mut self.buf_tail))?
                }
            }
        } else {
            if limits.buf_string_limit > 0 && self.buf_tail.len() > limits.buf_string_limit {
                return Err(Box::new(io::Error::new(
                    io::ErrorKind::OutOfMemory,
                    format!("BUF_STRING_LIMIT({}) was reached ", limits.buf_string_limit)
                )))
            }
        }
        Ok(())
    }

    pub fn set_file_path(&mut self, file_path: &str) {
        self.file_size = 0;
        self.file_path = Some(file_path.to_string());    
    }

    pub fn clear_file_name(&mut self) {
        self.file_size = 0;
        self.file_path = None;
    }

    pub fn read_until(&mut self, delimiter: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
        let delimiter_len = delimiter.len();
        'outer: loop {
            self.update_buf()?;
            if self.buf_len == 0 {
                break 'outer;
            }
            if delimiter_len > self.buf_len {
                return Err(Box::new(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "delimiter_len > buf_len",
                )))
            }            
            for i in self.index..self.buf_len-delimiter_len+1 {
                if &self.buf[i..i+delimiter_len] == delimiter {
                    self.buf_tail.extend_from_slice(&self.buf[self.index..i]);
                    self.index = i + delimiter_len;
                    break 'outer;
                }
            }
            self.check_limits()?;
            self.consume_buf();
        }
        // redirect output to file if file_path is set
        if let Some(file_path) = &self.file_path {
            fs::write(file_path, std::mem::take(&mut self.buf_tail))?;
            self.file_size = 0;
            self.file_path = None;
        }        
        Ok(std::mem::take(&mut self.buf_tail))
    }

    pub fn read_line(&mut self) -> Result<String, Box<dyn Error>> {
        Ok(String::from_utf8(self.read_until("\r\n".as_bytes())?)?)
    }
    
    fn _read_buf_until(&mut self, delimiter: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
        let delimiter_len = delimiter.len();
        if delimiter_len > self.buf_len {
            return Err(Box::new(io::Error::new(
                io::ErrorKind::InvalidInput,
                "delimiter_len > buf_len",
            )))
        }            
        for i in self.index..self.buf_len-delimiter_len+1 {
            if &self.buf[i..i+delimiter_len] == delimiter {
                let data = self.buf[self.index..i].to_owned();
                self.index = i + delimiter_len;
                return Ok(data)
            }
        }
        Ok(self.buf.clone())
    }
    
    pub fn _read_first_line(&mut self) -> Result<String, Box<dyn Error>> {
        Ok(String::from_utf8(self._read_buf_until("\r\n".as_bytes())?)?)
    }    

    pub fn _buf(&self) -> Vec<u8> {
        self.buf.clone()
    }

}

impl<'a> Drop for SafeBuf<'a> {
    fn drop(&mut self) {
        self.buf_reader.consume(self.buf_len);
    }
}

#[derive(Debug)]
pub struct Response {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl Response {
    pub fn new() -> Self {
        Response {
            status: 200,
            headers: HashMap::new(),
            body: Vec::new(),
        }
    }
    pub fn build_request_echo(request: &Request) -> Self {
        let mut headers = HashMap::new();
        headers.insert(
            "echo-header".to_string(),
            format!("{} {} {}", request.method.to_string(), request.url, VERSION),
        );
        headers.extend(request.headers.clone().into_iter());
        Response {
            status: 200,
            headers,
            body: request.body.clone(),
        }
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        let mut buf: Vec<u8> = vec![];
        if self.status > 0 {
            buf.extend_from_slice(
                format!("{} {} {}\r\n", VERSION, self.status, self.status_str()).as_bytes(),
            );
        }
        let mut headers_str = String::from("");
        for header in &self.headers {
            headers_str.push_str(&format!("{}: {}\r\n", header.0, header.1));
        }
        if !headers_str.is_empty() {
            buf.extend_from_slice(headers_str.as_bytes());
        }
        buf.extend_from_slice("\r\n".as_bytes());
        buf.extend_from_slice(&self.body);
        buf
    }
    fn status_str(&self) -> &str {
        match self.status {
            200 => "OK",
            201 => "CREATED",
            401 => "UNAUTHORIZED",
            403 => "FORBIDDEN",
            404 => "NOT FOUND",
            _ => "NOT OK",
        }
    }
    pub fn file_fill(&mut self, path_str: &str) {
        self.body.extend_from_slice(&fs::read(path_str).unwrap_or(format!("read file error, path: '{}'", path_str).as_bytes().to_owned()));
    }
    pub fn str_fill(&mut self, str: &str) {
        self.body.extend_from_slice(str.as_bytes());
    }
    pub fn from_file(path_str: &str) -> Self {
        let mut response = Self::new();
        response.file_fill(path_str);
        response
    }

    fn add_content_headers(&mut self, _content_type: Option<&str>) {
        self.headers
            .entry("Content-Length".to_string())
            .or_insert(self.body.len().to_string());
        // self.headers.entry("Content-Type".to_string())
        //     .or_insert(content_type
        //     .map_or("multipart/form-data; boundary=---------------------------12345678901234567890123456789".to_string(), |x| x.to_string()));
    }
}

impl From<&str> for Response {
    fn from(value: &str) -> Self {
        let mut response = Self::new();
        response.str_fill(value);
        response
    }
}

impl From<&[u8]> for Response {
    fn from(value: &[u8]) -> Self {
        let mut response = Self::new();
        response.body.extend_from_slice(value);
        response.add_content_headers(None);
        response
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    #[test]
    fn parse_headers() {
        let mut headers = HashMap::new();
        // let mut buf = String::from("");
        let request_str = String::from(
            "GET /abc HTTP/1.1
Host: localhost:8080
User-Agent: Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:109.0) Gecko/20100101 Firefox/113.0
Accept: text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8
Accept-Language: en-US,en;q=0.5
Accept-Encoding: gzip, deflate, br
Connection: keep-alive
Upgrade-Insecure-Requests: 1
Sec-Fetch-Dest: document
Sec-Fetch-Mode: navigate
Sec-Fetch-Site: none
Sec-Fetch-User: ?1

");
        let mut lines = request_str.lines();
        while let Some(buf) = lines.next() {
            if buf == "\r\n" {
                break;
            };
            {
                let mut header_split = buf.split(": ");
                let name = header_split.next().map_or("", |str| str);
                let value = header_split.next().map_or("", |str| str.trim());
                headers.insert(name.to_owned(), value.to_owned());
            };
            println!("{}", &buf);
        }
        println!("{:#?}", headers)
    }
}
