//! Build language-agnostic integration test definitions using JSON, then test the `socless` rust library against them.
//! Other languages can use the test descriptions generated here to ensure they conform to the latest API

mod localstack_setup;
mod models;

use hyper::Uri;
use localstack_setup::wait_for_localstack_container;
use serde_json::{from_value, json, Value};
use socless::{SoclessContext, StateConfig};
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

pub fn test_context_params_with_all_resolution_types() -> (StateConfig, SoclessContext) {
    (
        from_value::<StateConfig>(json!({
            "Name" : "testing_all",
            "Parameters" : {
                "test_age-jsonpath" : "$.artifacts.event.details.age",
                // "test_age-jinja" : "{{context.artifacts.event.details.age}}",

                // "test_item_0-jsonpath" : "$.artifacts.event.details.items[0]",
                // "test_item_0-jinja" : "{{context.artifacts.event.details.items[0]}}",
                // "test_item_1_pin-jsonpath": "$.artifacts.event.details.items[1].pin",
                // "test_item_1_pin-jinja": "{{context.artifacts.event.details.items[1].pin}}",

                "test_weight-jsonpath" : "$.artifacts.event.details.weight",
                // "test_weight-jinja" : "{{context.artifacts.event.details.weight}}",

                "test_secrets-jinja" : "asdf", // TODO

                // "test_vault-jsonpath": "vault:socless_vault_tests.txt",
                // "test_vault-jinja": "vault:socless_vault_tests.txt",
            }
        }))
        .unwrap(),
        from_value::<SoclessContext>(json!({
            "artifacts": {
                "event": {
                    "details": {
                        "name": "Leshy",
                        "age": 99,
                        "items": ["camera", {"pin" : 1234}],
                        "weight" : 33.57,
                    }
                }
            }
        }))
        .unwrap(),
    )
}

#[cfg(test)]
mod tests {
    use minijinja::{context, Environment};
    use pretty_assertions::{assert_eq, assert_ne};
    use pyo3::prelude::*;
    use pyo3::types::IntoPyDict;
    use serde_json::{json, to_value, Value};
    use socless::SoclessContext;

    use crate::test_context_params_with_all_resolution_types;

    #[tokio::test]
    async fn test_build_socless_boilerplate_with_complete_event_already_set_up() {
        let (mut state_config, context) = test_context_params_with_all_resolution_types();
        state_config.resolve_parameters(&context).await;

        let details = &context.artifacts.clone().unwrap()["event"]["details"];

        assert_eq!(
            to_value(state_config.parameters).unwrap(),
            json!({
                "test_age-jsonpath" : details["age"],
                // "test_age-jinja" : details["age"],

                // "test_item_0-jsonpath" : details["items"][0],
                // "test_item_0-jinja" :details["items"][0],
                // "test_item_1_pin-jsonpath":details["items"][1]["pin"],
                // "test_item_1_pin-jinja":details["items"][1]["pin"],

                "test_weight-jsonpath" :details["weight"],
                // "test_weight-jinja" :details["weight"],

                "test_secrets-jinja" : "asdf", // TODO

                // "test_vault-jsonpath": "vault:socless_vault_tests.txt",
                // "test_vault-jinja": "vault:socless_vault_tests.txt",
            })
        )
    }

    #[tokio::test]
    async fn test_jinja() {
        pub async fn jinja_template_resolver(
            reference_path: &Value,
            root_obj: &SoclessContext,
        ) -> Value {
            // let result = Tera::one_off(reference_path.as_str().unwrap(), context, true);

            Value::from("")
        }
        let mut env = Environment::new();
        // env.add_template("test.txt", )
        env.add_template("hello.txt", "{{ context.artifacts.event.details.age}}")
            .unwrap();

        let template = env.get_template("hello.txt").unwrap();

        let (mut state_config, context) = test_context_params_with_all_resolution_types();

        let result = template.render(context!(context)).unwrap();
        dbg!(result);
        // assert!(false);
    }

    #[tokio::test]
    async fn test_pyo3() -> PyResult<()> {
        Python::with_gil(|py| {
            let sys = py.import("sys")?;
            let version: String = sys.getattr("version")?.extract()?;

            let locals = [("os", py.import("os")?)].into_py_dict(py);
            let code = "os.getenv('USER') or os.getenv('USERNAME') or 'Unknown'";
            let user: String = py.eval(code, None, Some(&locals))?.extract()?;

            println!("Hello {}, I'm Python {}", user, version);
            assert!(false);
            Ok(())
        })
    }
}
