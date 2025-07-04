mod utils;
use utils::threading::ThreadPool;
use std::{
  env, fs::{self, File}, io::{self, BufRead, BufReader, BufWriter, Read, Write}, net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs}, panic, process::{Command, Stdio}, str, sync::{Arc,Mutex}, thread
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


fn parse_contents(contents: String) -> (Headers, Headers) {
  //println!("{:#?}", contents);
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

    /*let mut plain = String::from("Couldn't load body [report!].");
    let mut html = String::from("Couldn't load body [report!].");*/

    let mut contents = vec![];

    if ctype != String::new() {
      let splt = ctype.split(";").collect::<Vec<&str>>();
      println!("{}", splt[0].trim());
      if splt[0].trim() == "multipart/alternative" {
        if splt.len()>1 {
          let boundary = splt[1].split("=").collect::<Vec<&str>>()[1];
          let mut boundary_c = boundary.chars();
          if boundary.starts_with("\"") && boundary.ends_with("\"") {
            boundary_c.next();
            boundary_c.next_back();
          }
          let boundary = boundary_c.collect::<String>();
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
              println!("{ctype}");
              contents.push((ctype.to_string(), split[i+1].to_string()));
            }
          }
        }
      }
    } else {
      if split.len() > 1 {
        let mut plain = String::new();
        for i in &split[1..] {
          plain += i;
        }
        contents.push(("text/plain".to_string(), plain));
      }
    }

    println!("{:#?}", contents);
    (headers, Headers::new(contents))
  } else {
    return (Headers::new(vec![]), Headers::new(vec![("unknown".to_string(), contents)])) //specify further, please
  }
}
const months: [&str; 12] = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
fn handle_email(from: String, rcpts: Vec<String>, rcontents: String, db_path: String) {
  let mut recipents = String::new();
  for i in rcpts {
    recipents.push_str(&(i + ", "));
  }
  let (headers, contents) = parse_contents(rcontents.clone());
  //println!("{:#?} \n ::::::: \n {:#?}", headers, body);
  let subject = headers.get("Subject");
  let date = headers.get("Date");
  
  let mut plain = String::from("no text.");

  let plain_t = contents.get("text/plain");
  if plain_t != String::new() {
    plain = plain_t
  }

  println!("
NEW FOKMAIL
FROM: {from}
TO: {recipents}
DATE: {date}
SUBJECT: {subject}
CONTENTS:
{plain}
  ");
  println!("\nAlso contains:");
  for i in contents.headers {
    println!("{}", i.0);
  }
  println!("\n");

  let splt = date.split(" ").collect::<Vec<&str>>();
  let year = splt[3];
  let month = months.iter().position(|x| x==&splt[2][..3].to_lowercase()).unwrap()+1;
  let day = splt[1];
  let time = splt[4];

  let conn = sqlite::open(&db_path).unwrap();
  let query = &format!("
    INSERT INTO mails VALUES ('{subject}', '{rcontents}', '{from}', '{recipents}', '{year}-{month}-{day} {time}');
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

/*fn create_account(mail: String, hash: String, db_path: String) {
  let conn = sqlite::open(":mailserver:").unwrap();
  let query = &format!("
    INSERT INTO accounts VALUES ({mail}, {hash});
  ");
  conn.execute(query).unwrap();
}*/

fn estabilish_database(db_path: String) {
  let conn = sqlite::open(&db_path).unwrap();
  let query = "
    CREATE TABLE IF NOT EXISTS mails (subject TEXT, contents TEXT, sender TEXT, recipent TEXT, date DATE);
  ";
  conn.execute(query).unwrap();
}

fn compute_bh(body: String) -> String {
  let mut file = File::create("body.txt").unwrap();
  let normalized = body.replace("\n", "\r\n").trim_end_matches("\r\n").to_string();
  file.write_all(normalized.as_bytes()).unwrap();

  let output = Command::new("openssl")
    .args(&["dgst", "-sha256", "-binary", "body.txt"])
    .output()
    .expect("failed to execute openssl");

  // base64 encode using openssl base64 (or Rust manual if you want)
  let base64_output = Command::new("openssl")
    .args(&["base64", "-A"])
    .stdin(std::process::Stdio::piped())
    .stdout(std::process::Stdio::piped())
    .spawn()
    .and_then(|mut child| {
      let stdin = child.stdin.as_mut().unwrap();
      stdin.write_all(&output.stdout)?;
      let output = child.wait_with_output()?;
      Ok(output)
    })
    .expect("failed to base64 encode");

  String::from_utf8(base64_output.stdout).unwrap().trim().to_string()

}

fn compute_b(headers: &str, sig: &str) -> String{
  let mut result = String::new();
  for i in headers.split("\n") {
    let splt = i.split(":").collect::<Vec<&str>>();
    result += &splt[0].to_lowercase();
    //result += ":";
    for i in &splt[1..] {
      result += ":";
      result += i;
    }
    result += "\r\n";
  }
  let splt = sig.split(":").collect::<Vec<&str>>();
  result += &splt[0].to_lowercase();
  for i in &splt[1..] {
    result += ":";
    result += i;
  }

  let mut file = File::create("signing_string.txt").unwrap();
  file.write_all(result.as_bytes()).unwrap();

  let signed_output = Command::new("openssl")
    .args(&["dgst", "-sha256", "-sign", "/home/foko/KEYS/dkim_private.key", "signing_string.txt"])
    .output()
    .expect("Failed to sign DKIM headers");

  if !signed_output.status.success() {
    panic!("OpenSSL sign failed: {:?}", signed_output.stderr);
  }

  // Step 2: Base64-encode the binary signature
  let mut base64 = Command::new("openssl")
    .args(&["base64", "-A"])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .spawn()
    .expect("Failed to start base64 command");

  {
    let stdin = base64.stdin.as_mut().expect("Failed to open stdin");
    stdin.write_all(&signed_output.stdout).expect("Failed to write signature to base64");
  }

  let output = base64.wait_with_output().expect("Failed to read base64 output");
  if !output.status.success() {
    panic!("Base64 failed: {:?}", output.stderr);
  }

  // Convert to String and trim
  String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn send_email(from: String, to: String, subject: String, body: String) -> Result<(),io::Error> {
  let server = to.split("@").collect::<Vec<&str>>()[1].to_string() + ":25";
  println!("{server}");
  let sock = server.to_socket_addrs().unwrap().next().unwrap();
  println!("{:#?}", sock);
  let conn = TcpStream::connect(&sock)?;
  
  let mut reader = BufReader::new(conn.try_clone().unwrap());
  let mut writer = BufWriter::new(conn);
  let mut buf = String::new();
  reader.read_line(&mut buf)?;
  println!("=={buf}==");

  writer.write(b"HELO hetman.at\r\n")?;
  writer.flush()?;
  reader.read_line(&mut buf).unwrap();

  writer.write(&format!("MAIL FROM: <{from}>\r\n").as_bytes())?;
  writer.flush()?;
  reader.read_line(&mut buf)?;


  writer.write(&format!("RCPT TO:<{to}>\r\n").as_bytes())?;
  writer.flush()?;
  reader.read_line(&mut buf)?;

  // DATA
  writer.write(b"DATA\r\n")?;
  writer.flush()?;
  reader.read_line(&mut buf)?;

  let headers = &format!("From: You <{from}>
To: <{to}>
Subject: {subject}
Date: Fri, 27 Jun 2025 15:00:00 +0200");

  let bh = compute_bh(body.clone());
  let sig = &format!("DKIM-Signature: v=1; a=rsa-sha256; d=hetman.at; s=mail; h=from:to:subject:date; bh={bh}; b=");
  let b = compute_b(headers, sig);
  // Build email body with headers, including DKIM
  let email = format!("{sig}{b}

{headers}

{body}
");

  writer.write(email.as_bytes())?;
  writer.write(b"\r\n.\r\n")?; // End of DATA
  writer.flush()?;
  reader.read_line(&mut buf)?;

  // QUIT
  writer.write(b"QUIT\r\n")?;
  writer.flush()?;
  println!("{:#?}", buf);
  Ok(())

}

fn main() {
  let args = env::args().collect::<Vec<String>>();
  let mut db_path = ":mailserver:".to_string();
  for i in 0..args.len() {
    if args[i]=="--db" {
      db_path = args[i+1].to_string();
    }
    if args[i]=="--send" {
      send_email(args[i+1].clone(), args[i+2].clone(), args[i+3].clone(), args[i+4].clone());
      return
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
