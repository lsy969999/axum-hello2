use std::{time::Duration, env, fmt::{Formatter, Display}};

use axum::{Router, extract::{Path, State, FromRequestParts}, http::{StatusCode, request::Parts}, routing::{get, post}, response::{IntoResponse, Html, Response}, Json, RequestPartsExt, async_trait};
use axum_extra::{TypedHeader, headers::{Authorization, authorization::Bearer}};
use dotenv::dotenv;
use jsonwebtoken::{EncodingKey, DecodingKey, Validation, decode, encode, Header};
use once_cell::sync::Lazy;
use serde::{Serialize, Deserialize};
use serde_json::json;
use sqlx::{postgres::PgPoolOptions, PgPool};
use tokio::net::TcpListener;
use tower_http::services::ServeDir;
use tracing::{debug, Level};
use askama::Template;

static KEYS: Lazy<Keys> = Lazy::new(|| {
  let secret = env::var("JWT_SECRET").expect("JWT_SECRET must be set");
  Keys::new(secret.as_bytes())
});

#[tokio::main]
async fn main() {
    dotenv().ok();
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
                                  .with_max_level(Level::TRACE)
                                  .finish();
    tracing::subscriber::set_global_default(subscriber)
    .expect("setting default subscriber fialed");

    let database_url: String = env::var("DATABASE_URL").unwrap();
    let pool = PgPoolOptions::new()
                  .max_connections(5)
                  .acquire_timeout(Duration::from_secs(3))
                  .connect(&database_url)
                  .await
                  .expect("can't connect to database");

    let app = Router::new()
                .route("/", get(idx))
                .route("/messages", get(||async {
                  Html("<span class='test'>haha</span><script>console.log('dudu');</script>")
                }))
                .route("/greet/:name", get(greet))
                .route("/sample", get(get_sample_entity))
                .route("/authorize", post(authorize))
                .route("/protected", get(protected))
                .nest_service("/assets", ServeDir::new("assets"))
                .with_state(pool);

    let listener = TcpListener::bind("127.0.0.1:3000")
          .await
          .unwrap();
    
    debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();

}
#[derive(Template)]
#[template(path="index.html")]
struct IdxTemplate{}
async fn idx()-> impl IntoResponse{
  HtmlTemplate(IdxTemplate{})
}

#[derive(Template)]
#[template(path="hello.html")]
struct HelloTemplate {
  name: String,
}

async fn greet(Path(name): Path<String>) -> impl IntoResponse {
  // debug!("name: {name}");
  let template = HelloTemplate {name};
  HtmlTemplate(template)
}

struct HtmlTemplate<T>(T);

impl<T> IntoResponse for HtmlTemplate<T>
where
    T: Template,
{
    fn into_response(self) -> Response {
        match self.0.render() {
            Ok(html) => Html(html).into_response(),
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to render template. Error: {err}"),
            )
                .into_response(),
        }
    }
}

#[derive(Debug, Serialize)]
struct SampleEntity {
  id: i64,
  name: Option<String>
}

async fn get_sample_entity(State(pool): State<PgPool>) -> impl IntoResponse{
  let se = sqlx::query_as!(SampleEntity, "SELECT * FROM sample_entity").fetch_all(&pool).await;
  match se {
      Ok(r)=>Json(r).into_response(),
      Err(_)=>StatusCode::INTERNAL_SERVER_ERROR.into_response()
  }
}

struct Keys {
  encoding: EncodingKey,
  decoding: DecodingKey,
}

impl Keys {
    fn new(secret: &[u8]) -> Self {
      Self {
        encoding: EncodingKey::from_secret(secret),
        decoding: DecodingKey::from_secret(secret)
      }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
  sub: String,
  company: String,
  exp: usize
}

impl Display for Claims {
    fn fmt(&self, f: &mut Formatter<'_>) ->  std::fmt::Result {
      write!(f, "Email: {} Company: {}", self.sub, self.company)
    }
}

#[derive(Debug, Serialize)]
struct AuthBody{
  access_token: String,
  token_type: String,
}

impl AuthBody {
    fn new(access_token: String) -> Self {
      Self {
        access_token,
        token_type: "Bearer".to_string()
      }
    }
}

#[derive(Debug, Deserialize)]
struct AuthPayload {
  client_id: String,
  client_secret: String,
}

#[derive(Debug)]
enum AuthError {
  WrongCredentials,
  MissingCredentials,
  TokenCreation,
  InvalidToken
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
          AuthError::WrongCredentials => (StatusCode::UNAUTHORIZED, "Wrong credentials"),
          AuthError::MissingCredentials => (StatusCode::BAD_REQUEST, "Missing credentials"),
          AuthError::TokenCreation => (StatusCode::INTERNAL_SERVER_ERROR, "Token creation error"),
          AuthError::InvalidToken => (StatusCode::BAD_REQUEST, "Invalid Token")
        };
        let body = Json(json!({
          "error": error_message
        }));
        (status, body).into_response()
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for Claims where S: Send + Sync{
  type Rejection = AuthError;
  async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        // Extract the token from the authorization header
        let TypedHeader(Authorization(bearer)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|_| AuthError::InvalidToken)?;
        // Decode the user data
        let token_data = decode::<Claims>(bearer.token(), &KEYS.decoding, &Validation::default())
            .map_err(|_| AuthError::InvalidToken)?;

        Ok(token_data.claims)
  }
}


async fn protected(claims: Claims) -> Result<String, AuthError> {
  Ok(format!(
    "Welecom to the protected area! your data: {claims}"
  ))
}

async fn authorize(Json(payload): Json<AuthPayload>)->Result<Json<AuthBody>, AuthError> {
  if payload.client_id.is_empty() || payload.client_secret.is_empty() {
    return Err(AuthError::MissingCredentials);
  }

  if payload.client_id != "foo" || payload.client_secret != "bar" {
    return Err(AuthError::WrongCredentials);
  }

  let claims = Claims {
    sub: "b@b.com".to_owned(),
    company: "ACME".to_owned(),
    exp: 2000000000
  };

  let token = encode(&Header::default(), &claims, &KEYS.encoding).map_err(|_| AuthError::TokenCreation)?;

  Ok(Json(AuthBody::new(token)))
}