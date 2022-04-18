use std::sync::atomic::{AtomicI32, Ordering};

use async_graphql::{
    http::{playground_source, GraphQLPlaygroundConfig},
    Context, EmptyMutation, Object, Schema, Subscription,
};
use async_graphql_poem::{GraphQL, GraphQLSubscription};
use poem::{get, handler, listener::TcpListener, web::Html, IntoResponse, Route, Server};
use tokio::sync::broadcast;
use tokio_stream::{Stream, StreamExt};

struct State {
    seq: AtomicI32,
    sender: broadcast::Sender<i32>,
}

struct Query;

#[Object]
impl Query {
    async fn push(&self, ctx: &Context<'_>) -> bool {
        let state = ctx.data_unchecked::<State>();
        let value = state.seq.fetch_add(1, Ordering::SeqCst);
        let _ = state.sender.send(value);
        true
    }
}

struct Subscription;

#[Subscription]
impl Subscription {
    async fn subscribe(&self, ctx: &Context<'_>) -> impl Stream<Item = i32> {
        let state = ctx.data_unchecked::<State>();
        let receiver = state.sender.subscribe();
        tokio_stream::wrappers::BroadcastStream::new(receiver)
            .filter(|res| res.is_ok())
            .map(Result::unwrap)
    }
}

#[handler]
async fn graphql_playground() -> impl IntoResponse {
    Html(playground_source(
        GraphQLPlaygroundConfig::new("/graphql").subscription_endpoint("/ws"),
    ))
}

#[handler]
async fn index() -> impl IntoResponse {
    Html(include_str!("index.html"))
}

#[tokio::main]
async fn main() {
    let (sender, _) = broadcast::channel(32);
    let schema = Schema::build(Query, EmptyMutation, Subscription)
        .data(State {
            seq: AtomicI32::new(1),
            sender,
        })
        .finish();

    let app = Route::new()
        .at("/", get(index).post(GraphQL::new(schema.clone())))
        .at("/playground", get(graphql_playground))
        .at("/ws", get(GraphQLSubscription::new(schema)));

    Server::new(TcpListener::bind("0.0.0.0:3000"))
        .run(app)
        .await
        .unwrap();
}
