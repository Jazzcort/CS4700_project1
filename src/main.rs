use rustls::pki_types::ServerName;
use serde_json::Value;
use std::collections::HashSet;
use std::env;
use std::fs::File;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;

// import crate
use regex::Regex;
use rustls;
use webpki_roots;

// constant value for default port numbers and workd file directory
const UNENCRYPTIED_PORT: &str = "27993";
const ENCRYPTIED_PORT: &str = "27994";
const WORD_FILE: &str = "words.txt";

/**
 * This function is used to generate the JSON formatted string for
 * the "guess" message with given session id and word.
 */
fn make_a_guess(id: &String, word: String) -> String {
    let mut res = String::from("{");
    res.push_str(format!("\"type\":\"guess\",\"id\":{},\"word\":\"{}\"", id, word).as_str());
    res.push_str("}\n");
    res
}

/**
 * This function is used to load the words file from the root directory to a string.
 */
fn load_words() -> String {
    let mut f = File::open(WORD_FILE).expect("failed to open the words file");
    let mut words = vec![];

    f.read_to_end(&mut words).expect("failed to read file");

    return String::from_utf8_lossy(&words).to_string();
}

/**
 * This function is used to find the secret flag and returns it as a string.
 * After the client side shaked hands with the server side and got the "start" message, the socket stream
 * would be pass to this function with the session id for the guessing part of the game.
 */
fn find_flag<T: Write + Read>(id: String, mut stream: T) -> String {
    // load the words file and construct the words pool
    let large_words: String = load_words();
    let mut words_pool: Vec<&str> = large_words.split("\n").collect();

    // construct the initial Regular Expression pattern and the HashSet for the correct word
    let mut reg = ['.'; 5];
    let mut should_contain: HashSet<char> = HashSet::new();

    // Keep guessing untill it gets the correct word
    loop {
        let tmp: String = reg.iter().collect();
        // construct the Regex object from tmp
        let re = Regex::new(&format!("{}", tmp)).unwrap();
        let should_have: HashSet<char> = should_contain.clone();
        // filter out the invalid word
        words_pool.retain(|&s| {
            let mut flag = true;

            // check if the word contains all the correct letters
            for c in should_have.iter() {
                if !s.contains(*c) {
                    flag = false;
                    break;
                }
            }

            // check if the word matches the pattern
            re.is_match(s) && flag
        });

        // pick a random word
        let word = words_pool
            .pop()
            .expect("Running out of words before getting the correct answer");

        // write to the server stream
        stream
            .write(make_a_guess(&id, word.to_string()).as_bytes())
            .expect("failed to send a guess message");

        let mut guessed_buf = [0; 2048];
        // read the response from the server
        stream
            .read(&mut guessed_buf)
            .expect("failed to receive the result of the guess");

        // cut out the empty part and turn the byte code into a string
        let guessed_res = String::from_utf8_lossy(&guessed_buf);
        let (res, _) = guessed_res.split_at(guessed_res.find("\n").unwrap());

        // parse the string into a Value struct
        let v: Value = serde_json::from_str(res).expect("JSON parsing error (guess)");

        // check the returned message type
        if &v["type"] == "retry" {
            if let Value::Array(x) = &v["guesses"] {
                let last_guess = &x[x.len() - 1];
                if let Value::Array(marks) = &last_guess["marks"] {
                    // iterate through the latest marks array
                    for (ind, num) in marks.iter().enumerate() {
                        let word_v: Vec<char> = word.chars().collect();
                        match num.as_u64() {
                            Some(m) => {
                                if m == 2u64 {
                                    // add the char to the corresponding postion of the char array,
                                    // if mark is 2
                                    reg[ind] = word_v[ind];
                                } else if m == 1u64 {
                                    // add char to the HashSet, if mark is 1
                                    should_contain.insert(word_v[ind]);
                                }
                            }
                            None => {}
                        }
                    }
                } else {
                    return "JSON object formatting error".to_string();
                }
            } else {
                return "JSON object formatting error".to_string();
            }
        } else if &v["type"] == "bye" {
            let flag = v["flag"].to_string();
            let f_byte = flag.as_bytes();
            let strip_flag = String::from_utf8_lossy(&f_byte[1..f_byte.len() - 1]);
            // return the secret flag
            return strip_flag.to_string();
        } else {
            return format!("reveived wrong message: {}", v.to_string());
        }
    }
}

/**
 * This function is used to communicate with an unencrypted TCP server port
 * with given hostname, username, and port number
 */
fn unencrypted_tcp(host_name: &str, username: &str, port_num: &str) {
    // establish a TCP connection
    match TcpStream::connect(format!("{}:{}", host_name, port_num)) {
        Ok(mut stream) => {
            let mut data = "{\"type\":\"hello\",\"northeastern_username\":".to_string();
            data.push_str(&format!("\"{}\"", username));
            data.push_str("}");
            // send the "hello" message
            stream
                .write(format!("{}\n", data).as_bytes())
                .expect("failed to send the hello message");

            let mut buf = [0; 128];
            // read the response from the server
            stream
                .read(&mut buf)
                .expect("failed to read the start message");

            let hello_res = String::from_utf8_lossy(&buf);
            let (res, _) = hello_res.split_at(
                hello_res
                    .find('\n')
                    .expect("can't find a line breaker in the received message"),
            );

            let v: Value = serde_json::from_str(res).expect("JSON parsing failed");

            // get the session from the received message
            let id = v["id"].to_string();

            // pass the session id and socket stream to the find_flag() function
            // and print the result
            print!("{}", find_flag(id.clone(), stream));
        }
        Err(_) => {
            print!("can't connect to the unencrypted socket");
        }
    }
}

/**
 * This function is used to communicate with an encrypted TCP port
 * with given hostname, username and port number
 */
fn encrypted_tcp(host_name: String, username: &str, port_num: &str) {
    // set up root certificate store
    let mut root_store = rustls::RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    // set the client config
    let mut config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    config.key_log = Arc::new(rustls::KeyLogFile::new());

    let server_name = ServerName::try_from(host_name.clone()).expect("invalid hostname");
    let mut conn = rustls::ClientConnection::new(Arc::new(config), server_name).unwrap();

    // establish connection to the TCP server
    if let Ok(mut stream) = TcpStream::connect(format!("{}:{}", host_name, port_num)) {
        // pass the socket stream to the Stream struct
        let mut tls = rustls::Stream::new(&mut conn, &mut stream);
        let mut data = "{\"type\":\"hello\",\"northeastern_username\":".to_string();
        data.push_str(&format!("\"{}\"", username));
        data.push_str("}");

        // write the "hello" message to the server
        tls.write(format!("{}\n", data).as_bytes())
            .expect("failed to send the hello message");

        let mut buf = [0; 128];
        // read the response from the server
        tls.read(&mut buf)
            .expect("failed to read the start message");

        let hello_res = String::from_utf8_lossy(&buf);
        let (res, _) = hello_res.split_at(hello_res.find('\n').unwrap());

        let v: Value = serde_json::from_str(res).expect("JSON parsing failed");
        // get the session id from the received message
        let id = v["id"].to_string();

        // pass the session id and socket stream to the find_flag() function
        // and print the result
        print!("{}", find_flag(id.clone(), tls));
    } else {
        print!("can't connect to the encrypted socket");
    }
}

/**
 * This is the main drive of this Tcp client
 */
fn main() -> std::io::Result<()> {
    // get the command line arguments
    let args: Vec<String> = env::args().collect();

    // create the variables to store the parsed arguments
    let mut host_name = "".to_string();
    let mut username = "".to_string();
    let mut is_tls = false;
    let mut port_num = "".to_string();

    match args.len() {
        // valid len of the command line arguments
        3..=6 => {
            let n = args.len();
            let mut ind: usize = 1;
            while ind < n {
                match args[ind].as_str() {
                    // encounter -p flag
                    "-p" => match args.get(ind + 1) {
                        Some(x) => {
                            // check if the port numbers are all digits
                            for c in x.chars() {
                                if !c.is_ascii_digit() {
                                    writeln!(
                                        &mut std::io::stderr(),
                                        "-p flag with invalid port number"
                                    )?;
                                    return Ok(());
                                }
                            }

                            port_num.push_str(x.as_str());
                            ind += 2;
                        }
                        // if the port number is not provided after -p flag
                        None => {
                            writeln!(&mut std::io::stderr(), "-p flag without port number")?;
                            return Ok(());
                        }
                    },
                    // encounter -s flag
                    "-s" => {
                        is_tls = true;
                        ind += 1;
                    }
                    // fill the hostname first and then username
                    _ => {
                        if host_name.is_empty() && username.is_empty() {
                            host_name.push_str(args[ind].as_str());
                            ind += 1;
                        } else if username.is_empty() {
                            username.push_str(args[ind].as_str());
                            ind += 1;
                        } else {
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

    // check if both hostname and username are provided
    if host_name.is_empty() || username.is_empty() {
        writeln!(
            &mut std::io::stderr(),
            "missing hostname or Northeastern-username"
        )?;
        return Ok(());
    }

    // unencrypted or encrypted socket
    if is_tls {
        // if port number is not provided, use the default number
        if port_num.is_empty() {
            port_num = ENCRYPTIED_PORT.to_string();
        }
        encrypted_tcp(host_name.clone(), &username, &port_num);
    } else {
        // if port number is not provided, use the default number
        if port_num.is_empty() {
            port_num = UNENCRYPTIED_PORT.to_string();
        }
        unencrypted_tcp(&host_name, &username, &port_num);
    }
    Ok(())
}
