use std::{fs, path::Path, process::Command, time::UNIX_EPOCH};
use crate::utils::language::{self,Fructa};


static UNKNOWN_IMAGE_PATH: &str = "assets/whar.png";


pub fn render_gitrepo(path: &str, args: Vec<(String, Fructa)>) -> String {
  let mut args = args;

  let creation_date = (Path::new(path).metadata().unwrap().created().unwrap().duration_since(UNIX_EPOCH).unwrap().as_secs() / 31536000) + 1970;

  let root = path.to_owned();
  let desc_file = fs::read_to_string(root.clone() + "/description").unwrap().to_owned();
  let splt = desc_file.split(";").collect::<Vec<&str>>();

  let name = Box::new(Fructa::Str(splt[0].to_string()));
  let desc = Box::new(Fructa::Str(splt[1].to_string()));

  let tags;
  if splt.len()>2 {
    tags = splt[2].split("|").collect::<Vec<&str>>().iter().map(|x|

                                //      [Tag, Color]
      Box::new(Fructa::Inventarii(vec![Box::new(Fructa::Str(x.split(":").collect::<Vec<&str>>()[0].to_string())), Box::new(Fructa::Str(x.split(":").collect::<Vec<&str>>()[1].trim().to_string()))]))

    ).collect::<Vec<Box<Fructa>>>();
  } else {
    tags = vec![];
  }

  let parts = root.split("/").collect::<Vec<&str>>();
  let id = parts.last().unwrap();
  let mut parts = parts.clone();
  parts.pop();
  let user = parts.last().unwrap();

  let img_path;
  let pathe = root.clone() + "/banner.png";
  if Path::new(&pathe).exists() {
    img_path = "dynamic/".to_owned() + id + "_banner.png";
    Command::new("cp").arg(pathe).arg("static/".to_owned() + &img_path).output().unwrap().stdout;
  } else {
    img_path = UNKNOWN_IMAGE_PATH.to_string();
  }



  args.push(("repo".to_string(), Fructa::Dictario(vec![
    (Box::new(Fructa::Str(String::from("id"))), Box::new(Fructa::Str(id.to_string()))),
    (Box::new(Fructa::Str(String::from("user"))), Box::new(Fructa::Str(user.to_string()))),
    (Box::new(Fructa::Str(String::from("name"))), name),
    (Box::new(Fructa::Str(String::from("desc"))), desc),
    (Box::new(Fructa::Str(String::from("tags"))), Box::new(Fructa::Inventarii(tags))),

    (Box::new(Fructa::Str(String::from("img_path"))), Box::new(Fructa::Str(  img_path  ))),
    (Box::new(Fructa::Str(String::from("date"))), Box::new(Fructa::Str( creation_date.to_string() ))),

  ])));
  render_template("templates/repo.html", args)
}

pub fn render_gituser(path: &str, args: Vec<(String, Fructa)>) -> String {
  let mut repositories: Vec<Box<Fructa>> = vec![];
  for i in fs::read_dir(path).unwrap() {
    let i = i.unwrap();
    let creation_date = (i.metadata().unwrap().created().unwrap().duration_since(UNIX_EPOCH).unwrap().as_secs() / 31536000) + 1970;
    let root = i.path().to_str().unwrap().to_owned();
    let desc_file = fs::read_to_string(root.clone() + "/description").unwrap().to_owned();
    let splt = desc_file.split(";").collect::<Vec<&str>>();
    
    let parts = root.split("/").collect::<Vec<&str>>();
    let id = parts.last().unwrap();
    let mut parts = parts.clone();
    parts.pop();
    let user = parts.last().unwrap();


    let img_path;
    let pathe = root.clone() + "/banner.png";

    if Path::new(&pathe).exists() {
      img_path = "dynamic/".to_owned() + id + "_banner.png";
      Command::new("cp").arg(pathe).arg("static/".to_owned() + &img_path).output().unwrap().stdout;
    } else {
      img_path = UNKNOWN_IMAGE_PATH.to_string();
    }

    let tags;
    if splt.len()>2 {
      tags = splt[2].split("|").collect::<Vec<&str>>().iter().map(|x|

        //      [Tag, Color]
        Box::new(Fructa::Inventarii(vec![Box::new(Fructa::Str(x.split(":").collect::<Vec<&str>>()[0].to_string())), Box::new(Fructa::Str(x.split(":").collect::<Vec<&str>>()[1].trim().to_string()))]))

      ).collect::<Vec<Box<Fructa>>>();
    } else {
      tags = vec![];
    }
    

    repositories.push(Box::new(
      Fructa::Dictario(vec![
        (Box::new(Fructa::Str(String::from("id"))), Box::new(Fructa::Str(id.to_string()))),
        (Box::new(Fructa::Str(String::from("user"))), Box::new(Fructa::Str(user.to_string()))),
        (Box::new(Fructa::Str(String::from("name"))), Box::new(Fructa::Str(splt[0].to_string()))),
        (Box::new(Fructa::Str(String::from("desc"))), Box::new(Fructa::Str(splt[1].to_string()))),

        (Box::new(Fructa::Str(String::from("tags"))), Box::new(Fructa::Inventarii(tags))),

        (Box::new(Fructa::Str(String::from("img_path"))), Box::new(Fructa::Str(  img_path  ))),
        (Box::new(Fructa::Str(String::from("date"))), Box::new(Fructa::Str( creation_date.to_string() ))),
      ]
      )
    ));
  };
  let mut args = args;
  args.push(("repos".to_string(), Fructa::Inventarii(repositories)));
  render_template("templates/git.html", args)
}











pub fn render_template(path: &str, args: Vec<(String, Fructa)>) -> String {
  let template = fs::read_to_string(path).unwrap();
  let tokens = tokenize_template(template.clone());
  return parse(tokens, args);
}

#[derive(Debug,PartialEq,Clone)]
pub enum TemplateToken {
  CodeBlock(String),
  Irrelevant(String),
}

pub fn tokenize_template(template: String) -> Vec<TemplateToken>{
  let mut result: Vec<TemplateToken> = vec![];

  let mut chars = template.chars().collect::<Vec<char>>();

  while chars.len() > 1 {
    let mut buffer = String::new();
    while chars.len()>1 && (chars[0] != '{' || chars[1] != '{') {
      buffer += &chars[0].to_string();
      chars.remove(0);
    }
    result.push(TemplateToken::Irrelevant(buffer));
    for i in 0..2 {
      if chars.len()>0 {
        chars.remove(0);
      }
    }
    let mut buffer = String::new();
    while chars.len()>1 && (chars[0] != '}' || chars[1] != '}') {
      buffer += &chars[0].to_string();
      chars.remove(0);
    }
    for i in 0..2 {
      if chars.len()>0 {
        chars.remove(0);
      }
    }

    result.push(TemplateToken::CodeBlock(buffer));
  }
  result.push(TemplateToken::Irrelevant(chars.into_iter().collect::<String>()));


  result
}

pub fn parse(tokens: Vec<TemplateToken>, args: Vec<(String, Fructa)>) -> String {
  let mut result = String::new();
  println!("{:#?}", tokens);
  let mut parser = language::Parser {tokens: vec![]};
  let mut env =  language::Env {data: args};
  for i in tokens {
    result += &match i {
      TemplateToken::Irrelevant(s) => s,
      TemplateToken::CodeBlock(s) => template_lang(s, &mut parser.clone(), &mut env),
    }
  }
  result
}

pub fn template_lang(string: String, parser: &mut language::Parser, env: &mut language::Env) -> String {
  let tokens = language::tokenize_lang(string);
  println!("{:#?}", tokens);
  
  language::parse_lang(tokens, parser, env)
}


