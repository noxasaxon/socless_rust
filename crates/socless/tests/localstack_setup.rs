use anyhow::{bail, Result};
use hyper::Uri;
use std::{collections::HashMap, env};
use testcontainers::{clients::Cli, core::WaitFor, Image, ImageArgs};
pub struct LocalstackDynamo {
    env_vars: HashMap<String, String>,
}

impl Default for LocalstackDynamo {
    fn default() -> Self {
        let mut env_vars: HashMap<String, String> = HashMap::new();
        env_vars.insert("SERVICES".to_string(), "dynamodb".to_string());
        env_vars.insert("DEFAULT_REGION".to_string(), "us-east-1".to_string());

        Self {
            env_vars: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct LocalstackDynamoImageArgs {}

impl ImageArgs for LocalstackDynamoImageArgs {
    fn into_iterator(self) -> Box<dyn Iterator<Item = String>> {
        Box::new(Vec::default().into_iter())
    }
}

impl Image for LocalstackDynamo {
    type Args = LocalstackDynamoImageArgs;

    fn name(&self) -> String {
        "localstack/localstack".to_string()
    }

    fn tag(&self) -> String {
        "0.13.0.8".to_string()
    }

    fn ready_conditions(&self) -> Vec<WaitFor> {
        vec![WaitFor::seconds(10)]
    }

    fn expose_ports(&self) -> Vec<u16> {
        vec![4566, 4571]
    }

    fn env_vars(&self) -> Box<dyn Iterator<Item = (&String, &String)> + '_> {
        Box::new(self.env_vars.iter())
    }

    // fn volumes(&self) -> Box<dyn Iterator<Item = (&String, &String)> + '_> {
    //     Box::new(std::iter::empty())
    // }

    // fn entrypoint(&self) -> Option<String> {
    //     None
    // }

    // fn exec_after_start(&self, cs: testcontainers::core::ContainerState) -> Vec<testcontainers::core::ExecCommand> {
    //     Default::default()
    // }
}

pub async fn setup_mock_dynamo_docker() -> Uri {
    let client = Cli::default();
    let _docker_host = match env::var("DOCKER_HOST") {
        Ok(host_string) => {
            dbg!(&host_string);
            match host_string.parse::<Uri>() {
                // is there a way out of this without changing to String?
                Ok(ok) => ok.host().unwrap().to_owned(),
                Err(_) => "0.0.0.0".to_owned(),
            }
        }
        Err(_) => "0.0.0.0".to_owned(),
    };

    let container = client.run(LocalstackDynamo::default());

    container.start();

    let localstack_port = container.get_host_port(4566);
    let override_url = "localhost".to_string() + ":" + &localstack_port.to_string();

    let uri = Uri::builder()
        .scheme("http")
        .authority(override_url)
        .path_and_query("")
        .build()
        .unwrap();

    wait_for_localstack_container(uri.to_string())
        .await
        .unwrap();

    uri
}

pub async fn wait_for_localstack_container(container_url: String) -> Result<()> {
    let mut request_count = 0;
    let healthcheck_url = container_url + "health";

    loop {
        let client = reqwest::get(&healthcheck_url).await;

        match client {
            Ok(ok) => {
                if ok.status().eq(&reqwest::StatusCode::from_u16(200).unwrap()) {
                    println!("succeeded starting dynamo container");
                    return Ok(());
                }
            }
            Err(e) => {
                if request_count >= 60 {
                    bail!("unable to connect to container {}", e);
                }
                request_count += 1;
                std::thread::sleep(std::time::Duration::from_secs(1))
            }
        }
    }
}
