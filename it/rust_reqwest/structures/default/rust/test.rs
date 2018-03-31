use reqwest;
use reqwest::header::parsing;

#[derive(Serialize, Deserialize, Debug)]
pub struct Entry {
}

pub struct MyService_Reqwest{
  client: reqwest::Client,
  url: reqwest::Url,
}

impl MyService_Reqwest {
  pub fn new(client: reqwest::Client, url: Option<reqwest::Url>) -> reqwest::Result<Self> {
    let url = match url {
      Some(url) => url,
      None => reqwest::Url::parse("http://example.com")?,
    };

    Ok(Self {
      client,
      url,
    })
  }

  /// UNKNOWN
  pub fn unknown(&self, id: u32) -> reqwest::Result<()> {
    let mut path_ = String::new()
    path_.push_str("/");
    path_.push_str("unknown");
    path_.push_str("/");
    parsing::http_percent_encode(&mut path_, id.to_string().as_bytes())?;
    let url_ = self.url.join(path_)?;
    let req_ = self.client.request(reqwest::Method::Get, url_);
    req_.send()?;
    Ok(())
  }

  /// UNKNOWN
  pub fn unknown_return(&self, id: u32) -> reqwest::Result<Entry> {
    let mut path_ = String::new()
    path_.push_str("/");
    path_.push_str("unknown-return");
    path_.push_str("/");
    parsing::http_percent_encode(&mut path_, id.to_string().as_bytes())?;
    let url_ = self.url.join(path_)?;
    let req_ = self.client.request(reqwest::Method::Get, url_);
    let res_ = req_.send()?;
    res_.json()
  }

  /// UNKNOWN
  pub fn unknown_argument(&self, request: Entry, id: u32) -> reqwest::Result<()> {
    let mut path_ = String::new()
    path_.push_str("/");
    path_.push_str("unknown-argument");
    path_.push_str("/");
    parsing::http_percent_encode(&mut path_, id.to_string().as_bytes())?;
    let url_ = self.url.join(path_)?;
    let req_ = self.client.request(reqwest::Method::Get, url_);
    let req_ = req_.json(&request);
    req_.send()?;
    Ok(())
  }

  /// UNARY
  pub fn unary(&self, request: Entry, id: u32) -> reqwest::Result<Entry> {
    let mut path_ = String::new()
    path_.push_str("/");
    path_.push_str("unary");
    path_.push_str("/");
    parsing::http_percent_encode(&mut path_, id.to_string().as_bytes())?;
    let url_ = self.url.join(path_)?;
    let req_ = self.client.request(reqwest::Method::Get, url_);
    let req_ = req_.json(&request);
    let res_ = req_.send()?;
    res_.json()
  }
}
