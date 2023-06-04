use std::{fs, path::Path};

pub struct FilesHtml {
    // pub html: String,
    pub path: String,
    pub dir_entries: Result<Vec<(String, bool)>, String>,
    pub is_file: bool,
}

impl FilesHtml {
    pub fn new(path_str: &str) -> Self {
        let mut path = String::from("");
        path.push_str(path_str);
        let mut f = FilesHtml {
            // html: String::from(""),
            path: path.to_string(),
            dir_entries: Ok(Vec::with_capacity(10)),
            is_file: false,
        };
        f.read();
        f
    }
    
    fn syspath(&self) -> String {
        format!("./public{}", &self.path)
    }
    
    pub fn read(&mut self) {
        let syspath = self.syspath();
        let path = Path::new(&syspath);
        self.is_file = path.is_file();
        if !self.is_file {
            self.dir_entries = match path.read_dir() {
                Ok(read_dir) => Ok(read_dir
                    .map(|res| match res {
                        Ok(dir) => match dir.path().components().last() {
                            Some(comp) => (
                                comp.as_os_str().to_string_lossy().to_string(),
                                dir.path().is_file(),
                            ),
                            None => ("<>".to_string(), false),
                        },
                        Err(e) => (
                            format!("{}\n{}", e, &syspath),
                            false,
                        ),
                    })
                    .collect()),
                Err(e) => Err(format!("{}\n{}", e, &syspath)),
            }
        }
    }

    pub fn html(&mut self) -> String {
        let mut html = String::from("<!DOCTYPE html>");
        html.push_str("<html>");
        html.push_str("<head>");
        html.push_str("<title>content</title>");
        html.push_str("<link rel=\"stylesheet\" href=\"/static/style.css\">"); 
        html.push_str("</head>");         
        html.push_str("<body>");
        
        let up_path_str = if let Some(value) = self.path.rfind('/') {
            &self.path[..value]
        } else {
            "/"
        };
        let mut body_html = String::from("");        
        body_html.push_str(&format!(
            "<p><a href=\"{}\"><..></a></p>\n",
            if up_path_str.is_empty() {"/"} else {up_path_str}
        ));            
        match &self.dir_entries {
            Ok(entries) => {
                for entry in entries {
                    let entry_str = format!(
                        "<p><a href=\"{}/{}\">{}{}</a></p>",
                        &self.path,
                        entry.0,
                        if !entry.1 { "&#x1F4C1 " } else { "" },
                        entry.0
                    );
                    body_html.push_str(&entry_str);
                }
            },
            Err(e) => return e.to_owned(),
        };

        body_html.push_str("
        <hr></hr>
        <form method=\"post\" enctype=\"multipart/form-data\">
            <div>
                <p><label for=\"file\">Upload to this folder</label></p>
                <p></p><input type=\"file\" id=\"file\" name=\"file\" multiple /></p>
            </div>
            <div>
                <button>Upload</button>
            </div>
        </form>
        ");
        html.push_str(body_html.as_str());

        html.push_str("</body>");       
        html.push_str("</html>");
        // self.html = html;
        // &self.html
        html
    }

    pub fn response_body(&mut self) -> Vec<u8> {      
        if self.is_file {
            let file_path = self.syspath();
            match fs::read(&file_path) {
                Ok(data) => data,
                Err(e) => format!("read file error\npath: {}\n{}", &file_path, e).as_bytes().to_owned()
            }           
        } else {
            self.html().as_bytes().to_owned()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FilesHtml;

    #[test]
    fn test1() {
        let path_str = "/content";
        let mut fhtml = FilesHtml::new(path_str);
        println!("{:?}", fhtml.dir_entries);
        println!("{:?}", fhtml.html());
    }

    #[test]
    fn prev_dir() {
        let path_str = "/content/upload";
        // let path_str = "/content";
        let up_path_str = if let Some(value) = path_str.rfind("/") {
            &path_str[..value]
        } else {
            "/"
        };
        println!("{}", path_str);
        println!("{}", if up_path_str.is_empty() {"/"} else {up_path_str});
    }

    #[test]
    fn test_coma() {
        let res = "123".parse::<usize>();
        match res {
            Ok(_val) => {
                println!("123")
            },
            Err(e) => {
                println!("{}", e.to_string())
            }
        }
    }

}
