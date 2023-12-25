use axum::{Router, extract::Path, http::StatusCode, routing::get, response::{IntoResponse, Html, Response}};
use tokio::net::TcpListener;
use tracing::{debug, Level};
use askama::Template;

#[tokio::main]
async fn main() {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
                                  .with_max_level(Level::TRACE)
                                  .finish();
    tracing::subscriber::set_global_default(subscriber)
    .expect("setting default subscriber fialed");

      let app = Router::new()
                  .route("/greet/:name", get(greet));

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
  debug!("name: {name}");
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