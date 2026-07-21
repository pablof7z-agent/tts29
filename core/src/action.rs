use serde::Deserialize;
use tts29_protocol::QuestionAnswer;

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AppAction {
    SubmitAnswer {
        item_id: String,
        answers: Vec<QuestionAnswer>,
    },
    Logout,
}
