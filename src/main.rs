mod utils;
use utils::threading::ThreadPool;
use std::{
  env, io::{self, BufRead, BufReader, BufWriter, Read, Write}, net::{TcpListener, TcpStream}, panic, str, sync::{Arc,Mutex}, thread
};
use sqlite;

struct Controller {
  db_path: String
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


struct Headers {
  headers: Vec<(String, String)>
}
impl Headers {
  fn get(&self, key: &str) -> String {
    for i in &self.headers {
      if i.0 == key {
        return i.1.to_string()
      }
    }
    String::new()
  }
  fn new(headers: Vec<(String, String)>) -> Headers {
    Headers { headers }
  }
}


fn parse_contents(contents: String) -> (Headers, String, String) {
  println!("{:#?}", contents);
  let split = contents.split("\r\n\n\r\n\n").collect::<Vec<&str>>();
  // ASSUME [HEADERS, PLAIN, PLAIN_TEXT, HTML, HTML_TEXT] unless proven otherwise.

  if split.len()>2 {
    let mut headers = split[0].split("\r\n\n").map(|i| i.to_string()).collect::<Vec<String>>();
    let mut offset = 0;
    for i in 1..headers.len() {
      let i = i-offset;
      if headers[i].starts_with(" ") {
        let part = headers[i].clone();
        headers[i-1] += &part;
        headers.remove(i);
        offset += 1;
      }
    }

    
    
    let headers = Headers::new(headers.iter().map(|x| {
      let splt = x.split(":").collect::<Vec<&str>>();
      let name = splt[0].trim().to_string();
      let mut value = String::new();
      if splt.len()==1 {
        return (name, value);
      }
      for i in &splt[1..] {
        value += i;
      }
      (name, value.trim().to_string())
    }).collect::<Vec<(String, String)>>());

    let ctype = headers.get("Content-Type");

    let mut plain = String::from("Couldn't load body [report!].");
    let mut html = String::from("Couldn't load body [report!].");
    if ctype != String::new() {
      let splt = ctype.split(";").collect::<Vec<&str>>();
      if splt[0].trim() == "multipart/alternative" {
        if splt.len()>1 {
          let boundary = splt[1].split("=").collect::<Vec<&str>>()[1];
          let mut boundary_c = boundary.chars();
          if boundary.starts_with("\"") && boundary.ends_with("\"") {
            boundary_c.next();
            boundary_c.next_back();
          }
          let boundary = boundary_c.collect::<String>();
          plain = String::new();
          html = String::new();
          for i in 1..split.len()-1 {
            if split[i].starts_with(&("--".to_owned() + &boundary)) {
              let splt = split[i].split("\r\n\n").collect::<Vec<&str>>();
              if splt.len()<2 {
                continue
              }
              let tmp = splt[1].to_string();
              if !tmp.contains(":") {
                continue
              }
              let ctype = &tmp.split(":").collect::<Vec<&str>>()[1].split(";").collect::<Vec<&str>>()[0].trim();
              if ctype == &"text/plain" {
                plain = split[i+1].to_string();
              } else if ctype == &"text/html" {
                html = split[i+1].to_string();
              }
            }
          }
        }
      }
    } else {
      if split.len() > 1 {
        plain = String::new();
        for i in &split[1..] {
          plain += i;
        }
        html = plain.clone();
      }
    }


    (headers, plain, html)
  } else {
    return (Headers::new(vec![]), contents.clone(), contents) //specify further, please
  }
}
const months: [&str; 12] = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
fn handle_email(from: String, rcpts: Vec<String>, contents: String, db_path: String) {
  let mut recipents = String::new();
  for i in rcpts {
    recipents.push_str(&(i + ", "));
  }
  let (headers, plain, html) = parse_contents(contents.clone());
  //println!("{:#?} \n ::::::: \n {:#?}", headers, body);
  let subject = headers.get("Subject");
  let date = headers.get("Date");
  println!("
NEW FOKMAIL
FROM: {from}
TO: {recipents}
DATE: {date}
SUBJECT: {subject}
CONTENTS:
{plain}
  ");

  let splt = date.split(" ").collect::<Vec<&str>>();
  let year = splt[3];
  let month = months.iter().position(|x| x==&splt[2][..3].to_lowercase()).unwrap()+1;
  let day = splt[1];
  let time = splt[4];

  let conn = sqlite::open(&db_path).unwrap();
  let query = &format!("
    INSERT INTO mails VALUES ('{subject}', '{plain}', '{html}', '{from}', '{recipents}', '{year}-{month}-{day} {time}');
  ");
  conn.execute(query).unwrap();
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
          let lock = controller.lock();
          let contr = lock.unwrap();
          handle_email(from.unwrap(), rcpts, message, contr.db_path.clone());
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
    pool.execute(|| {
      let panic = panic::catch_unwind(||handle_connection(stream, clone));
      if panic.is_ok() {
        panic.unwrap()
      }
    });
  }
}

fn create_account(mail: String, hash: String, db_path: String) {
  let conn = sqlite::open(":mailserver:").unwrap();
  let query = &format!("
    INSERT INTO accounts VALUES ({mail}, {hash});
  ");
  conn.execute(query).unwrap();
}

fn estabilish_database(db_path: String) {
  let conn = sqlite::open(&db_path).unwrap();
  let query = "
    CREATE TABLE IF NOT EXISTS mails (subject TEXT, contents TEXT, html TEXT, sender TEXT, recipent TEXT, date DATE);
  ";
  conn.execute(query).unwrap();
}

fn main() {
  let args = env::args().collect::<Vec<String>>();
  let mut db_path = ":mailserver:".to_string();
  for i in 0..args.len() {
    if args[i]=="--db" {
      db_path = args[i+1].to_string();
    }
  }
  estabilish_database(db_path.clone());
  let mut controller_raw = Controller {db_path: db_path.clone()};
  let mut controller = Arc::new(Mutex::new(controller_raw));
  /*thread::spawn(|| */estabilish_listener("0.0.0.0:25", controller)//);

  /*loop {
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    let input = input.trim().split(" ").collect::<Vec<&str>>();
    match input[0] {
      "new_account" => {
        create_account(input[1].to_string(), input[2].to_string(), db_path.clone());
      }
      _ => {}
    }
  }*/
}
