use crate::{SoclessContext, ResponsesTableItem, gen_id};

// """Initialize the human interaction worfklow by saving the Human Interaction Task Token
// to SOCless Message Responses Table

// Args:
//     execution_context (dict): The playbook execution context object that contains the task token
//     message_draft (string):  The message you intend to send. This will be stored in alongside the task token in the SOCless
//         Message Responses Table for record keeping purposes. You still have to send the message yourself in your integration
//     message_id (string): The ID to use to track both the interaction request and the human's response


// Returns:
//     A message_id to embed in your message such that is returned as part of the human's response.
//     It serves as a call_back ID to help SOCless match the users response to the right playbook execution
// """
pub async fn init_human_interaction(
    execution_context: SoclessContext,
    message_draft: &str,
    message_id: Option<&str>
) -> &str {
    let confirmed_message_id = message_id.unwrap_or(gen_id());

    let response_table_item = t;

    return "";
}


pub fn build_results_table_item(
    execution_context: SoclessContext,
    message_draft: &str,
    message_id: Option<&str>
) -> ResultsTableItem {
    // if not message_id:
    //     message_id = gen_id(6)

    // RESPONSE_TABLE = os.environ["SOCLESS_MESSAGE_RESPONSE_TABLE"]
    // response_table = boto3.resource("dynamodb").Table(RESPONSE_TABLE)
    // try:
    //     investigation_id = execution_context["artifacts"]["event"]["investigation_id"]
    //     execution_id = execution_context["execution_id"]
    //     receiver = execution_context["state_name"]
    //     task_token = execution_context["task_token"]
    //     response_table.put_item(
    //         Item={
    //             "message_id": message_id,
    //             "datetime": gen_datetimenow(),
    //             "investigation_id": investigation_id,
    //             "message": message_draft,
    //             "fulfilled": False,
    //             "execution_id": execution_id,
    //             "receiver": receiver,
    //             "await_token": task_token,
    //         }
    //     )
    // except KeyError as e:
    //     socless_log_then_raise(
    //         f"Failed to initialize human response workflow because {e} does not exist in the execution_context."
    //     )
    // except Exception as e:
    //     socless_log_then_raise(
    //         f"Failed to initialize human response workflow because {e}"
    //     )
    // return message_id
}