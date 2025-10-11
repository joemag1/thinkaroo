use std::time::Duration;
use axum::Json;
use serde::Serialize;
use tokio::time::sleep;

#[derive(Serialize)]
pub struct ReadingContents {
    pub title: String,
    pub story: String,
    pub questions: Vec<String>,
}

pub async fn reading_contents() -> Json<ReadingContents> {
    // todo: remove once we load actual contents.
    sleep(Duration::from_secs(5)).await;

    // Placeholder implementation - will be replaced with AI generation later
    let contents = ReadingContents {
        title: "A story to behold".into(),
        story: "Once upon a time, in a small village nestled between rolling hills, there lived a curious young girl named Maya. Every day after school, she would explore the forests near her home, discovering new plants and animals. One afternoon, Maya stumbled upon a hidden grove where butterflies of every color danced among wildflowers. She sat quietly, watching them for hours, learning their patterns and behaviors. From that day forward, Maya knew she wanted to become a scientist who studied nature.".to_string(),
        questions: vec![
            "What is the main character's name?".to_string(),
            "Where does Maya like to spend her time after school?".to_string(),
            "What did Maya discover in the forest?".to_string(),
            "What did Maya decide she wanted to become?".to_string(),
            "How would you describe Maya's personality based on the story?".to_string(),
        ],
    };

    Json(contents)
}
