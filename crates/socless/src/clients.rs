use tokio::sync;
use tokio::sync::OnceCell;

pub static AWS_CONFIG: OnceCell<aws_config::Config> = OnceCell::const_new();
pub async fn get_or_init_aws_config() -> &'static aws_config::Config {
    AWS_CONFIG
        .get_or_init(|| async { aws_config::load_from_env().await })
        .await
}

pub static DYNAMO_CLIENT: OnceCell<aws_sdk_dynamodb::Client> = OnceCell::const_new();
pub async fn get_or_init_dynamo() -> &'static aws_sdk_dynamodb::Client {
    DYNAMO_CLIENT
        .get_or_init(|| async {
            let aws_config = get_or_init_aws_config().await;
            aws_sdk_dynamodb::Client::new(&aws_config)
        })
        .await
}

pub static SFN_CLIENT: OnceCell<aws_sdk_sfn::Client> = OnceCell::const_new();
pub async fn get_or_init_sfn() -> &'static aws_sdk_sfn::Client {
    SFN_CLIENT
        .get_or_init(|| async {
            let aws_config = get_or_init_aws_config().await;
            aws_sdk_sfn::Client::new(&aws_config)
        })
        .await
}

pub static S3_CLIENT: OnceCell<aws_sdk_s3::Client> = OnceCell::const_new();
pub async fn get_or_init_s3() -> &'static aws_sdk_s3::Client {
    S3_CLIENT
        .get_or_init(|| async {
            let aws_config = get_or_init_aws_config().await;
            aws_sdk_s3::Client::new(&aws_config)
        })
        .await
}
