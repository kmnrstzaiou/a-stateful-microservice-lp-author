#[macro_use]
extern crate lazy_static;

use std::{convert::Infallible, error::Error, net::SocketAddr, result::Result, str};

use hyper::{
    service::{make_service_fn, service_fn},
    Body, Method, Request, Response, Server, StatusCode,
};
pub use mysql_async::{
    params,
    prelude::{Query, WithParams},
    Opts, OptsBuilder, Pool, PoolConstraints, PoolOpts,
};
use serde::{Deserialize, Serialize};

lazy_static! {
    static ref SALES_TAX_RATE_SERVICE: String = {
        if let Ok(url) = std::env::var("SALES_TAX_RATE_SERVICE") {
            url
        } else {
            "http://localhost:8001/find_rate".into()
        }
    };
    static ref DATABASE_URL: String = {
        if let Ok(url) = std::env::var("DATABASE_URL") {
            url
        } else {
            "mysql://root:pass@127.0.0.1:3306/mysql".into()
        }
    };
}

#[derive(Serialize, Deserialize, Debug)]
struct Order {
    order_id: Option<i32>,
    product_id: i32,
    quantity: i32,
    subtotal: f32,
    shipping_address: String,
    shipping_zip: String,
    shipping_cost: f32,
    total: f32,
}

impl Order {
    #[allow(clippy::too_many_arguments)]
    fn new(
        order_id: Option<i32>,
        product_id: i32,
        quantity: i32,
        subtotal: f32,
        shipping_address: String,
        shipping_zip: String,
        shipping_cost: f32,
        total: f32,
    ) -> Self {
        Self {
            order_id,
            product_id,
            quantity,
            subtotal,
            shipping_address,
            shipping_zip,
            shipping_cost,
            total,
        }
    }
}

/// This is our service handler. It receives a Request, routes on its
/// path, and returns a Future of a Response.
async fn handle_request(req: Request<Body>, pool: Pool) -> Result<Response<Body>, anyhow::Error> {
    match (req.method(), req.uri().path()) {
        // CORS OPTIONS
        (&Method::OPTIONS, "/init") => Ok(response_build("")),
        (&Method::OPTIONS, "/create_order") => Ok(response_build("")),
        (&Method::OPTIONS, "/orders") => Ok(response_build("")),

        // Serve some instructions at /
        (&Method::GET, "/") => Ok(Response::new(Body::from(
            "Try to GET /init such as: `curl localhost:8003/init`",
        ))),

        (&Method::GET, "/init") => {
            let mut conn = pool.get_conn().await.map_err(|e| {
                dbg!(e.source());
                e
            })?;
            // "DROP TABLE IF EXISTS orders;".ignore(&mut conn).await?;
            "CREATE TABLE IF NOT EXISTS orders (order_id INT AUTO_INCREMENT PRIMARY KEY, product_id INT, quantity INT, subtotal FLOAT, shipping_address VARCHAR(1024), shipping_zip VARCHAR(32), shipping_cost FLOAT, total FLOAT);".ignore(&mut conn).await.map_err(|e| {
                dbg!(e.source());
                e
            })?;
            drop(conn);
            Ok(response_build("{\"status\":true}"))
        }

        (&Method::POST, "/create_order") => {
            let mut conn = pool.get_conn().await.unwrap();
            let byte_stream = hyper::body::to_bytes(req).await?;
            let mut order: Order = serde_json::from_slice(&byte_stream).unwrap();

            let client = reqwest::Client::new();
            let rate_resp = client
                .post(&*SALES_TAX_RATE_SERVICE)
                .body(order.shipping_zip.clone())
                .send()
                .await
                .map_err(|e| {
                    dbg!(e.source());
                    e
                })?;

            if rate_resp.status().is_success() {
                let rate = rate_resp.text().await?.parse::<f32>()?;
                order.total = order.subtotal * (1.0 + rate) + order.shipping_cost;

                let _ = "INSERT INTO orders (product_id, quantity, subtotal, shipping_address, shipping_zip, shipping_cost, total) VALUES (:product_id, :quantity, :subtotal, :shipping_address, :shipping_zip, :shipping_cost, :total)"
                    .with(params! {
                        "product_id" => order.product_id,
                        "quantity" => order.quantity,
                        "subtotal" => order.subtotal,
                        "shipping_address" => &order.shipping_address,
                        "shipping_zip" => &order.shipping_zip,
                        "shipping_cost" => order.shipping_cost,
                        "total" => order.total,
                    })
                    .ignore(&mut conn)
                    .await.map_err(|e| {
                    dbg!(e.source());
                    e
                });

                drop(conn);
                Ok(response_build(&serde_json::to_string_pretty(&order)?))
            } else if rate_resp.status() == StatusCode::NOT_FOUND {
                Ok(response_build(&String::from("{\"status\":\"error\", \"message\":\"The zip code in the order does not have a corresponding sales tax rate.\"}")))
            } else {
                Ok(response_build(&String::from("{\"status\":\"error\", \"message\":\"There is an unknown error from the sales tax rate lookup service.\"}")))
            }
        }

        (&Method::GET, "/orders") => {
            let mut conn = pool.get_conn().await.unwrap();

            let orders = "SELECT * FROM orders"
                .with(())
                .map(
                    &mut conn,
                    |(
                        order_id,
                        product_id,
                        quantity,
                        subtotal,
                        shipping_address,
                        shipping_zip,
                        shipping_cost,
                        total,
                    )| {
                        Order::new(
                            order_id,
                            product_id,
                            quantity,
                            subtotal,
                            shipping_address,
                            shipping_zip,
                            shipping_cost,
                            total,
                        )
                    },
                )
                .await?;

            drop(conn);
            Ok(response_build(serde_json::to_string(&orders)?.as_str()))
        }

        // Return the 404 Not Found for other routes.
        _ => {
            let mut not_found = Response::default();
            *not_found.status_mut() = StatusCode::NOT_FOUND;
            Ok(not_found)
        }
    }
}

// CORS headers
fn response_build(body: &str) -> Response<Body> {
    Response::builder()
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
        .header(
            "Access-Control-Allow-Headers",
            "api,Keep-Alive,User-Agent,Content-Type",
        )
        .body(Body::from(body.to_owned()))
        .unwrap()
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let opts = Opts::from_url(get_url())?;
    let builder = OptsBuilder::from_opts(opts);
    // The connection pool will have a min of 5 and max of 10 connections.
    let constraints = PoolConstraints::new(5, 10).unwrap();
    let pool_opts = PoolOpts::default().with_constraints(constraints);
    let pool = Pool::new(builder.pool_opts(pool_opts));
    dbg!(pool.clone());
    let _ = pool.get_conn().await?;

    let addr = SocketAddr::from(([0, 0, 0, 0], 8003));
    let make_svc = make_service_fn(|_| {
        let pool = pool.clone();
        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                let pool = pool.clone();
                handle_request(req, pool)
            }))
        }
    });
    let server = Server::bind(&addr).serve(make_svc);
    dbg!("Server started on port 8003");
    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
    Ok(())
}

fn get_url() -> &'static str {
    let opts = Opts::from_url(&DATABASE_URL).expect("DATABASE_URL invalid");
    if opts
        .db_name()
        .expect("a database name is required")
        .is_empty()
    {
        panic!("database name is empty");
    }
    &DATABASE_URL
}
