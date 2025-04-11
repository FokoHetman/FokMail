mod utils;
use utils::threading::ThreadPool;
use std::{
  io::{self, BufRead, BufReader, BufWriter, Read, Write}, net::{TcpListener, TcpStream}, str, sync::{Arc,Mutex}
};

struct Controller {

}


#[derive(PartialEq)]
enum STMTState {
  Command,
  Data,
  Quit,
}


macro_rules! respond {
    ($text:expr, $writer:expr) => {{
        use std::io::Write;
        $writer.write_all(($text.to_string() + "\r\n").as_bytes()).unwrap();
        $writer.flush().unwrap();
    }};
}


fn handle_email(from: String, rcpts: Vec<String>, message: String) {
  let mut recipents = String::new();
  for i in rcpts {
    recipents.push_str(&(i + ", "));
  }
  println!("
NEW FOKMAIL
FROM: {from}
TO: {recipents}
CONTENTS:
{message}
  ");
}

fn handle_connection(mut stream: TcpStream, controller: Arc<Mutex<Controller>>) {
  let (reader, writer) = (stream.try_clone().unwrap(), stream);
  let mut reader = BufReader::new(reader);
  let mut writer = BufWriter::new(writer);
  writer.write_all(b"220 FokMail Server\r\n").unwrap();
  writer.flush().unwrap();

  

  
  let mut state = STMTState::Command;
  let mut from: Option<String> = None;
  let mut rcpts: Vec<String> = vec![];
  let mut message = String::new();

  'main_loop: while state != STMTState::Quit {
    let mut buffer = String::new();
    reader.read_line(&mut buffer).unwrap();
    match state {
      STMTState::Command => {
    
        let cmd = buffer.trim().split(" ").collect::<Vec<&str>>();
        match cmd[0] {
          "EHLO" | "HELO" => {
            respond!("250 Hello", writer);
          }
          "QUIT" => {
            respond!("221 Bye", writer);
            state = STMTState::Quit;
          }
          "MAIL" => {
            for i in cmd[1..].iter() {
              if i.trim().to_lowercase().starts_with("from:") {
                from = Some(i[5..].trim().to_string());
                respond!("250 OK", writer);
                continue 'main_loop
              }
            }
            respond!("501 Can't find FROM clause", writer);
          }
          "RCPT" => {
            if from.is_none() {
              respond!("503 Error: Send MAIL first", writer);
              continue 'main_loop
            }
            for i in cmd[1..].iter() {
              if i.trim().to_lowercase().starts_with("to:") {
                rcpts.push(i[3..].trim().to_string());
                respond!("250 OK", writer);
                continue 'main_loop
              }
            }
          }
          "DATA" => {
            if rcpts.is_empty() {
              respond!("503 Error: Send RCPT first",writer);
              continue 'main_loop
            }
            state = STMTState::Data;
            respond!("354 End data with <CR><LF>.<CR><LF>", writer);
          }
          _ => {
            respond!(format!("500 Unknown Command: {}", cmd[0]), writer)
          }
        }
      }
      STMTState::Data => {
        if buffer.trim() == "." {
          respond!("250 OK", writer);
          handle_email(from.unwrap(), rcpts, message);
          from = None;
          rcpts = vec![];
          message = String::new();
          state = STMTState::Command;
          continue 'main_loop
        }
        message.push_str(&(buffer + "\n"));
      }
      _ => {}
    }
  }
}


fn estabilish_listener(ip: &str, controller: Arc<Mutex<Controller>>) {
  let listener = TcpListener::bind(ip).unwrap();
  println!("Listening on http://{ip}");
  let pool = ThreadPool::new(8);

  for stream in listener.incoming() {
    let stream = stream.unwrap();
    let clone = Arc::clone(&controller);
    pool.execute(|| {let _ = handle_connection(stream, clone);});
  }
}

fn main() {
  let mut controller_raw = Controller {};
  let mut controller = Arc::new(Mutex::new(controller_raw));
  estabilish_listener("0.0.0.0:25", controller);
}
