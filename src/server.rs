
use std::{fs, env::Args};

use base64::{Engine, engine::general_purpose as b64};

#[derive(Debug)]
pub enum AuthScheme {
    Basic,
    None,
}

impl TryFrom<&str> for AuthScheme {
    type Error = String;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value.to_lowercase().as_str() {
            "basic" => Ok(Self::Basic),
            "none" => Ok(Self::None),
            str => Err(format!("{} auth scheme not implemented", str))
        }
    }
}

pub trait Auth {
    fn authorize(&mut self) -> Result<(), String>;
}

#[derive(Debug)]
pub struct BasicAuth {
    credentials_str: String,
    username: String,
    password: String,
}

impl BasicAuth {
    pub fn new(str: &str) -> Self {
        let credentials_str = str.replace("Basic ", "");
        BasicAuth {
            credentials_str,
            username: String::from(""),
            password: String::from(""),
        }
    }
    fn parse(&mut self) -> Result<(), String> {
        if self.credentials_str.is_empty() {
            return Err("credentials required, but not provided".to_string())
        };
        let data_debase64 = match b64::STANDARD.decode(self.credentials_str.as_bytes()) {
            Ok(data) => data,
            Err(e) => return Err(format!("credentials parsing error: {}", e))
        };
        let data = match String::from_utf8(data_debase64) {
            Ok(value) => value,
            Err(e) => return Err(e.to_string())
        };
        let mut split = data.split(':');
        self.username = if let Some(value) = split.next() {
            value
        } else {
            return Err("no username provided".to_string())
        }.to_string();
        self.password = if let Some(value) = split.next() {
            value
        } else {
            ""
        }.to_string();
        Ok(())
    }
    fn validate_username(&self) -> Result<(), String> { 
        let users_str = match crate::get_from_cache("htpasswd") {
            Some(value) => value,
            None => String::new()
        };
        let users_lines = users_str.lines();
        for line in users_lines {
            if line.contains(format!("{}:", self.username).as_str()) {
                let mut line_split = line.split(':');
                if line_split.next().is_none() {
                    return Err("user not found".to_string())
                };
                if let Some(passwd) = line_split.next() {
                    if passwd == self.password {
                        return Ok(());
                    } else {
                        return Err("password is incorrect".to_string());
                    }
                };
            };
        };
        let users_str = {
            let file_path = "./private/.htpasswd";
            let htpasswd = match std::fs::read_to_string(file_path) {
                Ok(val) => val,
                Err(e) => panic!("{}: \"{}\"", e, file_path)
            };                
            htpasswd
        };
        let users_lines = users_str.lines();
        for line in users_lines {
            if line.contains(format!("{}:", self.username).as_str()) {
                let mut line_split = line.split(':');
                if line_split.next().is_none() {
                    return Err("user not found".to_string())
                };
                if let Some(passwd) = line_split.next() {
                    crate::append_to_cache("htpasswd", format!("{}\r\n", line).as_str());
                    if passwd == self.password {
                        return Ok(());
                    } else {
                        return Err("password is incorrect".to_string());
                    }
                };
            };
        };            
        Err("user not found".to_string())
    }
}

impl Auth for BasicAuth {
    fn authorize(&mut self) -> Result<(), String> {
        self.parse()?;
        self.validate_username()
    }
}

#[derive(Debug)]
pub struct ServerLimits {
    pub buf_string_limit: usize,
    pub file_buf_size_limit: usize,
    pub file_size_limit: usize,
}

#[derive(Debug)]
pub struct ServerConfig {
    // pub encryption: Option<Encryption>,
    pub auth_scheme: AuthScheme,
    pub thread_count: usize,
    pub port: usize,
    pub limits: ServerLimits,
}

impl ServerConfig {
    pub fn new() -> Self {
        ServerConfig {
            auth_scheme: AuthScheme::None,
            thread_count: 1,
            port: 8080,
            limits: ServerLimits { 
                buf_string_limit: 0,
                file_buf_size_limit: 0,
                file_size_limit: 0,
            },
        }
    }
    #[allow(dead_code)]
    pub fn from_args(mut args: Args) -> Self {
        let mut s_conf = Self::new();
        while let Some(arg) = args.next() {
            match arg.as_str() { 
                "--auth" | "-a" => {
                    if let Some(value) = args.next() {
                        s_conf.auth_scheme = AuthScheme::try_from(value.as_str()).unwrap();
                    } else {
                        s_conf.auth_scheme = AuthScheme::Basic;
                    }
                },
                "-t" | "--threads" => {
                    if let Some(value) = args.next() {
                        s_conf.thread_count = value.parse().unwrap();
                    }   
                },
                "-p" | "--ports" => {
                    if let Some(value) = args.next() {
                        s_conf.port = value.parse().unwrap();
                    }
                },                
                _ => {}
            }
        };
        s_conf
    }
    pub fn from_config_file() -> Self {
        fn size_str_to_bytes_number(str: &str, lines_count: &usize) -> Result<usize, String> {
            let last_index = str.len()-1;
            let multiplier = match &str[last_index..last_index+1] {
                "k" => {
                    1024
                },
                "m" => {
                    1024 * 1024
                },
                "g" => {
                    1024 * 1024 * 1024
                },
                other => {
                    return Err(format!("unexpected letter '{}' at the end of a number in line {}", other, lines_count))
                }                                                        
            };
            let value = str[..last_index].parse::<usize>().unwrap() * multiplier;
            Ok(value.to_owned())
        }        
        let mut s_conf = Self::new();
        let file_path = "./private/.config";
        let file_result = fs::read_to_string(file_path);
        let file_str = if let Ok(file_str) = file_result {
            file_str
        } else {
            return s_conf;
        };
        let mut lines_count = 0;
        for line in file_str.lines() {
            lines_count += 1;
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            // omit comment
            if line.len() > 1 && &line[0..2] == "//" {
                continue;
            }
            let mut line_split = line.split('=');
            let name = if let Some(val) = line_split.next() {
                val.trim().to_lowercase()
            } else {
                panic!("no name for parameter in line {}", lines_count)
            };
            if name.is_empty() {
                panic!("name len == 0 in line {}", lines_count)
            }                
            let value = if let Some(val) = line_split.next() {
                val.trim().to_lowercase()
            } else {
                panic!("no value for parameter {} in line {}", name, lines_count)
            };
            if value.is_empty() {
                panic!("value len == 0 in line {}", lines_count)
            }                
            match name.as_str() {
                "auth" => {
                    s_conf.auth_scheme = AuthScheme::try_from(value.as_str()).unwrap()
                },
                "threads" => {
                    s_conf.thread_count = value.parse().unwrap();
                }, 
                "port" => {
                    s_conf.port = value.parse().unwrap();
                },
                "buf_string_limit" => {
                    s_conf.limits.buf_string_limit = size_str_to_bytes_number(&value, &lines_count).unwrap();
                },   
                "file_buf_size_limit" => {
                    s_conf.limits.file_buf_size_limit = size_str_to_bytes_number(&value, &lines_count).unwrap();
                },
                "file_size_limit" => {
                    s_conf.limits.file_size_limit = size_str_to_bytes_number(&value, &lines_count).unwrap();
                },                                                                        
                other => {
                    panic!("wrong parameter name: '{}' in line {}", other, lines_count);
                }
            }
        }
        s_conf
    }    
    
    pub fn update_from_args(&mut self, mut args: Args) {
        while let Some(arg) = args.next() {
            match arg.as_str() { 
                "--auth" | "-a" => {
                    if let Some(value) = args.next() {
                        self.auth_scheme = AuthScheme::try_from(value.as_str()).unwrap();
                    } else {
                        self.auth_scheme = AuthScheme::Basic;
                    }
                },
                "-t" | "--threads" => {
                    if let Some(value) = args.next() {
                        self.thread_count = value.parse().unwrap();
                    }   
                },
                "-p" | "--ports" => {
                    if let Some(value) = args.next() {
                        self.port = value.parse().unwrap();
                    }
                },                
                _ => {}
            }
        };
    }    
    
    pub fn auth_scheme(&self) -> &AuthScheme {
        &self.auth_scheme
    }
    pub fn limits(&self) -> &ServerLimits {
        &self.limits
    }      
}

#[cfg(test)]
mod test {

    use base64::{Engine, engine::general_purpose as b64};

    use super::ServerConfig;

    #[test]
    fn b64() {
        let data = String::from("user1:pwd1");
        println!("{}", data);
        let data_base64 = b64::STANDARD.encode(data.as_bytes());
        println!("{}", data_base64);
        let data_debase64 = b64::STANDARD.decode(data_base64.as_bytes()).unwrap();
        println!("{}", String::from_utf8_lossy(&data_debase64));        
    }
    #[test]
    fn config_from_file() {
        let s_conf = ServerConfig::from_config_file();
        println!("{:#?}", s_conf)
    }
}
