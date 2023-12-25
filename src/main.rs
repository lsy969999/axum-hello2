use std::{time::Duration, env};

use axum::{Router, extract::{Path, State}, http::StatusCode, routing::get, response::{IntoResponse, Html, Response}, Json};
use dotenv::dotenv;
use serde::Serialize;
use sqlx::{postgres::PgPoolOptions, PgPool};
use tokio::net::TcpListener;
use tracing::{debug, Level};
use askama::Template;

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
                .route("/greet/:name", get(greet))
                .route("/sample", get(get_sample_entity))
                .with_state(pool);

    let listener = TcpListener::bind("127.0.0.1:3000")
          .await
          .unwrap();
    
    debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();

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