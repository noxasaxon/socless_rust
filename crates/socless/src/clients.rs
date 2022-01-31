use aws_sdk_dynamodb::Endpoint;
use hyper::Uri;
use tokio::sync::OnceCell;

pub const AWS_ENDPOINT_URL: &str = "AWS_ENDPOINT_URL";

pub static AWS_CONFIG_AND_URL: OnceCell<(aws_config::Config, Option<String>)> =
    OnceCell::const_new();
pub async fn get_or_init_aws_config_and_url() -> &'static (aws_config::Config, Option<String>) {
    // You can select a profile by setting the `AWS_PROFILE` environment variable.
    AWS_CONFIG_AND_URL
        .get_or_init(|| async {
            (
                aws_config::load_from_env().await,
                std::env::var(AWS_ENDPOINT_URL).ok(),
            )
        })
        .await
}

pub static DYNAMO_CLIENT: OnceCell<aws_sdk_dynamodb::Client> = OnceCell::const_new();
pub async fn get_or_init_dynamo() -> &'static aws_sdk_dynamodb::Client {
    DYNAMO_CLIENT
        .get_or_init(|| async {
            let (base_config, base_url) = get_or_init_aws_config_and_url().await;

            if let Some(endpoint_url) = base_url {
                aws_sdk_dynamodb::Client::from_conf(
                    aws_sdk_dynamodb::config::Builder::from(base_config)
                        .endpoint_resolver(Endpoint::immutable(
                            endpoint_url.parse::<Uri>().expect("invald endpoint url"),
                        ))
                        .build(),
                )
            } else {
                aws_sdk_dynamodb::Client::new(&base_config)
            }
        })
        .await
}

pub static SFN_CLIENT: OnceCell<aws_sdk_sfn::Client> = OnceCell::const_new();
pub async fn get_or_init_sfn() -> &'static aws_sdk_sfn::Client {
    SFN_CLIENT
        .get_or_init(|| async {
            let (base_config, base_url) = get_or_init_aws_config_and_url().await;

            if let Some(endpoint_url) = base_url {
                aws_sdk_sfn::Client::from_conf(
                    aws_sdk_sfn::config::Builder::from(base_config)
                        .endpoint_resolver(Endpoint::immutable(
                            endpoint_url.parse::<Uri>().expect("invald endpoint url"),
                        ))
                        .build(),
                )
            } else {
                aws_sdk_sfn::Client::new(&base_config)
            }
        })
        .await
}

pub static S3_CLIENT: OnceCell<aws_sdk_s3::Client> = OnceCell::const_new();
pub async fn get_or_init_s3() -> &'static aws_sdk_s3::Client {
    S3_CLIENT
        .get_or_init(|| async {
            let (base_config, base_url) = get_or_init_aws_config_and_url().await;

            if let Some(endpoint_url) = base_url {
                aws_sdk_s3::Client::from_conf(
                    aws_sdk_s3::config::Builder::from(base_config)
                        .endpoint_resolver(Endpoint::immutable(
                            endpoint_url.parse::<Uri>().expect("invald endpoint url"),
                        ))
                        .build(),
                )
            } else {
                aws_sdk_s3::Client::new(&base_config)
            }
        })
        .await
}
