//! Build language-agnostic integration test definitions using JSON, then test the `socless` rust library against them.
//! Other languages can use the test descriptions generated here to ensure they conform to the latest API

mod localstack_setup;
mod models;

use hyper::Uri;
use localstack_setup::wait_for_localstack_container;
use testcontainers::clients::Cli;

// #[tokio::test]
// async fn tester_2_electric_boogaloo() {
//     // create container (it will automatically be killed when dropped from memory)
//     let client = Cli::default();
//     let container = client.run(localstack_setup::LocalstackDynamo::default());
//     container.start();

//     let localstack_port = container.get_host_port(4566);
//     let override_url = "localhost".to_string() + ":" + &localstack_port.to_string();

//     let uri = Uri::builder()
//         .scheme("http")
//         .authority(override_url)
//         .path_and_query("")
//         .build()
//         .unwrap();

//     // healthcheck that container is running
//     wait_for_localstack_container(uri.to_string())
//         .await
//         .expect("unable to reach container");

//     // println!("big sleep");
//     // std::thread::sleep(std::time::Duration::from_secs(2000));

//     // wait_for_table("table_name", &uri.to_string()).await;
// }
