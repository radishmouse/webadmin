use gloo_net::http::{Headers, Method, RequestBuilder};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub struct HttpRequest {
    method: Method,
    url: form_urlencoded::Serializer<'static, String>,
    headers: Headers,
    body: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum Response<T> {
    Data { data: T },
    Error { error: String, details: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Error {
    Unauthorized,
    Network(String),
    Serializer { error: String, response: String },
    Server { error: String, details: String },
}

pub type Result<T> = std::result::Result<T, Error>;

impl<'x> HttpRequest {
    pub fn new(method: Method, url: impl Into<String>) -> Self {
        Self {
            method,
            url: form_urlencoded::Serializer::new(url.into()),
            headers: Headers::new(),
            body: None,
        }
    }

    pub fn get(url: impl Into<String>) -> Self {
        Self::new(Method::GET, url)
    }

    pub fn post(url: impl Into<String>) -> Self {
        Self::new(Method::POST, url)
    }

    pub fn put(url: impl Into<String>) -> Self {
        Self::new(Method::PUT, url)
    }

    pub fn delete(url: impl Into<String>) -> Self {
        Self::new(Method::DELETE, url)
    }

    pub fn patch(url: impl Into<String>) -> Self {
        Self::new(Method::PATCH, url)
    }

    pub fn with_parameter(mut self, key: impl AsRef<str>, value: impl AsRef<str>) -> Self {
        self.url.append_pair(key.as_ref(), value.as_ref());
        self
    }

    pub fn with_optional_parameter(
        mut self,
        key: impl AsRef<str>,
        value: Option<impl AsRef<str>>,
    ) -> Self {
        if let Some(value) = value {
            self.url.append_pair(key.as_ref(), value.as_ref());
        }
        self
    }

    pub fn with_header(self, name: impl AsRef<str>, value: impl AsRef<str>) -> Self {
        self.headers.set(name.as_ref(), value.as_ref());
        self
    }

    pub fn with_body<B: Serialize>(mut self, body: B) -> Result<Self> {
        match serde_json::to_string(&body) {
            Ok(body) => {
                self.body = Some(body);
                Ok(self)
            }
            Err(err) => Err(Error::Serializer {
                error: err.to_string(),
                response: "".to_string(),
            }),
        }
    }

    pub fn with_raw_body(mut self, body: String) -> Self {
        self.body = Some(body);
        self
    }

    pub async fn send<T>(self) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let response = self.send_raw().await?;
        match serde_json::from_slice::<Response<T>>(response.as_bytes()) {
            Ok(Response::Data { data }) => Ok(data),
            Ok(Response::Error { error, details }) => Err(Error::Server { error, details }),
            Err(err) => Err(Error::Serializer {
                error: err.to_string(),
                response,
            }),
        }
    }

    pub async fn send_raw(mut self) -> Result<String> {
        let abort_controller = web_sys::AbortController::new().ok();
        let abort_signal = abort_controller.as_ref().map(|a| a.signal());

        // abort in-flight requests if, e.g., we've navigated away from this page
        leptos::on_cleanup(move || {
            if let Some(abort_controller) = abort_controller {
                abort_controller.abort()
            }
        });

        let builder = RequestBuilder::new(&self.url.finish())
            .method(self.method)
            .headers(self.headers)
            .abort_signal(abort_signal.as_ref());
        let req = if let Some(body) = self.body {
            builder.body(body)
        } else {
            builder.build()
        }?;

        let response = req.send().await?;

        match response.status() {
            200..=299 => response.text().await.map_err(Into::into),
            401 => Err(Error::Unauthorized),
            code => Err(Error::Server {
                error: format!("Invalid response code {code}"),
                details: response.status_text(),
            }),
        }
    }
}

impl From<gloo_net::Error> for Error {
    fn from(err: gloo_net::Error) -> Self {
        Error::Network(format!("HTTP request failed: {err}"))
    }
}
