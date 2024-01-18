use rustls::pki_types::ServerName;
use serde_json::Value;
use std::collections::HashSet;
use std::env;
use std::fs::File;
use std::sync::Arc;
use std::io::{Read, Write};
use std::net::TcpStream;

use rustls;
use webpki_roots;
use regex::Regex;

const UNENCRYPTIED_PORT: &str = "27993";
const ENCRYPTIED_PORT: &str = "27994";
const WORD_FILE: &str = "words.txt";

fn make_a_guess(id: &String, word: String) -> String {
    let mut res = String::from("{");
    res.push_str(format!("\"type\":\"guess\",\"id\":{},\"word\":\"{}\"", id, word).as_str());
    res.push_str("}\n");
    res
}

fn load_words() -> String {
    let mut f = File::open(WORD_FILE).expect("failed to open the words file");
    let mut words = vec![];

    f.read_to_end(&mut words).expect("failed to read file");

    return String::from_utf8_lossy(&words).to_string();
}

fn find_flag<T: Write + Read>(id: String, mut stream: T) -> String {
    let large_words: String = load_words();
    let words_vec: Vec<&str> = large_words.split("\n").collect();
    let mut words_pool: HashSet<&str> = words_vec.into_iter().collect();
    let mut reg = ['.'; 5];
    let mut should_contain: HashSet<char> = HashSet::new();

    loop {
        let tmp: String = reg.iter().collect();
        let re = Regex::new(&format!("{}", tmp)).unwrap();
        let should_have: HashSet<char> = should_contain.clone();
        words_pool.retain(|&s| {
            let mut flag = true;

            for c in should_have.iter() {
                if !s.contains(*c) {
                    flag = false;
                    break;
                }
            }

            re.is_match(s) && flag
        });

        let word = words_pool.clone().drain().next().unwrap();
        words_pool.remove(word);

        stream
            .write(make_a_guess(&id, word.to_string()).as_bytes())
            .expect("failed to send a guess message");

        let mut guessed_buf = [0; 1024];
        stream
            .read(&mut guessed_buf)
            .expect("failed to receive the result of the guess");
        let guessed_res = String::from_utf8_lossy(&guessed_buf);

        let (res, _) = guessed_res.split_at(guessed_res.find("\n").unwrap());

        let v: Value = serde_json::from_str(res).expect("JSON parsing error (guess)");

        if &v["type"] == "retry" {
            if let Value::Array(x) = &v["guesses"] {
                let last_guess = &x[x.len() - 1];
                if let Value::Array(marks) = &last_guess["marks"] {
                    for (ind, num) in marks.iter().enumerate() {
                        let word_v: Vec<char> = word.chars().collect();
                        match num.as_u64() {
                            Some(m) => {
                                if m == 2u64 {
                                    reg[ind] = word_v[ind];
                                } else if m == 1u64 {
                                    should_contain.insert(word_v[ind]);
                                }
                            }
                            None => {}
                        }
                    }
                }
            }
        } else {
            // println!("flag: {}, answer: {}", &v["flag"], &word);
            return v["flag"].to_string();
            // break;
        }
    }
}

fn unencrypted_tcp(host_name: &str, username: &str, port_num: &str) {
    match TcpStream::connect(format!("{}:{}", host_name, port_num)) {
        Ok(mut stream) => {
            // let data = r#"{"type": "hello","northeastern_username": "lee.chih-"}"#;
            let mut data = "{\"type\":\"hello\",\"northeastern_username\":".to_string();
            data.push_str(&format!("\"{}\"", username));
            data.push_str("}");
            stream
                .write(format!("{}\n", data).as_bytes())
                .expect("failed to send the hello message");

            let mut buf = [0; 128];
            stream
                .read(&mut buf)
                .expect("failed to read the start message");

            let hello_res = String::from_utf8_lossy(&buf);
            let (res, _) = hello_res.split_at(hello_res.find('\n').unwrap());

            let v: Value = serde_json::from_str(res).expect("JSON parsing failed");

            let id = v["id"].to_string();

            println!("{}", find_flag(id.clone(), stream));
        }
        Err(_) => {}
    }
}

fn encrypted_tcp(host_name: String, username: &str, port_num: &str) {
    let mut root_store = rustls::RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let mut config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    // Allow using SSLKEYLOGFILE.
    config.key_log = Arc::new(rustls::KeyLogFile::new());

    let server_name = ServerName::try_from(host_name.clone()).expect("invalid hostname");
    let mut conn = rustls::ClientConnection::new(Arc::new(config), server_name).unwrap();
    // let mut sock = TcpStream::connect("proj1.3700.network:27994").unwrap();

    if let Ok(mut stream) = TcpStream::connect(format!("{}:{}", host_name, port_num)) {

        let mut tls = rustls::Stream::new(&mut conn, &mut stream);
        let mut data = "{\"type\":\"hello\",\"northeastern_username\":".to_string();
        data.push_str(&format!("\"{}\"", username));
        data.push_str("}");

        tls.write(format!("{}\n", data).as_bytes())
            .expect("failed to send the hello message");

        let mut buf = [0; 128];
        tls.read(&mut buf)
            .expect("failed to read the start message");

        let hello_res = String::from_utf8_lossy(&buf);
        let (res, _) = hello_res.split_at(hello_res.find('\n').unwrap());

        let v: Value = serde_json::from_str(res).expect("JSON parsing failed");
        let id = v["id"].to_string();

        println!("{}", find_flag(id.clone(), tls));
    }
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();

    let mut host_name = "".to_string();
    let mut username = "".to_string();
    let mut is_tls = false;
    let mut port_num = "".to_string();

    match args.len() {
        3..=6 => {
            let n = args.len();
            let mut ind: usize = 1;
            while ind < n {
                match args[ind].as_str() {
                    "-p" => match args.get(ind + 1) {
                        Some(x) => {
                            port_num.push_str(x.as_str());
                            ind += 2;
                        }
                        None => {
                            writeln!(&mut std::io::stderr(), "-p flag without port number")?;
                            return Ok(());
                        }
                    },
                    "-s" => {
                        is_tls = true;
                        ind += 1;
                    }
                    _ => {
                        if host_name.is_empty() && username.is_empty() {
                            host_name.push_str(args[ind].as_str());
                            ind += 1;
                        } else if username.is_empty() {
                            username.push_str(args[ind].as_str());
                            ind += 1;
                        }
                    }
                }
            }
        }
        _ => {
            writeln!(&mut std::io::stderr(), "invalid command line syntax")?;
            return Ok(());
        }
    }

    if host_name.is_empty() || username.is_empty() {
        writeln!(
            &mut std::io::stderr(),
            "missing hostname or Northeastern-username"
        )?;
        return Ok(());
    }

    if port_num.is_empty() {
        if is_tls {
            port_num = ENCRYPTIED_PORT.to_string();
            encrypted_tcp(host_name.clone(), &username, &port_num);
        } else {
            port_num = UNENCRYPTIED_PORT.to_string();
            unencrypted_tcp(&host_name, &username, &port_num);
        }
    }
    Ok(())
}
