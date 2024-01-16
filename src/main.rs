use serde_json::{json, Result, Value};
use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::time::Duration;
use std::{
    io::{Error, Read, Write},
    net::{Ipv4Addr, Shutdown, SocketAddrV4, TcpStream},
};

use regex::Regex;

const HOST: &str = "proj1.3700.network:27993";

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct Message {
    r#type: String,
    id: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct Hello {
    r#type: String,
    northeastern_username: String,
}

fn make_a_guess(id: &String, word: String) -> String {
    let mut res = String::from("{");
    res.push_str(format!("\"type\":\"guess\",\"id\":{},\"word\":\"{}\"", id, word).as_str());
    res.push_str("}\n");
    res
}

fn load_words() -> String {
    let mut f = File::open("words.txt").expect("failed to open the words file");
    let mut words = vec![];

    f.read_to_end(&mut words).expect("failed to read file");

    return String::from_utf8_lossy(&words).to_string();
}

fn main() -> std::io::Result<()> {

    match TcpStream::connect("proj1.3700.network:27993") {
        Ok(mut stream) => {
            let data = r#"{"type": "hello","northeastern_username": "lee.chih-"}"#;
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

            let large_words: String = load_words();
            let words_vec: Vec<&str> = large_words.split("\n").collect();
            let mut words_pool: HashSet<&str> = words_vec.into_iter().collect();

            let id = v["id"].to_string();
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
                // re.is_match(word);
                // dbg!(words_pool.len());

                let word = words_pool.clone().drain().next().unwrap();
                words_pool.remove(word);
                dbg!(&word);

                stream
                    .write(make_a_guess(&id, word.to_string()).as_bytes())
                    .expect("failed to send a guess message");


                let mut guessed_buf = [0; 18932];
                stream
                    .read(&mut guessed_buf)
                    .expect("failed to receive the result of the guess");
                let guessed_res = String::from_utf8_lossy(&guessed_buf);

                let (res, _) = guessed_res.split_at(guessed_res.find("\n").unwrap());
                dbg!(res.len());
                let v: Value = serde_json::from_str(res).expect("JSON parsing error (guess)");

                if &v["type"] == "retry" {
                    if let Value::Array(x) = &v["guesses"] {
                        let last_guess = &x[x.len() - 1];
                        if let Value::Array(marks) = &last_guess["marks"] {
      
                            for (ind , num) in marks.iter().enumerate() {
                                let word_v: Vec<char> = word.chars().collect();
                                match num.as_u64() {
                                    Some(m) => {
                                        if m == 2u64 {
                                            reg[ind] = word_v[ind];
                                        } else if m == 1u64 {
                                            should_contain.insert(word_v[ind]);
                                        }
                                    },
                                    None => {}
                                }
                            }
                        }
                        
                    }
                } else {
                    println!("flag: {}, answer: {}", &v["flag"], &word);
                    break;
                }
            }

            
        }
        Err(_) => {}
    }

    Ok(())
}

// use std::io::{Read, Write};
// use std::net::TcpStream;

// fn main() {
//     // Replace "proj1.3700.network" and 27993 with your target hostname and port
//     let host = "proj1.3700.network";
//     let port = 27993;

//     // Your JSON object
//     let json_object = r#"{"type":"hello", "northeastern_username":"lee.chih-"}"#;

//     // Convert JSON to a string with a line feed at the end
//     let json_string = format!("{}\n", json_object);

//     // Create a TcpStream and connect
//     if let Ok(mut stream) = TcpStream::connect(format!("{}:{}", host, port)) {
//         // Send the request
//         stream.write_all(json_string.as_bytes()).expect("Failed to write to stream");

//         // Read the response
//         let mut buffer = [0; 1024];
//         stream.read(&mut buffer).expect("Failed to read from stream");

//         // Print the response as a string
//         println!("Received {:?}", String::from_utf8_lossy(&buffer));
//     } else {
//         eprintln!("Failed to connect to the server");
//     }
// }
